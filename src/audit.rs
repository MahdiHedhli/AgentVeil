use serde::Serialize;
use thiserror::Error;

use crate::domain::{Action, Confidence, DataClass, DetectorId, FieldClass};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditDecision {
    Allowed,
    Rewritten,
    Blocked,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpstreamOutcome {
    NotStarted,
    Pending,
    Started,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditFinding {
    pub detector: DetectorId,
    pub data_class: DataClass,
    pub confidence: Confidence,
    pub action: Action,
    pub source_field: FieldClass,
    pub count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditEvent {
    pub schema_version: u8,
    pub event_id: String,
    pub timestamp_unix_ms: u64,
    pub session_pseudonym: String,
    pub request_sequence: u64,
    pub route: AuditRoute,
    pub decision: AuditDecision,
    pub findings: Vec<AuditFinding>,
    pub upstream_outcome: UpstreamOutcome,
    pub scan_latency_ms: u64,
    pub policy_name: String,
    pub policy_hash: String,
    pub agentveil_version: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditRoute {
    Responses,
    Models,
}

impl AuditEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_id: String,
        timestamp_unix_ms: u64,
        session_pseudonym: String,
        request_sequence: u64,
        decision: AuditDecision,
        findings: Vec<AuditFinding>,
        upstream_outcome: UpstreamOutcome,
        scan_latency_ms: u64,
        policy_name: String,
        policy_hash: String,
        agentveil_version: String,
    ) -> Result<Self, AuditError> {
        if !safe_identifier(&event_id, 8, 72)
            || !safe_identifier(&session_pseudonym, 8, 72)
            || !safe_identifier(&policy_name, 1, 48)
            || !safe_hex(&policy_hash, 64)
            || !safe_version(&agentveil_version)
        {
            return Err(AuditError::UnsafeMetadata);
        }
        Ok(Self {
            schema_version: 1,
            event_id,
            timestamp_unix_ms,
            session_pseudonym,
            request_sequence,
            route: AuditRoute::Responses,
            decision,
            findings,
            upstream_outcome,
            scan_latency_ms,
            policy_name,
            policy_hash,
            agentveil_version,
        })
    }

    pub fn to_json_line(&self) -> Result<Vec<u8>, AuditError> {
        let mut bytes = serde_json::to_vec(self).map_err(AuditError::Serialize)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

fn safe_identifier(value: &str, minimum: usize, maximum: usize) -> bool {
    (minimum..=maximum).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn safe_hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn safe_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'-'))
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("audit metadata is unsafe")]
    UnsafeMetadata,
    #[error("audit serialization failed")]
    Serialize(#[source] serde_json::Error),
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn event_is_value_free_by_construction() {
        let event = AuditEvent::new(
            "av_evt_12345678".to_string(),
            1,
            "av_session_12345678".to_string(),
            1,
            AuditDecision::Blocked,
            vec![AuditFinding {
                detector: DetectorId::GithubToken,
                data_class: DataClass::Credentials,
                confidence: Confidence::High,
                action: Action::Block,
                source_field: FieldClass::FunctionOutput,
                count: 1,
            }],
            UpstreamOutcome::NotStarted,
            2,
            "default".to_string(),
            "a".repeat(64),
            "0.1.0".to_string(),
        )
        .expect("audit fixture should validate");
        let serialized = String::from_utf8(event.to_json_line().expect("event should serialize"))
            .expect("audit should be UTF-8");
        for forbidden in ["fixture@example.test", "ghp_", "[AV_EMAIL_"] {
            assert!(!serialized.contains(forbidden));
        }
        assert!(serialized.ends_with('\n'));
    }

    #[test]
    fn unsafe_user_controlled_metadata_is_rejected() {
        let event = AuditEvent::new(
            "value@example.test".to_string(),
            1,
            "av_session_12345678".to_string(),
            1,
            AuditDecision::Rejected,
            Vec::new(),
            UpstreamOutcome::NotStarted,
            1,
            "default".to_string(),
            "a".repeat(64),
            "0.1.0".to_string(),
        );
        assert!(matches!(event, Err(AuditError::UnsafeMetadata)));
    }
}
