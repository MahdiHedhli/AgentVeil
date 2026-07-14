use serde::Serialize;

use crate::audit::{AuditDecision, AuditFinding, UpstreamOutcome};

pub const HTML: &str = include_str!("../assets/dashboard/index.html");
pub const CSS: &str = include_str!("../assets/dashboard/app.css");
pub const JS: &str = include_str!("../assets/dashboard/app.js");

#[derive(Clone, Debug, Serialize)]
pub struct DashboardActivity {
    pub request_sequence: u64,
    pub timestamp_unix_ms: u64,
    pub decision: AuditDecision,
    pub findings: Vec<AuditFinding>,
    pub upstream_outcome: UpstreamOutcome,
    pub scan_latency_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct DashboardSnapshot {
    pub schema_version: u8,
    pub protected: bool,
    pub binding: &'static str,
    pub transport: &'static str,
    pub upstream: &'static str,
    pub restoration: &'static str,
    pub audit_healthy: bool,
    pub session_pseudonym: String,
    pub policy_name: String,
    pub request_count: u64,
    pub wire_proof: &'static str,
    pub activity: Vec<DashboardActivity>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_assets_are_local_and_do_not_handle_secrets() {
        assert!(HTML.contains("/dashboard/app.css"));
        assert!(HTML.contains("/dashboard/app.js"));
        assert!(!HTML.contains("<script>"));
        assert!(!HTML.contains("style="));
        assert!(!HTML.contains("http://"));
        assert!(!HTML.contains("https://"));
        assert!(!CSS.contains("url("));
        assert!(!JS.contains("innerHTML"));
        assert!(!JS.contains("localStorage"));
        assert!(!JS.contains("sessionStorage"));
        assert!(!JS.contains("AGENTVEIL_SESSION_TOKEN"));
        assert!(HTML.contains("data-proof-scope=\"synthetic\""));
        assert!(HTML.contains("data-proof-scope=\"configuration\""));
        assert!(JS.contains("markStateUnavailable"));
        assert!(!JS.contains("document.querySelectorAll(\".check\")"));
    }

    #[test]
    fn dashboard_copy_states_the_boundary() {
        assert!(HTML.contains("AgentVeil"));
        assert!(JS.contains("Local Responses client"));
        assert!(JS.contains("OpenAI Responses"));
        assert!(HTML.contains("Synthetic wire proof"));
        assert!(HTML.contains("Live restoration is disabled"));
    }
}
