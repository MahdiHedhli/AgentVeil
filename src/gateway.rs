use std::collections::VecDeque;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_stream::stream;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::header::{AUTHORIZATION, CONTENT_ENCODING, CONTENT_TYPE, HOST};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::StreamExt;
use reqwest::Url;
use serde::Serialize;
use subtle::ConstantTimeEq;
use thiserror::Error;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

use crate::VERSION;
use crate::audit::{AuditDecision, AuditEvent, AuditFinding, UpstreamOutcome};
use crate::audit_sink::{AuditSink, AuditSinkError};
use crate::dashboard::{self, DashboardActivity, DashboardSnapshot};
use crate::engine::{
    EngineError, ForwardDecision, PrivacyEngine, ProtectionOutcome, ProtectionSummary,
};
use crate::ledger::SessionScope;
use crate::policy::ValidatedPolicy;
use crate::sse::{SseRestorationMode, SseTransformer};

const LOCAL_SESSION_HEADER: &str = "x-agentveil-session";
const CHATGPT_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const CHATGPT_MODELS_URL: &str = "https://chatgpt.com/backend-api/codex/models";
const API_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";
const API_MODELS_URL: &str = "https://api.openai.com/v1/models";
const DASHBOARD_ACTIVITY_LIMIT: usize = 64;

pub struct GatewayConfig {
    pub bind: SocketAddr,
    pub policy: ValidatedPolicy,
    pub scope: SessionScope,
    pub local_session_token: Zeroizing<String>,
    pub audit_path: PathBuf,
    pub upstream: UpstreamMode,
    pub client: GatewayClient,
    pub restoration_mode: RestorationMode,
    pub synthetic_wire_proof: SyntheticWireProof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GatewayClient {
    Generic,
    CodexCli,
}

impl GatewayClient {
    const fn label(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::CodexCli => "codex_cli",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestorationMode {
    Disabled,
    /// Synthetic harness compatibility mode. Restores every currently supported
    /// assistant display-text copy, including completed response snapshots.
    SyntheticDisplayText,
    /// Codex-safe synthetic mode. Restores only streamed assistant text deltas;
    /// completed response items remain tokenized for history and replay.
    SyntheticDisplayDeltaOnly,
}

impl RestorationMode {
    const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::SyntheticDisplayText => "synthetic_full",
            Self::SyntheticDisplayDeltaOnly => "synthetic_display_delta_only",
        }
    }

    const fn sse_mode(self) -> Option<SseRestorationMode> {
        match self {
            Self::Disabled => None,
            Self::SyntheticDisplayText => Some(SseRestorationMode::SyntheticDisplayText),
            Self::SyntheticDisplayDeltaOnly => Some(SseRestorationMode::SyntheticDisplayDeltaOnly),
        }
    }
}

#[derive(Clone)]
pub struct UpstreamMode(UpstreamKind);

#[derive(Clone)]
enum UpstreamKind {
    OpenAi,
    LoopbackTest(Url),
}

impl UpstreamMode {
    pub fn openai() -> Self {
        Self(UpstreamKind::OpenAi)
    }

    pub fn loopback_test(url: Url) -> Result<Self, GatewayError> {
        validate_loopback_test_url(&url)?;
        Ok(Self(UpstreamKind::LoopbackTest(url)))
    }

    fn validate(&self) -> Result<(), GatewayError> {
        match &self.0 {
            UpstreamKind::OpenAi => Ok(()),
            UpstreamKind::LoopbackTest(url) => validate_loopback_test_url(url),
        }
    }

    fn is_openai(&self) -> bool {
        matches!(self.0, UpstreamKind::OpenAi)
    }

    fn is_loopback_test(&self) -> bool {
        matches!(self.0, UpstreamKind::LoopbackTest(_))
    }

    pub(crate) fn label(&self) -> &'static str {
        match self.0 {
            UpstreamKind::OpenAi => "openai",
            UpstreamKind::LoopbackTest(_) => "loopback_test",
        }
    }
}

fn validate_loopback_test_url(url: &Url) -> Result<(), GatewayError> {
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(GatewayError::UnsafeTestUpstream);
    }
    let host = url.host_str().ok_or(GatewayError::UnsafeTestUpstream)?;
    let address: IpAddr = host.parse().map_err(|_| GatewayError::UnsafeTestUpstream)?;
    if !address.is_loopback() {
        return Err(GatewayError::UnsafeTestUpstream);
    }
    Ok(())
}

#[derive(Clone, Default)]
pub struct SyntheticWireProof {
    state: Arc<AtomicU8>,
}

impl SyntheticWireProof {
    const UNMEASURED: u8 = 0;
    const PASSED: u8 = 1;
    const FAILED: u8 = 2;

    pub fn unmeasured() -> Self {
        Self::default()
    }

    pub(crate) fn mark_passed(&self) {
        let _ = self.state.compare_exchange(
            Self::UNMEASURED,
            Self::PASSED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    pub(crate) fn mark_failed(&self) {
        self.state.store(Self::FAILED, Ordering::Release);
    }

    pub(crate) fn label(&self) -> &'static str {
        match self.state.load(Ordering::Acquire) {
            Self::UNMEASURED => "not_measured",
            Self::PASSED => "synthetic_passed",
            Self::FAILED => "synthetic_failed",
            _ => "synthetic_failed",
        }
    }
}

struct GatewayState {
    engine: Mutex<PrivacyEngine>,
    http_client: reqwest::Client,
    scope: SessionScope,
    expected_local_token: Zeroizing<String>,
    upstream: UpstreamMode,
    gateway_client: GatewayClient,
    restoration_mode: RestorationMode,
    audit: AuditSink,
    audit_healthy: AtomicBool,
    session_pseudonym: String,
    policy_name: String,
    policy_hash: String,
    started: Instant,
    request_sequence: AtomicU64,
    dashboard_activity: StdMutex<VecDeque<DashboardActivity>>,
    expected_dashboard_authority: String,
    synthetic_wire_proof: SyntheticWireProof,
}

pub async fn serve(mut config: GatewayConfig) -> Result<(), GatewayError> {
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|_| GatewayError::Bind)?;
    let bind = listener.local_addr().map_err(|_| GatewayError::Bind)?;
    config.bind = bind;
    let app = build_router(config)?;
    tracing::info!(
        address = %bind,
        transport = "responses_sse",
        "AgentVeil gateway ready"
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|_| GatewayError::Serve)
}

pub fn build_router(config: GatewayConfig) -> Result<Router, GatewayError> {
    if !config.bind.ip().is_loopback() {
        return Err(GatewayError::NonLoopbackBind);
    }
    if !(32..=128).contains(&config.local_session_token.len())
        || !config
            .local_session_token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err(GatewayError::InvalidLocalSessionToken);
    }
    config.upstream.validate()?;
    if config.restoration_mode != RestorationMode::Disabled && config.upstream.is_openai() {
        return Err(GatewayError::LiveRestorationNotVerified);
    }
    if config.policy.name() == "demo" && config.upstream.is_openai() {
        return Err(GatewayError::DemoPolicyRequiresTestUpstream);
    }

    let max_request_bytes = config.policy.defaults().max_request_bytes;
    let session_pseudonym = pseudonym(&config.scope);
    let policy_name = config.policy.name().to_string();
    let policy_hash = config.policy.hash().to_string();
    let expected_dashboard_authority = config.bind.to_string();
    let state = Arc::new(GatewayState {
        engine: Mutex::new(PrivacyEngine::new(config.policy)?),
        http_client: reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| GatewayError::HttpClient)?,
        scope: config.scope,
        expected_local_token: config.local_session_token,
        upstream: config.upstream,
        gateway_client: config.client,
        restoration_mode: config.restoration_mode,
        audit: AuditSink::open(&config.audit_path)?,
        audit_healthy: AtomicBool::new(true),
        session_pseudonym,
        policy_name,
        policy_hash,
        started: Instant::now(),
        request_sequence: AtomicU64::new(0),
        dashboard_activity: StdMutex::new(VecDeque::with_capacity(DASHBOARD_ACTIVITY_LIMIT)),
        expected_dashboard_authority,
        synthetic_wire_proof: config.synthetic_wire_proof,
    });
    Ok(Router::new()
        .route("/health", get(health))
        .route("/dashboard", get(dashboard_page))
        .route("/dashboard/app.css", get(dashboard_css))
        .route("/dashboard/app.js", get(dashboard_js))
        .route("/dashboard/state", get(dashboard_state))
        .route("/v1/models", get(models))
        .route("/v1/responses", post(responses))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(max_request_bytes))
        .with_state(state))
}

async fn dashboard_page(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if !dashboard_authority_allowed(&state, &headers) {
        return dashboard_authority_rejected();
    }
    dashboard_asset(dashboard::HTML, "text/html; charset=utf-8")
}

async fn dashboard_css(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if !dashboard_authority_allowed(&state, &headers) {
        return dashboard_authority_rejected();
    }
    dashboard_asset(dashboard::CSS, "text/css; charset=utf-8")
}

async fn dashboard_js(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if !dashboard_authority_allowed(&state, &headers) {
        return dashboard_authority_rejected();
    }
    dashboard_asset(dashboard::JS, "text/javascript; charset=utf-8")
}

async fn dashboard_state(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if !dashboard_authority_allowed(&state, &headers) {
        return dashboard_authority_rejected();
    }
    let activity = state
        .dashboard_activity
        .lock()
        .map(|activity| activity.iter().cloned().collect())
        .unwrap_or_default();
    let snapshot = DashboardSnapshot {
        schema_version: 2,
        protected: true,
        binding: "loopback",
        transport: "responses_sse",
        upstream: state.upstream.label(),
        client: state.gateway_client.label(),
        restoration: state.restoration_mode.label(),
        audit_healthy: state.audit_healthy.load(Ordering::Acquire),
        session_pseudonym: state.session_pseudonym.clone(),
        policy_name: state.policy_name.clone(),
        request_count: state.request_sequence.load(Ordering::Relaxed),
        wire_proof: state.synthetic_wire_proof.label(),
        activity,
    };
    let mut response = axum::Json(snapshot).into_response();
    apply_dashboard_headers(response.headers_mut());
    response
}

fn dashboard_authority_allowed(state: &GatewayState, headers: &HeaderMap) -> bool {
    let mut values = headers.get_all(HOST).iter();
    let Some(value) = values.next() else {
        return false;
    };
    values.next().is_none()
        && value
            .to_str()
            .is_ok_and(|value| value == state.expected_dashboard_authority)
}

fn dashboard_authority_rejected() -> Response {
    let mut response = Response::new(Body::from("AgentVeil rejected the dashboard authority."));
    *response.status_mut() = StatusCode::MISDIRECTED_REQUEST;
    apply_dashboard_headers(response.headers_mut());
    response
}

fn dashboard_asset(body: &'static str, content_type: &'static str) -> Response {
    let mut response = Response::new(Body::from(body));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    apply_dashboard_headers(response.headers_mut());
    response
}

fn apply_dashboard_headers(headers: &mut HeaderMap) {
    const SECURITY_HEADERS: [(&str, &str); 8] = [
        (
            "content-security-policy",
            "default-src 'none'; style-src 'self'; script-src 'self'; connect-src 'self'; img-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'",
        ),
        ("cache-control", "no-store, max-age=0"),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        (
            "permissions-policy",
            "camera=(), microphone=(), geolocation=()",
        ),
        ("cross-origin-opener-policy", "same-origin"),
        ("cross-origin-resource-policy", "same-origin"),
    ];
    for (name, value) in SECURITY_HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
}

async fn health(State(state): State<Arc<GatewayState>>) -> impl IntoResponse {
    axum::Json(HealthResponse {
        status: "ok",
        binding: "loopback",
        transport: "responses_sse",
        upstream: state.upstream.label(),
        restoration: state.restoration_mode.label(),
        audit_healthy: state.audit_healthy.load(Ordering::Relaxed),
    })
}

async fn models(State(state): State<Arc<GatewayState>>, headers: HeaderMap, uri: Uri) -> Response {
    if !authorized(&state, &headers) {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "agentveil_unauthorized",
            "AgentVeil rejected the local client.",
            None,
            None,
        );
    }
    if state.upstream.is_openai() && !has_single_authorization(&headers) {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "agentveil_upstream_auth_missing",
            "AgentVeil did not receive one valid Codex authorization header.",
            None,
            None,
        );
    }
    let query = match validated_models_query(uri.query()) {
        Ok(query) => query,
        Err(()) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "agentveil_invalid_models_query",
                "AgentVeil rejected an unsupported model-discovery query.",
                None,
                None,
            );
        }
    };
    let target = match upstream_url(&state.upstream, &headers, Route::Models, query) {
        Ok(target) => target,
        Err(_) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                "agentveil_upstream_configuration",
                "AgentVeil could not select the fixed upstream.",
                None,
                None,
            );
        }
    };
    let forwarded = forward_request_headers(&headers, &state.upstream, false);
    match state
        .http_client
        .get(target)
        .headers(forwarded)
        .timeout(Duration::from_secs(15))
        .send()
        .await
    {
        Ok(upstream) => relay_upstream(upstream, None),
        Err(_) => error_response(
            StatusCode::BAD_GATEWAY,
            "agentveil_upstream_unavailable",
            "AgentVeil could not reach model discovery.",
            None,
            None,
        ),
    }
}

async fn responses(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !authorized(&state, &headers) {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "agentveil_unauthorized",
            "AgentVeil rejected the local client.",
            None,
            None,
        );
    }
    if !state.audit_healthy.load(Ordering::Acquire) {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "agentveil_audit_unavailable",
            "AgentVeil failed closed because its audit sink is unavailable.",
            None,
            Some(0),
        );
    }
    if !content_type_is_json(&headers) || has_unsupported_content_encoding(&headers) {
        return error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "agentveil_unsupported_encoding",
            "AgentVeil requires uncompressed application/json requests.",
            None,
            None,
        );
    }
    if state.upstream.is_openai() && !has_single_authorization(&headers) {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "agentveil_upstream_auth_missing",
            "AgentVeil did not receive required Codex authentication.",
            None,
            None,
        );
    }

    let event_id = match random_identifier("av_evt_") {
        Ok(identifier) => identifier,
        Err(_) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "agentveil_random_unavailable",
                "AgentVeil failed closed because secure randomness is unavailable.",
                None,
                None,
            );
        }
    };
    let request_sequence = state.request_sequence.fetch_add(1, Ordering::Relaxed) + 1;
    let scan_started = Instant::now();
    let outcome = {
        let mut engine = state.engine.lock().await;
        engine.protect(&body, &state.scope, monotonic_ms(&state))
    };
    let scan_latency_ms = duration_ms(scan_started.elapsed());

    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            write_audit(
                &state,
                &event_id,
                request_sequence,
                AuditDecision::Rejected,
                None,
                UpstreamOutcome::NotStarted,
                scan_latency_ms,
            );
            let (status, code) = engine_error_status(&error);
            return error_response(
                status,
                code,
                "AgentVeil could not safely inspect this Codex request. No request body was sent upstream.",
                Some(&event_id),
                Some(0),
            );
        }
    };

    let forward = match outcome {
        ProtectionOutcome::Blocked(blocked) => {
            write_audit(
                &state,
                &event_id,
                request_sequence,
                AuditDecision::Blocked,
                Some(&blocked.summary),
                UpstreamOutcome::NotStarted,
                scan_latency_ms,
            );
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "agentveil_blocked",
                "AgentVeil blocked this Codex request before it reached the remote model. Remove the credential or replace it with a safe reference.",
                Some(&event_id),
                Some(0),
            );
        }
        ProtectionOutcome::Forward(forward) => forward,
    };

    let target = match upstream_url(&state.upstream, &headers, Route::Responses, None) {
        Ok(target) => target,
        Err(_) => {
            write_audit(
                &state,
                &event_id,
                request_sequence,
                audit_decision(forward.decision),
                Some(&forward.summary),
                UpstreamOutcome::NotStarted,
                scan_latency_ms,
            );
            return error_response(
                StatusCode::BAD_GATEWAY,
                "agentveil_upstream_configuration",
                "AgentVeil could not select the fixed upstream. No request body was sent upstream.",
                Some(&event_id),
                Some(0),
            );
        }
    };
    let forwarded_headers = forward_request_headers(&headers, &state.upstream, true);
    let decision = forward.decision;
    let summary = forward.summary.clone();
    let forward_body = forward.into_body();
    if !write_audit(
        &state,
        &event_id,
        request_sequence,
        audit_decision(decision),
        Some(&summary),
        UpstreamOutcome::Pending,
        scan_latency_ms,
    ) {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "agentveil_audit_unavailable",
            "AgentVeil failed closed because it could not record the protected request.",
            Some(&event_id),
            Some(0),
        );
    }
    let upstream = state
        .http_client
        .post(target)
        .headers(forwarded_headers)
        .body(forward_body)
        .send()
        .await;
    match upstream {
        Ok(upstream) => {
            write_audit(
                &state,
                &event_id,
                request_sequence,
                audit_decision(decision),
                Some(&summary),
                UpstreamOutcome::Started,
                scan_latency_ms,
            );
            relay_upstream(upstream, Some(state))
        }
        Err(_) => {
            write_audit(
                &state,
                &event_id,
                request_sequence,
                audit_decision(decision),
                Some(&summary),
                UpstreamOutcome::Failed,
                scan_latency_ms,
            );
            error_response(
                StatusCode::BAD_GATEWAY,
                "agentveil_upstream_unavailable",
                "AgentVeil could not reach the fixed upstream.",
                Some(&event_id),
                None,
            )
        }
    }
}

fn relay_upstream(upstream: reqwest::Response, state: Option<Arc<GatewayState>>) -> Response {
    let status = upstream.status();
    let response_headers = response_headers(upstream.headers());
    let is_sse = upstream
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("text/event-stream"));
    let restoration_mode = if is_sse {
        state
            .as_ref()
            .and_then(|state| state.restoration_mode.sse_mode())
    } else {
        None
    };
    let source = upstream.bytes_stream();
    let body = if let (Some(restoration_mode), Some(state)) = (restoration_mode, state) {
        let stream = stream! {
            let mut source = source;
            let mut transformer = SseTransformer::new(restoration_mode);
            while let Some(chunk) = source.next().await {
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(error) => {
                        yield Err::<Bytes, io::Error>(io::Error::other(error));
                        return;
                    }
                };
                let now_ms = monotonic_ms(&state);
                let frames_result = {
                    let mut engine = state.engine.lock().await;
                    transformer.push(&chunk, |text| {
                        engine.restore_display_text(&state.scope, text, now_ms)
                    })
                };
                let frames = match frames_result {
                    Ok(frames) => frames,
                    Err(error) => {
                        yield Err::<Bytes, io::Error>(io::Error::other(error));
                        return;
                    }
                };
                for frame in frames {
                    yield Ok::<Bytes, io::Error>(frame);
                }
            }
            if let Err(error) = transformer.finish() {
                yield Err::<Bytes, io::Error>(io::Error::other(error));
            }
        };
        Body::from_stream(stream)
    } else {
        Body::from_stream(source.map(|result| result.map_err(io::Error::other)))
    };
    let mut response = Response::builder()
        .status(status)
        .body(body)
        .unwrap_or_else(|_| {
            error_response(
                StatusCode::BAD_GATEWAY,
                "agentveil_response_build",
                "AgentVeil could not construct the local response.",
                None,
                None,
            )
        });
    *response.headers_mut() = response_headers;
    response
}

fn write_audit(
    state: &GatewayState,
    event_id: &str,
    request_sequence: u64,
    decision: AuditDecision,
    summary: Option<&ProtectionSummary>,
    upstream_outcome: UpstreamOutcome,
    scan_latency_ms: u64,
) -> bool {
    let findings = summary.map_or_else(Vec::new, |summary| {
        summary
            .findings
            .iter()
            .map(|finding| AuditFinding {
                detector: finding.detector,
                data_class: finding.data_class,
                confidence: finding.confidence,
                action: finding.action,
                source_field: finding.source_field,
                count: finding.count,
            })
            .collect()
    });
    let event = AuditEvent::new(
        event_id.to_string(),
        unix_ms(),
        state.session_pseudonym.clone(),
        request_sequence,
        decision,
        findings,
        upstream_outcome,
        scan_latency_ms,
        state.policy_name.clone(),
        state.policy_hash.clone(),
        VERSION.to_string(),
    );
    let event = match event {
        Ok(event) => event,
        Err(_) => {
            state.audit_healthy.store(false, Ordering::Relaxed);
            tracing::warn!("AgentVeil audit metadata validation failed");
            return false;
        }
    };
    if state.audit.write(&event).is_err() {
        state.audit_healthy.store(false, Ordering::Relaxed);
        tracing::warn!("AgentVeil audit sink is degraded");
        return false;
    }
    record_dashboard_activity(state, &event);
    true
}

fn record_dashboard_activity(state: &GatewayState, event: &AuditEvent) {
    let Ok(mut activity) = state.dashboard_activity.lock() else {
        return;
    };
    if let Some(existing) = activity
        .iter_mut()
        .find(|item| item.request_sequence == event.request_sequence)
    {
        existing.upstream_outcome = event.upstream_outcome;
        existing.scan_latency_ms = event.scan_latency_ms;
        return;
    }
    activity.push_front(DashboardActivity {
        request_sequence: event.request_sequence,
        timestamp_unix_ms: event.timestamp_unix_ms,
        decision: event.decision,
        findings: event.findings.clone(),
        upstream_outcome: event.upstream_outcome,
        scan_latency_ms: event.scan_latency_ms,
    });
    activity.truncate(DASHBOARD_ACTIVITY_LIMIT);
}

fn audit_decision(decision: ForwardDecision) -> AuditDecision {
    match decision {
        ForwardDecision::Allowed => AuditDecision::Allowed,
        ForwardDecision::Rewritten => AuditDecision::Rewritten,
    }
}

fn authorized(state: &GatewayState, headers: &HeaderMap) -> bool {
    let Some(candidate) = headers
        .get(LOCAL_SESSION_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    if candidate.len() != state.expected_local_token.len() {
        return false;
    }
    bool::from(
        candidate
            .as_bytes()
            .ct_eq(state.expected_local_token.as_bytes()),
    )
}

fn has_single_authorization(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let Some(value) = values.next() else {
        return false;
    };
    values.next().is_none()
        && value
            .to_str()
            .ok()
            .is_some_and(|value| value.starts_with("Bearer ") && value.len() > "Bearer ".len())
}

fn content_type_is_json(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value.split(';').next().is_some_and(|media_type| {
                media_type.trim().eq_ignore_ascii_case("application/json")
            })
        })
}

fn has_unsupported_content_encoding(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| !value.eq_ignore_ascii_case("identity"))
}

fn forward_request_headers(
    source: &HeaderMap,
    upstream: &UpstreamMode,
    include_content_type: bool,
) -> HeaderMap {
    if upstream.is_loopback_test() {
        let mut headers = HeaderMap::new();
        if include_content_type {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }
        headers.insert(
            HeaderName::from_static("x-agentveil-synthetic"),
            HeaderValue::from_static("true"),
        );
        return headers;
    }
    let mut forwarded = HeaderMap::new();
    for (name, value) in source {
        let name_string = name.as_str();
        let allowed = matches!(
            name_string,
            "accept"
                | "authorization"
                | "chatgpt-account-id"
                | "content-type"
                | "openai-beta"
                | "openai-organization"
                | "openai-project"
                | "originator"
                | "session-id"
                | "thread-id"
                | "user-agent"
                | "x-client-request-id"
                | "x-codex-beta-features"
                | "x-codex-turn-metadata"
                | "x-codex-window-id"
                | "x-openai-internal-codex-responses-lite"
        );
        if allowed && (include_content_type || name_string != "content-type") {
            forwarded.append(name.clone(), value.clone());
        }
    }
    forwarded
}

fn response_headers(source: &HeaderMap) -> HeaderMap {
    let mut forwarded = HeaderMap::new();
    for (name, value) in source {
        let name_string = name.as_str();
        let allowed = matches!(
            name_string,
            "cache-control" | "content-type" | "retry-after" | "x-request-id" | "request-id"
        ) || name_string.starts_with("x-ratelimit-")
            || name_string.starts_with("openai-");
        if allowed {
            forwarded.append(name.clone(), value.clone());
        }
    }
    forwarded
}

#[derive(Clone, Copy)]
enum Route {
    Responses,
    Models,
}

fn upstream_url(
    upstream: &UpstreamMode,
    headers: &HeaderMap,
    route: Route,
    query: Option<&str>,
) -> Result<Url, GatewayError> {
    let mut url = match &upstream.0 {
        UpstreamKind::LoopbackTest(base) => base
            .join(match route {
                Route::Responses => "/v1/responses",
                Route::Models => "/v1/models",
            })
            .map_err(|_| GatewayError::UpstreamUrl)?,
        UpstreamKind::OpenAi => {
            let chatgpt = headers.contains_key("chatgpt-account-id");
            Url::parse(match (route, chatgpt) {
                (Route::Responses, true) => CHATGPT_RESPONSES_URL,
                (Route::Models, true) => CHATGPT_MODELS_URL,
                (Route::Responses, false) => API_RESPONSES_URL,
                (Route::Models, false) => API_MODELS_URL,
            })
            .map_err(|_| GatewayError::UpstreamUrl)?
        }
    };
    if let Some(query) = query {
        url.set_query(Some(query));
    }
    Ok(url)
}

fn validated_models_query(query: Option<&str>) -> Result<Option<&str>, ()> {
    let Some(query) = query else {
        return Ok(None);
    };
    let Some(value) = query.strip_prefix("client_version=") else {
        return Err(());
    };
    if value.is_empty()
        || value.len() > 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(());
    }
    Ok(Some(query))
}

fn engine_error_status(error: &EngineError) -> (StatusCode, &'static str) {
    match error {
        EngineError::Oversize => (StatusCode::PAYLOAD_TOO_LARGE, "agentveil_oversize"),
        EngineError::Payload(crate::payload::PayloadError::Json(_)) => {
            (StatusCode::BAD_REQUEST, "agentveil_invalid_json")
        }
        EngineError::Payload(_) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "agentveil_unsupported_payload",
        ),
        EngineError::Detector(_)
        | EngineError::Ledger(_)
        | EngineError::InvalidRewriteSpan
        | EngineError::BlockedReachedRewrite => (
            StatusCode::SERVICE_UNAVAILABLE,
            "agentveil_protection_failure",
        ),
    }
}

fn error_response(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    event_id: Option<&str>,
    forwarded_bytes: Option<u64>,
) -> Response {
    (
        status,
        axum::Json(ErrorEnvelope {
            error: ErrorBody {
                code,
                message,
                event_id,
            },
            agentveil: forwarded_bytes.map(|protected_request_bytes_forwarded| EnforcementProof {
                protected_request_bytes_forwarded,
            }),
        }),
    )
        .into_response()
}

async fn not_found() -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        "agentveil_route_not_found",
        "AgentVeil exposes only the verified Codex routes.",
        None,
        None,
    )
}

fn random_identifier(prefix: &str) -> Result<String, GatewayError> {
    let mut bytes = [0_u8; 12];
    getrandom::fill(&mut bytes).map_err(|_| GatewayError::RandomUnavailable)?;
    let mut identifier = String::with_capacity(prefix.len() + bytes.len() * 2);
    identifier.push_str(prefix);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut identifier, "{byte:02x}").map_err(|_| GatewayError::Identifier)?;
    }
    Ok(identifier)
}

fn pseudonym(scope: &SessionScope) -> String {
    let hash = blake3::hash(scope.as_str().as_bytes()).to_hex().to_string();
    format!("av_session_{}", &hash[..16])
}

fn monotonic_ms(state: &GatewayState) -> u64 {
    duration_ms(state.started.elapsed())
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(duration_ms)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::SyntheticWireProof;

    #[test]
    fn synthetic_wire_proof_failure_has_sticky_precedence() {
        let proof = SyntheticWireProof::unmeasured();
        assert_eq!(proof.label(), "not_measured");

        proof.mark_passed();
        assert_eq!(proof.label(), "synthetic_passed");

        proof.mark_failed();
        assert_eq!(proof.label(), "synthetic_failed");

        proof.mark_passed();
        assert_eq!(proof.label(), "synthetic_failed");
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    binding: &'static str,
    transport: &'static str,
    upstream: &'static str,
    restoration: &'static str,
    audit_healthy: bool,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agentveil: Option<EnforcementProof>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'static str,
    message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<&'a str>,
}

#[derive(Serialize)]
struct EnforcementProof {
    protected_request_bytes_forwarded: u64,
}

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("gateway must bind to loopback")]
    NonLoopbackBind,
    #[error("local session token is invalid")]
    InvalidLocalSessionToken,
    #[error("test upstream must be an unauthenticated loopback HTTP URL")]
    UnsafeTestUpstream,
    #[error("live restoration is not verified and remains disabled")]
    LiveRestorationNotVerified,
    #[error("the demo policy is restricted to a loopback synthetic upstream")]
    DemoPolicyRequiresTestUpstream,
    #[error("HTTP client initialization failed")]
    HttpClient,
    #[error("gateway bind failed")]
    Bind,
    #[error("gateway server failed")]
    Serve,
    #[error("upstream URL construction failed")]
    UpstreamUrl,
    #[error("secure randomness is unavailable")]
    RandomUnavailable,
    #[error("safe identifier construction failed")]
    Identifier,
    #[error("privacy engine initialization failed")]
    Engine(#[from] EngineError),
    #[error("audit sink initialization failed")]
    Audit(#[from] AuditSinkError),
}
