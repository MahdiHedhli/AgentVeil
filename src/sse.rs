use bytes::Bytes;
use serde_json::Value;
use thiserror::Error;

const MAX_SSE_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SseRestorationMode {
    SyntheticDisplayText,
    SyntheticDisplayDeltaOnly,
}

pub struct SseTransformer {
    buffer: Vec<u8>,
    restoration_mode: SseRestorationMode,
}

impl Default for SseTransformer {
    fn default() -> Self {
        Self::new(SseRestorationMode::SyntheticDisplayText)
    }
}

impl SseTransformer {
    pub fn new(restoration_mode: SseRestorationMode) -> Self {
        Self {
            buffer: Vec::new(),
            restoration_mode,
        }
    }

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
            frames.push(Bytes::from(transform_frame(
                &frame,
                self.restoration_mode,
                &mut restore,
            )?));
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

fn transform_frame<F>(
    frame: &[u8],
    restoration_mode: SseRestorationMode,
    restore: &mut F,
) -> Result<Vec<u8>, SseError>
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
    if !is_restorable_event(restoration_mode, &event_type) {
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

fn is_restorable_event(restoration_mode: SseRestorationMode, event_type: &str) -> bool {
    match restoration_mode {
        SseRestorationMode::SyntheticDisplayDeltaOnly => event_type == "response.output_text.delta",
        SseRestorationMode::SyntheticDisplayText => matches!(
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
        ),
    }
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

    fn typed_event(event_type: &str, event: Value) -> String {
        format!(
            "event: {event_type}\ndata: {}\n\n",
            serde_json::to_string(&event).expect("fixture should serialize")
        )
    }

    fn event_values(frames: &[Bytes]) -> Vec<Value> {
        frames
            .iter()
            .map(|frame| {
                let frame = std::str::from_utf8(frame).expect("frame should be UTF-8");
                let data = frame
                    .lines()
                    .find_map(|line| line.strip_prefix("data: "))
                    .expect("frame should contain data");
                serde_json::from_str(data).expect("event data should parse")
            })
            .collect()
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
    fn delta_only_mode_keeps_completed_history_items_tokenized() {
        let message = serde_json::json!({
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": TOKEN}]
        });
        let source = [
            typed_event(
                "response.output_text.delta",
                serde_json::json!({
                    "type": "response.output_text.delta",
                    "sequence_number": 1,
                    "delta": TOKEN
                }),
            ),
            typed_event(
                "response.output_text.done",
                serde_json::json!({
                    "type": "response.output_text.done",
                    "sequence_number": 2,
                    "text": TOKEN
                }),
            ),
            typed_event(
                "response.content_part.done",
                serde_json::json!({
                    "type": "response.content_part.done",
                    "sequence_number": 3,
                    "part": {"type": "output_text", "text": TOKEN}
                }),
            ),
            typed_event(
                "response.output_item.done",
                serde_json::json!({
                    "type": "response.output_item.done",
                    "sequence_number": 4,
                    "item": message.clone()
                }),
            ),
            typed_event(
                "response.completed",
                serde_json::json!({
                    "type": "response.completed",
                    "sequence_number": 5,
                    "response": {"output": [message]}
                }),
            ),
        ]
        .concat();
        let mut transformer = SseTransformer::new(SseRestorationMode::SyntheticDisplayDeltaOnly);
        let frames = transformer
            .push(source.as_bytes(), |text| {
                text.replace(TOKEN, "ava@example.test")
            })
            .expect("events should transform");
        transformer.finish().expect("stream should finish");
        let events = event_values(&frames);

        assert_eq!(events[0]["delta"], "ava@example.test");
        assert_eq!(events[1]["text"], TOKEN);
        assert_eq!(events[2]["part"]["text"], TOKEN);
        assert_eq!(events[3]["item"]["content"][0]["text"], TOKEN);
        assert_eq!(
            events[4]["response"]["output"][0]["content"][0]["text"],
            TOKEN
        );
    }

    #[test]
    fn full_synthetic_mode_preserves_existing_snapshot_restoration() {
        let source = typed_event(
            "response.output_item.done",
            serde_json::json!({
                "type": "response.output_item.done",
                "sequence_number": 1,
                "item": {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": TOKEN}]
                }
            }),
        );
        let mut transformer = SseTransformer::new(SseRestorationMode::SyntheticDisplayText);
        let frames = transformer
            .push(source.as_bytes(), |text| {
                text.replace(TOKEN, "ava@example.test")
            })
            .expect("event should transform");
        let events = event_values(&frames);
        assert_eq!(events[0]["item"]["content"][0]["text"], "ava@example.test");
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
