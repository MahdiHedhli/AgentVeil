use std::collections::HashSet;
use std::fmt;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Number, Value};
use thiserror::Error;

use crate::domain::FieldClass;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RewriteMode {
    Rewrite,
    BlockOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextTarget {
    path: Vec<PathSegment>,
    pub field_class: FieldClass,
    pub mode: RewriteMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PathSegment {
    Key(String),
    Index(usize),
}

#[derive(Clone, Debug)]
pub struct ValidatedRequest {
    value: Value,
    targets: Vec<TextTarget>,
}

impl ValidatedRequest {
    pub fn parse(bytes: &[u8]) -> Result<Self, PayloadError> {
        let UniqueValue(value) = serde_json::from_slice(bytes).map_err(PayloadError::Json)?;
        let root = value.as_object().ok_or(PayloadError::RootMustBeObject)?;
        ensure_allowed_keys(
            root,
            &[
                "model",
                "instructions",
                "input",
                "tools",
                "tool_choice",
                "parallel_tool_calls",
                "reasoning",
                "store",
                "stream",
                "stream_options",
                "include",
                "service_tier",
                "prompt_cache_key",
                "text",
                "client_metadata",
            ],
            PayloadError::UnsupportedTopLevelField,
        )?;
        require_string(root, "model")?;
        require_bool(root, "store")?;
        if root.get("stream").and_then(Value::as_bool) != Some(true) {
            return Err(PayloadError::StreamingRequired);
        }

        let mut targets = Vec::new();
        if let Some(instructions) = root.get("instructions") {
            if !instructions.is_string() {
                return Err(PayloadError::InvalidFieldType("instructions"));
            }
            targets.push(TextTarget::new(
                [PathSegment::key("instructions")],
                FieldClass::Instructions,
                RewriteMode::Rewrite,
            ));
        }

        let input = root
            .get("input")
            .ok_or(PayloadError::MissingField("input"))?;
        match input {
            Value::String(_) => targets.push(TextTarget::new(
                [PathSegment::key("input")],
                FieldClass::MessageText,
                RewriteMode::Rewrite,
            )),
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    collect_input_item(item, index, &mut targets)?;
                }
            }
            _ => return Err(PayloadError::InvalidFieldType("input")),
        }

        if let Some(tools) = root.get("tools") {
            collect_tool_array(tools, vec![PathSegment::key("tools")], &mut targets)?;
        }
        if let Some(metadata) = root.get("client_metadata") {
            let object = metadata
                .as_object()
                .ok_or(PayloadError::InvalidFieldType("client_metadata"))?;
            for (key, value) in object {
                if !value.is_string() {
                    return Err(PayloadError::InvalidFieldType("client_metadata value"));
                }
                targets.push(TextTarget::new(
                    [
                        PathSegment::key("client_metadata"),
                        PathSegment::Key(key.clone()),
                    ],
                    FieldClass::ClientMetadata,
                    RewriteMode::BlockOnly,
                ));
            }
        }

        Ok(Self { value, targets })
    }

    pub fn targets(&self) -> &[TextTarget] {
        &self.targets
    }

    pub fn text(&self, target: &TextTarget) -> Result<&str, PayloadError> {
        locate(&self.value, &target.path)?
            .as_str()
            .ok_or(PayloadError::TargetChangedType)
    }

    pub fn replace_text(
        &mut self,
        target_index: usize,
        replacement: String,
    ) -> Result<(), PayloadError> {
        let target = self
            .targets
            .get(target_index)
            .ok_or(PayloadError::TargetMissing)?;
        let value = locate_mut(&mut self.value, &target.path)?;
        if !value.is_string() {
            return Err(PayloadError::TargetChangedType);
        }
        *value = Value::String(replacement);
        Ok(())
    }

    pub fn serialize(&self) -> Result<Vec<u8>, PayloadError> {
        serde_json::to_vec(&self.value).map_err(PayloadError::Serialize)
    }
}

impl TextTarget {
    fn new(
        path: impl IntoIterator<Item = PathSegment>,
        field_class: FieldClass,
        mode: RewriteMode,
    ) -> Self {
        Self {
            path: path.into_iter().collect(),
            field_class,
            mode,
        }
    }
}

impl PathSegment {
    fn key(value: &str) -> Self {
        Self::Key(value.to_string())
    }
}

fn collect_input_item(
    item: &Value,
    index: usize,
    targets: &mut Vec<TextTarget>,
) -> Result<(), PayloadError> {
    let object = item
        .as_object()
        .ok_or(PayloadError::InputItemMustBeObject)?;
    let item_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or(PayloadError::MissingItemType)?;
    let base = vec![PathSegment::key("input"), PathSegment::Index(index)];
    match item_type {
        "message" => collect_message(object, base, targets, FieldClass::MessageText),
        "agent_message" => collect_agent_message(object, base, targets),
        "function_call_output" => collect_output_payload(
            object,
            base,
            targets,
            FieldClass::FunctionOutput,
            &[
                "id",
                "type",
                "call_id",
                "output",
                "internal_chat_message_metadata_passthrough",
            ],
        ),
        "custom_tool_call_output" => collect_output_payload(
            object,
            base,
            targets,
            FieldClass::CustomToolOutput,
            &[
                "id",
                "type",
                "call_id",
                "name",
                "output",
                "internal_chat_message_metadata_passthrough",
            ],
        ),
        "function_call" => collect_string_field(
            object,
            base,
            targets,
            "arguments",
            FieldClass::FunctionArguments,
            RewriteMode::Rewrite,
            &[
                "id",
                "type",
                "name",
                "namespace",
                "arguments",
                "call_id",
                "internal_chat_message_metadata_passthrough",
            ],
        ),
        "custom_tool_call" => collect_string_field(
            object,
            base,
            targets,
            "input",
            FieldClass::CustomToolInput,
            RewriteMode::Rewrite,
            &[
                "id",
                "type",
                "status",
                "call_id",
                "name",
                "namespace",
                "input",
                "internal_chat_message_metadata_passthrough",
            ],
        ),
        "additional_tools" => {
            ensure_allowed_keys(
                object,
                &["id", "type", "role", "tools"],
                PayloadError::UnsupportedItemField,
            )?;
            let mut path = base;
            path.push(PathSegment::key("tools"));
            collect_tool_array(
                object
                    .get("tools")
                    .ok_or(PayloadError::MissingField("tools"))?,
                path,
                targets,
            )
        }
        "reasoning" => collect_reasoning(object, base, targets),
        "local_shell_call" | "tool_search_call" | "tool_search_output" | "web_search_call" => {
            collect_known_structural_item(object, base, targets)
        }
        "compaction" | "compaction_summary" | "context_compaction" | "compaction_trigger" => {
            collect_compaction(object)
        }
        "image_generation_call" => Err(PayloadError::UnsupportedMedia),
        _ => Err(PayloadError::UnsupportedInputItem),
    }
}

fn collect_message(
    object: &Map<String, Value>,
    base: Vec<PathSegment>,
    targets: &mut Vec<TextTarget>,
    field_class: FieldClass,
) -> Result<(), PayloadError> {
    ensure_allowed_keys(
        object,
        &[
            "id",
            "type",
            "role",
            "content",
            "phase",
            "internal_chat_message_metadata_passthrough",
        ],
        PayloadError::UnsupportedItemField,
    )?;
    let content = object
        .get("content")
        .ok_or(PayloadError::MissingField("content"))?;
    if content.is_string() {
        let mut path = base;
        path.push(PathSegment::key("content"));
        targets.push(TextTarget::new(path, field_class, RewriteMode::Rewrite));
        return Ok(());
    }
    collect_content_array(content, base, targets, field_class)
}

fn collect_agent_message(
    object: &Map<String, Value>,
    base: Vec<PathSegment>,
    targets: &mut Vec<TextTarget>,
) -> Result<(), PayloadError> {
    ensure_allowed_keys(
        object,
        &[
            "id",
            "type",
            "author",
            "recipient",
            "content",
            "internal_chat_message_metadata_passthrough",
        ],
        PayloadError::UnsupportedItemField,
    )?;
    let content = object
        .get("content")
        .and_then(Value::as_array)
        .ok_or(PayloadError::InvalidFieldType("agent_message.content"))?;
    for (part_index, part) in content.iter().enumerate() {
        let part_object = part
            .as_object()
            .ok_or(PayloadError::ContentPartMustBeObject)?;
        match part_object.get("type").and_then(Value::as_str) {
            Some("input_text") => {
                ensure_text_part(part_object)?;
                let mut path = base.clone();
                path.extend([
                    PathSegment::key("content"),
                    PathSegment::Index(part_index),
                    PathSegment::key("text"),
                ]);
                targets.push(TextTarget::new(
                    path,
                    FieldClass::AgentMessageText,
                    RewriteMode::Rewrite,
                ));
            }
            Some("encrypted_content") => ensure_allowed_keys(
                part_object,
                &["type", "encrypted_content"],
                PayloadError::UnsupportedContentPartField,
            )?,
            _ => return Err(PayloadError::UnsupportedContentPart),
        }
    }
    Ok(())
}

fn collect_content_array(
    content: &Value,
    base: Vec<PathSegment>,
    targets: &mut Vec<TextTarget>,
    field_class: FieldClass,
) -> Result<(), PayloadError> {
    let parts = content
        .as_array()
        .ok_or(PayloadError::InvalidFieldType("content"))?;
    for (part_index, part) in parts.iter().enumerate() {
        let object = part
            .as_object()
            .ok_or(PayloadError::ContentPartMustBeObject)?;
        match object.get("type").and_then(Value::as_str) {
            Some("input_text" | "output_text") => {
                ensure_text_part(object)?;
                let mut path = base.clone();
                path.extend([
                    PathSegment::key("content"),
                    PathSegment::Index(part_index),
                    PathSegment::key("text"),
                ]);
                targets.push(TextTarget::new(path, field_class, RewriteMode::Rewrite));
            }
            Some("input_image" | "input_file") => return Err(PayloadError::UnsupportedMedia),
            _ => return Err(PayloadError::UnsupportedContentPart),
        }
    }
    Ok(())
}

fn ensure_text_part(object: &Map<String, Value>) -> Result<(), PayloadError> {
    ensure_allowed_keys(
        object,
        &["type", "text"],
        PayloadError::UnsupportedContentPartField,
    )?;
    require_string(object, "text")
}

fn collect_output_payload(
    object: &Map<String, Value>,
    base: Vec<PathSegment>,
    targets: &mut Vec<TextTarget>,
    field_class: FieldClass,
    allowed_keys: &[&str],
) -> Result<(), PayloadError> {
    ensure_allowed_keys(object, allowed_keys, PayloadError::UnsupportedItemField)?;
    let output = object
        .get("output")
        .ok_or(PayloadError::MissingField("output"))?;
    if output.is_string() {
        let mut path = base;
        path.push(PathSegment::key("output"));
        targets.push(TextTarget::new(path, field_class, RewriteMode::Rewrite));
        return Ok(());
    }
    let parts = output
        .as_array()
        .ok_or(PayloadError::InvalidFieldType("output"))?;
    for (part_index, part) in parts.iter().enumerate() {
        let part_object = part
            .as_object()
            .ok_or(PayloadError::ContentPartMustBeObject)?;
        match part_object.get("type").and_then(Value::as_str) {
            Some("input_text") => {
                ensure_text_part(part_object)?;
                let mut path = base.clone();
                path.extend([
                    PathSegment::key("output"),
                    PathSegment::Index(part_index),
                    PathSegment::key("text"),
                ]);
                targets.push(TextTarget::new(path, field_class, RewriteMode::Rewrite));
            }
            Some("encrypted_content") => ensure_allowed_keys(
                part_object,
                &["type", "encrypted_content"],
                PayloadError::UnsupportedContentPartField,
            )?,
            Some("input_image") => return Err(PayloadError::UnsupportedMedia),
            _ => return Err(PayloadError::UnsupportedContentPart),
        }
    }
    Ok(())
}

fn collect_string_field(
    object: &Map<String, Value>,
    mut base: Vec<PathSegment>,
    targets: &mut Vec<TextTarget>,
    field: &'static str,
    field_class: FieldClass,
    mode: RewriteMode,
    allowed_keys: &[&str],
) -> Result<(), PayloadError> {
    ensure_allowed_keys(object, allowed_keys, PayloadError::UnsupportedItemField)?;
    require_string(object, field)?;
    base.push(PathSegment::key(field));
    targets.push(TextTarget::new(base, field_class, mode));
    Ok(())
}

fn collect_reasoning(
    object: &Map<String, Value>,
    base: Vec<PathSegment>,
    targets: &mut Vec<TextTarget>,
) -> Result<(), PayloadError> {
    ensure_allowed_keys(
        object,
        &[
            "id",
            "type",
            "summary",
            "content",
            "encrypted_content",
            "internal_chat_message_metadata_passthrough",
        ],
        PayloadError::UnsupportedItemField,
    )?;
    for field in ["summary", "content"] {
        let Some(parts) = object.get(field) else {
            continue;
        };
        if parts.is_null() {
            continue;
        }
        let parts = parts
            .as_array()
            .ok_or(PayloadError::InvalidFieldType("reasoning text"))?;
        for (index, part) in parts.iter().enumerate() {
            let part_object = part
                .as_object()
                .ok_or(PayloadError::ContentPartMustBeObject)?;
            match part_object.get("type").and_then(Value::as_str) {
                Some("summary_text" | "reasoning_text") => ensure_text_part(part_object)?,
                _ => return Err(PayloadError::UnsupportedContentPart),
            }
            let mut path = base.clone();
            path.extend([
                PathSegment::key(field),
                PathSegment::Index(index),
                PathSegment::key("text"),
            ]);
            targets.push(TextTarget::new(
                path,
                FieldClass::ReasoningSummary,
                RewriteMode::Rewrite,
            ));
        }
    }
    Ok(())
}

fn collect_known_structural_item(
    object: &Map<String, Value>,
    base: Vec<PathSegment>,
    targets: &mut Vec<TextTarget>,
) -> Result<(), PayloadError> {
    for (key, value) in object {
        if matches!(
            key.as_str(),
            "id" | "type" | "call_id" | "status" | "internal_chat_message_metadata_passthrough"
        ) {
            continue;
        }
        let mut path = base.clone();
        path.push(PathSegment::Key(key.clone()));
        collect_all_strings(value, path, targets, FieldClass::FunctionArguments)?;
    }
    Ok(())
}

fn collect_compaction(object: &Map<String, Value>) -> Result<(), PayloadError> {
    ensure_allowed_keys(
        object,
        &[
            "id",
            "type",
            "encrypted_content",
            "internal_chat_message_metadata_passthrough",
        ],
        PayloadError::UnsupportedItemField,
    )
}

fn collect_tool_array(
    tools: &Value,
    path: Vec<PathSegment>,
    targets: &mut Vec<TextTarget>,
) -> Result<(), PayloadError> {
    let tools = tools
        .as_array()
        .ok_or(PayloadError::InvalidFieldType("tools"))?;
    for (index, tool) in tools.iter().enumerate() {
        let mut tool_path = path.clone();
        tool_path.push(PathSegment::Index(index));
        collect_tool_value(tool, tool_path, targets, None)?;
    }
    Ok(())
}

fn collect_tool_value(
    value: &Value,
    path: Vec<PathSegment>,
    targets: &mut Vec<TextTarget>,
    key: Option<&str>,
) -> Result<(), PayloadError> {
    match value {
        Value::String(_) => {
            if matches!(
                key,
                Some("name" | "type" | "namespace" | "id" | "$id" | "$ref")
            ) {
                return Ok(());
            }
            let field_class = if key == Some("description") {
                FieldClass::ToolDescription
            } else {
                FieldClass::ToolSchemaText
            };
            targets.push(TextTarget::new(path, field_class, RewriteMode::BlockOnly));
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                let mut child_path = path.clone();
                child_path.push(PathSegment::Index(index));
                collect_tool_value(child, child_path, targets, key)?;
            }
        }
        Value::Object(object) => {
            for (child_key, child) in object {
                let mut child_path = path.clone();
                child_path.push(PathSegment::Key(child_key.clone()));
                collect_tool_value(child, child_path, targets, Some(child_key))?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn collect_all_strings(
    value: &Value,
    path: Vec<PathSegment>,
    targets: &mut Vec<TextTarget>,
    field_class: FieldClass,
) -> Result<(), PayloadError> {
    match value {
        Value::String(_) => {
            targets.push(TextTarget::new(path, field_class, RewriteMode::BlockOnly))
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                let mut child_path = path.clone();
                child_path.push(PathSegment::Index(index));
                collect_all_strings(child, child_path, targets, field_class)?;
            }
        }
        Value::Object(object) => {
            for (key, child) in object {
                let mut child_path = path.clone();
                child_path.push(PathSegment::Key(key.clone()));
                collect_all_strings(child, child_path, targets, field_class)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn require_string(object: &Map<String, Value>, field: &'static str) -> Result<(), PayloadError> {
    if object.get(field).and_then(Value::as_str).is_none() {
        return Err(if object.contains_key(field) {
            PayloadError::InvalidFieldType(field)
        } else {
            PayloadError::MissingField(field)
        });
    }
    Ok(())
}

fn require_bool(object: &Map<String, Value>, field: &'static str) -> Result<(), PayloadError> {
    if object.get(field).and_then(Value::as_bool).is_none() {
        return Err(if object.contains_key(field) {
            PayloadError::InvalidFieldType(field)
        } else {
            PayloadError::MissingField(field)
        });
    }
    Ok(())
}

fn ensure_allowed_keys<F>(
    object: &Map<String, Value>,
    allowed: &[&str],
    error: F,
) -> Result<(), PayloadError>
where
    F: Fn(String) -> PayloadError,
{
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(error(key.clone()));
        }
    }
    Ok(())
}

fn locate<'a>(root: &'a Value, path: &[PathSegment]) -> Result<&'a Value, PayloadError> {
    let mut current = root;
    for segment in path {
        current = match segment {
            PathSegment::Key(key) => current
                .as_object()
                .and_then(|object| object.get(key))
                .ok_or(PayloadError::TargetMissing)?,
            PathSegment::Index(index) => current
                .as_array()
                .and_then(|array| array.get(*index))
                .ok_or(PayloadError::TargetMissing)?,
        };
    }
    Ok(current)
}

fn locate_mut<'a>(
    root: &'a mut Value,
    path: &[PathSegment],
) -> Result<&'a mut Value, PayloadError> {
    let mut current = root;
    for segment in path {
        current = match segment {
            PathSegment::Key(key) => current
                .as_object_mut()
                .and_then(|object| object.get_mut(key))
                .ok_or(PayloadError::TargetMissing)?,
            PathSegment::Index(index) => current
                .as_array_mut()
                .and_then(|array| array.get_mut(*index))
                .ok_or(PayloadError::TargetMissing)?,
        };
    }
    Ok(current)
}

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueValueVisitor).map(Self)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_string()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        UniqueValue::deserialize(deserializer).map(|value| value.0)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(UniqueValue(value)) = sequence.next_element::<UniqueValue>()? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut object = Map::new();
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            let UniqueValue(value) = map.next_value::<UniqueValue>()?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

#[derive(Debug, Error)]
pub enum PayloadError {
    #[error("request JSON is invalid or contains duplicate keys")]
    Json(#[source] serde_json::Error),
    #[error("request JSON root must be an object")]
    RootMustBeObject,
    #[error("request is missing required field {0}")]
    MissingField(&'static str),
    #[error("request field {0} has an invalid type")]
    InvalidFieldType(&'static str),
    #[error("Responses SSE streaming is required")]
    StreamingRequired,
    #[error("unsupported top-level request field {0}")]
    UnsupportedTopLevelField(String),
    #[error("input item must be an object")]
    InputItemMustBeObject,
    #[error("input item has no type")]
    MissingItemType,
    #[error("unsupported input item type")]
    UnsupportedInputItem,
    #[error("unsupported input item field {0}")]
    UnsupportedItemField(String),
    #[error("content part must be an object")]
    ContentPartMustBeObject,
    #[error("unsupported content part")]
    UnsupportedContentPart,
    #[error("unsupported content part field {0}")]
    UnsupportedContentPartField(String),
    #[error("image, file, or binary content is unsupported")]
    UnsupportedMedia,
    #[error("validated target is missing")]
    TargetMissing,
    #[error("validated target changed type")]
    TargetChangedType,
    #[error("sanitized request serialization failed")]
    Serialize(#[source] serde_json::Error),
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn request(input: Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "model": "gpt-5.6-luna",
            "instructions": "be useful",
            "input": input,
            "tools": [],
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "reasoning": {"effort": "medium"},
            "store": false,
            "stream": true,
            "include": []
        }))
        .expect("fixture should serialize")
    }

    #[test]
    fn collects_message_and_function_output_text_only() {
        let bytes = request(serde_json::json!([
            {"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]},
            {"type":"function_call_output","call_id":"safe-id","output":"tool result"}
        ]));
        let mut parsed = ValidatedRequest::parse(&bytes).expect("fixture should validate");
        assert_eq!(parsed.targets().len(), 3);
        assert_eq!(
            parsed
                .text(&parsed.targets()[1])
                .expect("target should exist"),
            "hello"
        );
        parsed
            .replace_text(1, "protected".to_string())
            .expect("target should rewrite");
        let serialized: Value =
            serde_json::from_slice(&parsed.serialize().expect("should serialize"))
                .expect("serialized request should parse");
        assert_eq!(serialized["input"][0]["content"][0]["text"], "protected");
        assert_eq!(serialized["input"][1]["call_id"], "safe-id");
    }

    #[test]
    fn rejects_unknown_item_content_and_media() {
        for input in [
            serde_json::json!([{"type":"future_tool_output","output":"text"}]),
            serde_json::json!([{"type":"message","role":"user","content":[{"type":"future_text","text":"text"}]}]),
            serde_json::json!([{"type":"message","role":"user","content":[{"type":"input_image","image_url":"data:synthetic"}]}]),
        ] {
            assert!(ValidatedRequest::parse(&request(input)).is_err());
        }
    }

    #[test]
    fn rejects_unknown_top_level_context_and_duplicate_keys() {
        let unknown = br#"{"model":"gpt-5.6","input":"safe","store":false,"stream":true,"future_context":"text"}"#;
        assert!(matches!(
            ValidatedRequest::parse(unknown),
            Err(PayloadError::UnsupportedTopLevelField(_))
        ));
        let duplicate =
            br#"{"model":"gpt-5.6","input":"first","input":"second","store":false,"stream":true}"#;
        assert!(matches!(
            ValidatedRequest::parse(duplicate),
            Err(PayloadError::Json(_))
        ));
    }
}
