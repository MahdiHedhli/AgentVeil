use regex::Regex;
use serde::Serialize;
use thiserror::Error;

use crate::detector::{DetectorError, Scanner, resolve_overlaps};
use crate::domain::{Action, Confidence, DataClass, DetectorId, FieldClass, Finding};
use crate::ledger::{LedgerError, OsRandom, RandomSource, SessionScope, Token, TokenLedger};
use crate::payload::{PayloadError, RewriteMode, ValidatedRequest};
use crate::policy::{DEFAULT_RESTORATION_TTL_SECONDS, ValidatedPolicy};

pub struct PrivacyEngine<R = OsRandom> {
    policy: ValidatedPolicy,
    scanner: Scanner,
    ledger: TokenLedger<R>,
    issued_token_pattern: Regex,
}

impl PrivacyEngine<OsRandom> {
    pub fn new(policy: ValidatedPolicy) -> Result<Self, EngineError> {
        let capacity = policy.defaults().ledger_max_entries;
        Self::with_random(policy, OsRandom, capacity)
    }
}

impl<R> PrivacyEngine<R>
where
    R: RandomSource,
{
    pub fn with_random(
        policy: ValidatedPolicy,
        random: R,
        ledger_capacity: usize,
    ) -> Result<Self, EngineError> {
        let scanner = Scanner::new(policy.detectors())?;
        let issued_token_pattern =
            Regex::new(r"\[AV_(?:EMAIL|PHONE|IPV4|HOST|PATH|TERM)_[A-F0-9]{32}\]")
                .map_err(DetectorError::Pattern)?;
        Ok(Self {
            policy,
            scanner,
            ledger: TokenLedger::with_random(ledger_capacity, random)?,
            issued_token_pattern,
        })
    }

    pub fn policy(&self) -> &ValidatedPolicy {
        &self.policy
    }

    pub fn protect(
        &mut self,
        body: &[u8],
        scope: &SessionScope,
        now_ms: u64,
    ) -> Result<ProtectionOutcome, EngineError> {
        if body.len() > self.policy.defaults().max_request_bytes {
            return Err(EngineError::Oversize);
        }
        let mut request = ValidatedRequest::parse(body)?;
        let mut plans = Vec::new();

        for (target_index, target) in request.targets().iter().enumerate() {
            let text = request.text(target)?.to_string();
            let mut findings_with_actions = Vec::new();
            for finding in self.scanner.scan(&text)? {
                let mut action = self.policy.rule_for(finding.data_class).action;
                if finding.data_class == DataClass::TokenNamespace {
                    let candidate = text
                        .get(finding.source_span.clone())
                        .ok_or(EngineError::InvalidRewriteSpan)?;
                    if self.ledger.owns_token(scope, candidate, now_ms) {
                        action = Action::Allow;
                    }
                }
                if target.mode == RewriteMode::BlockOnly && action != Action::Allow {
                    action = Action::Block;
                }
                findings_with_actions.push((finding, action));
            }
            let findings = resolve_overlaps(findings_with_actions);
            plans.push(TargetPlan {
                target_index,
                field_class: target.field_class,
                original: text,
                findings,
            });
        }

        let summary = summarize(&plans);
        if summary
            .findings
            .iter()
            .any(|finding| finding.action == Action::Block)
        {
            return Ok(ProtectionOutcome::Blocked(BlockedRequest { summary }));
        }

        let mut created_tokens = Vec::new();
        let rewrite_result =
            self.apply_rewrites(&mut request, &plans, scope, now_ms, &mut created_tokens);
        if let Err(error) = rewrite_result {
            self.ledger.rollback_created(scope, &created_tokens);
            return Err(error);
        }
        let serialized = match request.serialize() {
            Ok(serialized) => serialized,
            Err(error) => {
                self.ledger.rollback_created(scope, &created_tokens);
                return Err(error.into());
            }
        };
        let decision = if summary
            .findings
            .iter()
            .any(|finding| matches!(finding.action, Action::Mask | Action::Tokenize))
        {
            ForwardDecision::Rewritten
        } else {
            ForwardDecision::Allowed
        };
        Ok(ProtectionOutcome::Forward(ForwardRequest {
            body: serialized,
            decision,
            summary,
        }))
    }

    fn apply_rewrites(
        &mut self,
        request: &mut ValidatedRequest,
        plans: &[TargetPlan],
        scope: &SessionScope,
        now_ms: u64,
        created_tokens: &mut Vec<Token>,
    ) -> Result<(), EngineError> {
        for plan in plans {
            let mut rewritten = plan.original.clone();
            let mut findings = plan.findings.clone();
            findings.sort_by_key(|(finding, _)| std::cmp::Reverse(finding.source_span.start));
            for (finding, action) in findings {
                if action == Action::Allow {
                    continue;
                }
                if !rewritten.is_char_boundary(finding.source_span.start)
                    || !rewritten.is_char_boundary(finding.source_span.end)
                {
                    return Err(EngineError::InvalidRewriteSpan);
                }
                let original = plan
                    .original
                    .get(finding.source_span.clone())
                    .ok_or(EngineError::InvalidRewriteSpan)?;
                let replacement = match action {
                    Action::Block => return Err(EngineError::BlockedReachedRewrite),
                    Action::Mask => mask(finding.data_class),
                    Action::Tokenize => {
                        let rule = self.policy.rule_for(finding.data_class);
                        let ttl = rule.ttl_seconds.unwrap_or(DEFAULT_RESTORATION_TTL_SECONDS);
                        let issued = self.ledger.issue_tracked(
                            scope,
                            finding.data_class,
                            original,
                            now_ms,
                            ttl,
                        )?;
                        if issued.created {
                            created_tokens.push(issued.token.clone());
                        }
                        issued.token.as_str().to_string()
                    }
                    Action::Allow => String::new(),
                };
                rewritten.replace_range(finding.source_span, &replacement);
            }
            request.replace_text(plan.target_index, rewritten)?;
        }
        Ok(())
    }

    pub fn restore_display_text(
        &mut self,
        scope: &SessionScope,
        text: &str,
        now_ms: u64,
    ) -> String {
        let candidates: Vec<_> = self
            .issued_token_pattern
            .find_iter(text)
            .map(|candidate| (candidate.range(), candidate.as_str().to_string()))
            .collect();
        let mut restored = text.to_string();
        for (range, token) in candidates.into_iter().rev() {
            if let Some(value) = self.ledger.restore_exact(scope, &token, now_ms) {
                restored.replace_range(range, value.as_str());
            }
        }
        restored
    }

    pub fn clear_scope(&mut self, scope: &SessionScope) -> usize {
        self.ledger.clear_scope(scope)
    }

    pub fn ledger_entry_count(&mut self, now_ms: u64) -> usize {
        self.ledger.entry_count(now_ms)
    }
}

#[derive(Clone)]
struct TargetPlan {
    target_index: usize,
    field_class: FieldClass,
    original: String,
    findings: Vec<(Finding, Action)>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FindingSummary {
    pub detector: DetectorId,
    pub data_class: DataClass,
    pub confidence: Confidence,
    pub action: Action,
    pub source_field: FieldClass,
    pub count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProtectionSummary {
    pub scanned_fields: u32,
    pub findings: Vec<FindingSummary>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ForwardDecision {
    Allowed,
    Rewritten,
}

#[derive(Clone, Debug)]
pub struct ForwardRequest {
    body: Vec<u8>,
    pub decision: ForwardDecision,
    pub summary: ProtectionSummary,
}

impl ForwardRequest {
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn into_body(self) -> Vec<u8> {
        self.body
    }
}

#[derive(Clone, Debug)]
pub struct BlockedRequest {
    pub summary: ProtectionSummary,
}

#[derive(Clone, Debug)]
pub enum ProtectionOutcome {
    Forward(ForwardRequest),
    Blocked(BlockedRequest),
}

fn summarize(plans: &[TargetPlan]) -> ProtectionSummary {
    let mut summaries: Vec<FindingSummary> = Vec::new();
    for plan in plans {
        for (finding, action) in &plan.findings {
            if let Some(summary) = summaries.iter_mut().find(|summary| {
                summary.detector == finding.detector
                    && summary.data_class == finding.data_class
                    && summary.confidence == finding.confidence
                    && summary.action == *action
                    && summary.source_field == plan.field_class
            }) {
                summary.count = summary.count.saturating_add(1);
            } else {
                summaries.push(FindingSummary {
                    detector: finding.detector,
                    data_class: finding.data_class,
                    confidence: finding.confidence,
                    action: *action,
                    source_field: plan.field_class,
                    count: 1,
                });
            }
        }
    }
    summaries.sort_by_key(|summary| (summary.data_class, summary.detector, summary.source_field));
    ProtectionSummary {
        scanned_fields: u32::try_from(plans.len()).unwrap_or(u32::MAX),
        findings: summaries,
    }
}

fn mask(data_class: DataClass) -> String {
    format!("[REDACTED_{}]", data_class.token_label())
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("request exceeds the configured size limit")]
    Oversize,
    #[error("request payload cannot be protected")]
    Payload(#[from] PayloadError),
    #[error("sensitive-data detection failed closed")]
    Detector(#[from] DetectorError),
    #[error("token ledger operation failed closed")]
    Ledger(#[from] LedgerError),
    #[error("normalization produced an invalid rewrite span")]
    InvalidRewriteSpan,
    #[error("blocked finding reached the rewrite stage")]
    BlockedReachedRewrite,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::ledger::RandomSource;

    struct CounterRandom(u8);

    impl RandomSource for CounterRandom {
        fn fill(&mut self, destination: &mut [u8]) -> Result<(), LedgerError> {
            self.0 = self.0.wrapping_add(1);
            destination.fill(self.0);
            Ok(())
        }
    }

    fn engine() -> PrivacyEngine<CounterRandom> {
        let policy = ValidatedPolicy::from_yaml(include_bytes!("../policies/demo.yaml"))
            .expect("demo policy should validate");
        PrivacyEngine::with_random(policy, CounterRandom(0), 32).expect("engine should initialize")
    }

    fn scope() -> SessionScope {
        SessionScope::parse("scope_demo_123456").expect("scope should validate")
    }

    fn request(text: &str, item_type: &str) -> Vec<u8> {
        let input = if item_type == "message" {
            json!([{"type":"message","role":"user","content":[{"type":"input_text","text":text}]}])
        } else {
            json!([{"type":"function_call_output","call_id":"call_safe","output":text}])
        };
        serde_json::to_vec(&json!({
            "model":"gpt-5.6-luna",
            "instructions":"Protect the user",
            "input": input,
            "tools":[],
            "tool_choice":"auto",
            "parallel_tool_calls":true,
            "reasoning":{"effort":"medium"},
            "store":false,
            "stream":true,
            "include":[]
        }))
        .expect("fixture should serialize")
    }

    #[test]
    fn tokenizes_then_restores_email_and_private_ip() {
        let raw = "Contact ava.agentveil@example.test on 10.24.8.15";
        let mut engine = engine();
        let outcome = engine
            .protect(&request(raw, "message"), &scope(), 0)
            .expect("request should protect");
        let ProtectionOutcome::Forward(forward) = outcome else {
            panic!("lower-risk data should not block");
        };
        let body = String::from_utf8(forward.body().to_vec()).expect("body should be UTF-8");
        assert!(!body.contains("ava.agentveil@example.test"));
        assert!(!body.contains("10.24.8.15"));
        assert!(body.contains("[AV_EMAIL_"));
        assert!(body.contains("[AV_IPV4_"));
        let value: Value = serde_json::from_str(&body).expect("body should parse");
        let protected = value["input"][0]["content"][0]["text"]
            .as_str()
            .expect("protected text should exist");
        assert_eq!(engine.restore_display_text(&scope(), protected, 1), raw);
    }

    #[test]
    fn blocks_secret_before_allocating_any_token() {
        let secret = format!("{}{}", "ghp_", "A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5");
        let text = format!("email ava.agentveil@example.test token {secret}");
        let mut engine = engine();
        let outcome = engine
            .protect(&request(&text, "message"), &scope(), 0)
            .expect("block is an enforcement outcome");
        assert!(matches!(outcome, ProtectionOutcome::Blocked(_)));
        assert_eq!(engine.ledger_entry_count(0), 0);
    }

    #[test]
    fn protects_tool_output_before_next_model_turn() {
        let raw = "tool printed ava.agentveil@example.test and 10.24.8.15";
        let mut engine = engine();
        let outcome = engine
            .protect(&request(raw, "function_call_output"), &scope(), 0)
            .expect("tool output should protect");
        let ProtectionOutcome::Forward(forward) = outcome else {
            panic!("lower-risk tool data should not block");
        };
        let body = String::from_utf8(forward.into_body()).expect("body should be UTF-8");
        assert!(!body.contains("ava.agentveil@example.test"));
        assert!(!body.contains("10.24.8.15"));
    }

    #[test]
    fn unknown_agentveil_token_is_blocked_but_owned_token_can_replay() {
        let mut engine = engine();
        let unknown = "[AV_EMAIL_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA]";
        assert!(matches!(
            engine
                .protect(&request(unknown, "message"), &scope(), 0)
                .expect("unknown token should produce enforcement"),
            ProtectionOutcome::Blocked(_)
        ));

        let first = engine
            .protect(
                &request("ava.agentveil@example.test", "message"),
                &scope(),
                0,
            )
            .expect("email should tokenize");
        let ProtectionOutcome::Forward(first) = first else {
            panic!("email should forward");
        };
        let value: Value = serde_json::from_slice(first.body()).expect("body should parse");
        let token = value["input"][0]["content"][0]["text"]
            .as_str()
            .expect("token should exist");
        assert!(matches!(
            engine
                .protect(&request(token, "message"), &scope(), 1)
                .expect("owned token should be admitted"),
            ProtectionOutcome::Forward(_)
        ));
    }
}
