use bytes::Bytes;
use serde_json::Value;
use thiserror::Error;

const MAX_SSE_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Default)]
pub struct SseTransformer {
    buffer: Vec<u8>,
}

impl SseTransformer {
    pub fn push<F>(&mut self, chunk: &[u8], mut restore: F) -> Result<Vec<Bytes>, SseError>
    where
        F: FnMut(&str) -> String,
    {
        self.buffer.extend_from_slice(chunk);
        if self.buffer.len() > MAX_SSE_EVENT_BYTES && find_frame_end(&self.buffer).is_none() {
            return Err(SseError::OversizeEvent);
        }
        let mut frames = Vec::new();
        while let Some(end) = find_frame_end(&self.buffer) {
            let frame: Vec<u8> = self.buffer.drain(..end).collect();
            frames.push(Bytes::from(transform_frame(&frame, &mut restore)?));
        }
        Ok(frames)
    }

    pub fn finish(self) -> Result<(), SseError> {
        if self.buffer.iter().all(u8::is_ascii_whitespace) {
            Ok(())
        } else {
            Err(SseError::TruncatedEvent)
        }
    }
}

fn find_frame_end(bytes: &[u8]) -> Option<usize> {
    for index in 0..bytes.len() {
        if bytes.get(index..index + 2) == Some(b"\n\n") {
            return Some(index + 2);
        }
        if bytes.get(index..index + 4) == Some(b"\r\n\r\n") {
            return Some(index + 4);
        }
    }
    None
}

fn transform_frame<F>(frame: &[u8], restore: &mut F) -> Result<Vec<u8>, SseError>
where
    F: FnMut(&str) -> String,
{
    let text = std::str::from_utf8(frame).map_err(|_| SseError::InvalidUtf8)?;
    let mut event_type_header = None;
    let mut data_line = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        if let Some(value) = content.strip_prefix("event:") {
            event_type_header = Some(value.trim());
        }
        if content.starts_with("data:") {
            if data_line.is_some() {
                return Err(SseError::MultipleDataLines);
            }
            data_line = Some((offset, offset + line.len(), line));
        }
        offset += line.len();
    }
    let Some((start, end, original_line)) = data_line else {
        return Ok(frame.to_vec());
    };
    let line_without_newline = original_line.trim_end_matches(['\r', '\n']);
    let newline = &original_line[line_without_newline.len()..];
    let data = line_without_newline
        .strip_prefix("data:")
        .ok_or(SseError::MissingData)?
        .trim_start();
    if data == "[DONE]" {
        return Ok(frame.to_vec());
    }
    let mut event: Value = serde_json::from_str(data).map_err(|_| SseError::InvalidJson)?;
    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .ok_or(SseError::MissingEventType)?
        .to_string();
    if event_type_header.is_some_and(|header| header != event_type) {
        return Err(SseError::MismatchedEventType);
    }
    if !is_restorable_event(&event_type) {
        return Ok(frame.to_vec());
    }
    restore_event(&event_type, &mut event, restore)?;
    let serialized = serde_json::to_string(&event).map_err(|_| SseError::Serialize)?;
    let mut output = Vec::with_capacity(frame.len() + serialized.len());
    output.extend_from_slice(&frame[..start]);
    output.extend_from_slice(b"data: ");
    output.extend_from_slice(serialized.as_bytes());
    output.extend_from_slice(newline.as_bytes());
    output.extend_from_slice(&frame[end..]);
    Ok(output)
}

fn is_restorable_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "response.output_text.delta"
            | "response.output_text.done"
            | "response.content_part.added"
            | "response.content_part.done"
            | "response.output_item.added"
            | "response.output_item.done"
            | "response.created"
            | "response.in_progress"
            | "response.completed"
    )
}

fn restore_event<F>(event_type: &str, event: &mut Value, restore: &mut F) -> Result<(), SseError>
where
    F: FnMut(&str) -> String,
{
    match event_type {
        "response.output_text.delta" => restore_string_field(event, "delta", restore),
        "response.output_text.done" => restore_string_field(event, "text", restore),
        "response.content_part.added" | "response.content_part.done" => {
            if let Some(part) = event.get_mut("part") {
                restore_output_text_part(part, restore)?;
            }
            Ok(())
        }
        "response.output_item.added" | "response.output_item.done" => {
            if let Some(item) = event.get_mut("item") {
                restore_message_item(item, restore)?;
            }
            Ok(())
        }
        "response.created" | "response.in_progress" | "response.completed" => {
            if let Some(response) = event.get_mut("response") {
                restore_response_snapshot(response, restore)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn restore_response_snapshot<F>(response: &mut Value, restore: &mut F) -> Result<(), SseError>
where
    F: FnMut(&str) -> String,
{
    let Some(output) = response.get_mut("output") else {
        return Ok(());
    };
    let output = output.as_array_mut().ok_or(SseError::InvalidTextShape)?;
    for item in output {
        restore_message_item(item, restore)?;
    }
    Ok(())
}

fn restore_message_item<F>(item: &mut Value, restore: &mut F) -> Result<(), SseError>
where
    F: FnMut(&str) -> String,
{
    if item.get("type").and_then(Value::as_str) != Some("message") {
        return Ok(());
    }
    let Some(content) = item.get_mut("content") else {
        return Ok(());
    };
    let content = content.as_array_mut().ok_or(SseError::InvalidTextShape)?;
    for part in content {
        restore_output_text_part(part, restore)?;
    }
    Ok(())
}

fn restore_output_text_part<F>(part: &mut Value, restore: &mut F) -> Result<(), SseError>
where
    F: FnMut(&str) -> String,
{
    if part.get("type").and_then(Value::as_str) == Some("output_text") {
        restore_string_field(part, "text", restore)?;
    }
    Ok(())
}

fn restore_string_field<F>(
    value: &mut Value,
    field: &'static str,
    restore: &mut F,
) -> Result<(), SseError>
where
    F: FnMut(&str) -> String,
{
    let Some(current) = value.get_mut(field) else {
        return Ok(());
    };
    let text = current.as_str().ok_or(SseError::InvalidTextShape)?;
    *current = Value::String(restore(text));
    Ok(())
}

#[derive(Debug, Error)]
pub enum SseError {
    #[error("SSE event exceeds the local limit")]
    OversizeEvent,
    #[error("SSE event is not valid UTF-8")]
    InvalidUtf8,
    #[error("SSE event has multiple data lines")]
    MultipleDataLines,
    #[error("SSE event has no data payload")]
    MissingData,
    #[error("SSE data is not valid JSON")]
    InvalidJson,
    #[error("SSE data has no typed event name")]
    MissingEventType,
    #[error("SSE event header and JSON type differ")]
    MismatchedEventType,
    #[error("SSE text-bearing field has an unsupported shape")]
    InvalidTextShape,
    #[error("SSE event serialization failed")]
    Serialize,
    #[error("SSE stream ended with a truncated event")]
    TruncatedEvent,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const TOKEN: &str = "[AV_EMAIL_01010101010101010101010101010101]";

    fn event(text: &str) -> Vec<u8> {
        format!(
            "event: response.output_text.delta\ndata: {{\"type\":\"response.output_text.delta\",\"sequence_number\":4,\"delta\":{}}}\n\n",
            serde_json::to_string(text).expect("fixture should serialize")
        )
        .into_bytes()
    }

    #[test]
    fn restores_text_across_every_transport_split() {
        let source = event(&format!("pré🙂 {TOKEN} 終"));
        for split in 0..=source.len() {
            let mut transformer = SseTransformer::default();
            let mut output = Vec::new();
            for chunk in [&source[..split], &source[split..]] {
                for frame in transformer
                    .push(chunk, |text| text.replace(TOKEN, "ava@example.test"))
                    .expect("fragment should transform")
                {
                    output.extend_from_slice(&frame);
                }
            }
            transformer.finish().expect("stream should finish");
            let text = String::from_utf8(output).expect("output should be UTF-8");
            assert!(text.contains("pré🙂 ava@example.test 終"));
            assert!(!text.contains(TOKEN));
        }
    }

    #[test]
    fn never_restores_structural_or_function_argument_fields() {
        let frame = format!(
            "event: response.function_call_arguments.delta\ndata: {{\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"{TOKEN}\",\"delta\":\"{TOKEN}\"}}\n\n"
        );
        let mut transformer = SseTransformer::default();
        let output = transformer
            .push(frame.as_bytes(), |text| {
                text.replace(TOKEN, "ava@example.test")
            })
            .expect("event should pass");
        let text = String::from_utf8(output[0].to_vec()).expect("output should be UTF-8");
        assert_eq!(text, frame);
    }

    #[test]
    fn rejects_malformed_or_truncated_events_without_emitting_frame() {
        let malformed = b"event: response.output_text.delta\ndata: {nope}\n\n";
        let mut transformer = SseTransformer::default();
        assert!(matches!(
            transformer.push(malformed, str::to_string),
            Err(SseError::InvalidJson)
        ));

        let mut transformer = SseTransformer::default();
        assert!(
            transformer
                .push(b"event: response.output_text.delta\n", str::to_string)
                .expect("partial event should buffer")
                .is_empty()
        );
        assert!(matches!(
            transformer.finish(),
            Err(SseError::TruncatedEvent)
        ));
    }
}
