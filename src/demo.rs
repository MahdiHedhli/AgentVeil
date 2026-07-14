use std::collections::BTreeSet;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use reqwest::Url;
use serde_json::{Value, json};
use thiserror::Error;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;
use zeroize::Zeroizing;

use crate::gateway::{GatewayConfig, GatewayError, SyntheticWireProof, UpstreamMode, build_router};
use crate::ledger::{LedgerError, SessionScope};
use crate::policy::{PolicyError, ValidatedPolicy};

const LOCAL_SESSION_HEADER: &str = "x-agentveil-session";
const SYNTHETIC_EMAIL: &str = "ava.agentveil@example.test";
const SYNTHETIC_PRIVATE_IP: &str = "10.24.8.15";
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const SERVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

struct FakeUpstreamState {
    request_count: AtomicUsize,
    authorization_seen: AtomicBool,
    forbidden_header_seen: AtomicBool,
    forbidden_header_hashes: Vec<blake3::Hash>,
    bodies: Mutex<Vec<Vec<u8>>>,
}

impl FakeUpstreamState {
    fn new(forbidden_header_values: &[&[u8]]) -> Self {
        Self {
            request_count: AtomicUsize::new(0),
            authorization_seen: AtomicBool::new(false),
            forbidden_header_seen: AtomicBool::new(false),
            forbidden_header_hashes: forbidden_header_values
                .iter()
                .map(|value| blake3::hash(value))
                .collect(),
            bodies: Mutex::new(Vec::new()),
        }
    }

    fn observe_headers(&self, headers: &HeaderMap) {
        let authorization_seen = headers.contains_key("authorization");
        self.authorization_seen
            .fetch_or(authorization_seen, Ordering::SeqCst);
        let forbidden_value_seen = headers.values().any(|value| {
            self.forbidden_header_hashes
                .contains(&blake3::hash(value.as_bytes()))
        });
        self.forbidden_header_seen.fetch_or(
            headers.contains_key(LOCAL_SESSION_HEADER) || forbidden_value_seen,
            Ordering::SeqCst,
        );
    }
}

pub async fn run(check_only: bool) -> Result<(), DemoError> {
    let runtime = RuntimeGuard::new(private_runtime_path()?);
    let audit_path = runtime.audit_path().to_path_buf();
    let local_session_token = random_hex(32)?;
    let synthetic_authorization =
        Zeroizing::new(format!("{}{}", "Bearer ", "SYNTHETIC_DEMO_NOT_REUSABLE"));
    let scope = SessionScope::parse(format!("avscope_{}", random_hex(16)?.as_str()))?;
    let policy = ValidatedPolicy::from_yaml(include_bytes!("../policies/demo.yaml"))?;

    let fake_state = Arc::new(FakeUpstreamState::new(&[
        local_session_token.as_bytes(),
        synthetic_authorization.as_bytes(),
    ]));
    let fake_app = Router::new()
        .route("/v1/responses", post(fake_responses))
        .with_state(fake_state.clone());
    let fake_listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|_| DemoError::LoopbackBind)?;
    let fake_address = fake_listener
        .local_addr()
        .map_err(|_| DemoError::LoopbackBind)?;
    let gateway_listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|_| DemoError::LoopbackBind)?;
    let gateway_address = gateway_listener
        .local_addr()
        .map_err(|_| DemoError::LoopbackBind)?;
    let fake_url =
        Url::parse(&format!("http://{fake_address}")).map_err(|_| DemoError::LoopbackUrl)?;
    let synthetic_wire_proof = SyntheticWireProof::unmeasured();
    let gateway_app = build_router(GatewayConfig {
        bind: gateway_address,
        policy,
        scope,
        local_session_token: local_session_token.clone(),
        audit_path: audit_path.clone(),
        upstream: UpstreamMode::loopback_test(fake_url)?,
        restore_display_text: false,
        synthetic_wire_proof: synthetic_wire_proof.clone(),
    });
    let gateway_app = match gateway_app {
        Ok(app) => app,
        Err(error) => return Err(error.into()),
    };

    let (fake_shutdown_tx, fake_shutdown_rx) = oneshot::channel::<()>();
    let mut fake_task = tokio::spawn(async move {
        axum::serve(fake_listener, fake_app)
            .with_graceful_shutdown(async {
                let _ = fake_shutdown_rx.await;
            })
            .await
    });
    let (gateway_shutdown_tx, gateway_shutdown_rx) = oneshot::channel::<()>();
    let mut gateway_task = tokio::spawn(async move {
        axum::serve(gateway_listener, gateway_app)
            .with_graceful_shutdown(async {
                let _ = gateway_shutdown_rx.await;
            })
            .await
    });

    let check_result = tokio::time::timeout(
        CHECK_TIMEOUT,
        run_checks(
            gateway_address,
            local_session_token.as_str(),
            synthetic_authorization.as_str(),
            &fake_state,
            &audit_path,
            &synthetic_wire_proof,
        ),
    )
    .await
    .unwrap_or(Err(DemoError::Check("whole_run_timeout")));
    if let Err(error) = check_result {
        let _ = gateway_shutdown_tx.send(());
        let _ = fake_shutdown_tx.send(());
        let _ = stop_server(&mut gateway_task).await;
        let _ = stop_server(&mut fake_task).await;
        return Err(error);
    }

    if check_only {
        let _ = gateway_shutdown_tx.send(());
        let _ = fake_shutdown_tx.send(());
        let gateway_result = stop_server(&mut gateway_task).await;
        let fake_result = stop_server(&mut fake_task).await;
        gateway_result.and(fake_result)?;
        println!(
            "demo-check: status=pass route=synthetic_loopback allow=pass tokenize=pass zero_connect=pass tool_reentry=pass dashboard=pass audit=pass output=value_free"
        );
        return Ok(());
    }

    println!(
        "AgentVeil demo ready: status=pass route=synthetic_loopback requests=4 hard_blocks=1 dashboard=http://{gateway_address}/dashboard"
    );
    enum Stop {
        Signal(Result<(), io::Error>),
        Gateway(Result<Result<(), io::Error>, tokio::task::JoinError>),
        Fake(Result<Result<(), io::Error>, tokio::task::JoinError>),
    }
    let stop = tokio::select! {
        signal = tokio::signal::ctrl_c() => Stop::Signal(signal),
        result = &mut gateway_task => Stop::Gateway(result),
        result = &mut fake_task => Stop::Fake(result),
    };
    match stop {
        Stop::Signal(signal) => {
            let signal_result = signal.map_err(|_| DemoError::Signal);
            let _ = gateway_shutdown_tx.send(());
            let _ = fake_shutdown_tx.send(());
            let gateway_result = stop_server(&mut gateway_task).await;
            let fake_result = stop_server(&mut fake_task).await;
            signal_result.and(gateway_result).and(fake_result)
        }
        Stop::Gateway(result) => {
            let _ = fake_shutdown_tx.send(());
            let fake_result = stop_server(&mut fake_task).await;
            match validate_server_result(result) {
                Err(error) => Err(error),
                Ok(()) => fake_result.and(Err(DemoError::ServerStopped)),
            }
        }
        Stop::Fake(result) => {
            let _ = gateway_shutdown_tx.send(());
            let gateway_result = stop_server(&mut gateway_task).await;
            match validate_server_result(result) {
                Err(error) => Err(error),
                Ok(()) => gateway_result.and(Err(DemoError::ServerStopped)),
            }
        }
    }
}

async fn run_checks(
    gateway_address: SocketAddr,
    local_session_token: &str,
    synthetic_authorization: &str,
    fake_state: &FakeUpstreamState,
    audit_path: &Path,
    synthetic_wire_proof: &SyntheticWireProof,
) -> Result<(), DemoError> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(10))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| DemoError::Client)?;
    let responses_url = format!("http://{gateway_address}/v1/responses");
    let raw = format!("Contact {SYNTHETIC_EMAIL} on {SYNTHETIC_PRIVATE_IP}");

    let hard_secret = format!("{}{}", "sk-proj-", "A1b2C3d4E5f6G7h8I9j0");
    let blocked = client
        .post(&responses_url)
        .header(LOCAL_SESSION_HEADER, local_session_token)
        .json(&codex_request(&hard_secret, false))
        .send()
        .await
        .map_err(|_| DemoError::Client)?;
    if blocked.status() != StatusCode::UNPROCESSABLE_ENTITY {
        return Err(DemoError::Check("hard_secret_status"));
    }
    let blocked_body = blocked.text().await.map_err(|_| DemoError::Client)?;
    if blocked_body.contains(&hard_secret)
        || !blocked_body.contains("\"protected_request_bytes_forwarded\":0")
        || fake_state.request_count.load(Ordering::SeqCst) != 0
    {
        return Err(DemoError::Check("zero_connect_block"));
    }

    let allowed = client
        .post(&responses_url)
        .header(LOCAL_SESSION_HEADER, local_session_token)
        .json(&codex_request("SAFE_SYNTHETIC_INPUT", false))
        .send()
        .await
        .map_err(|_| DemoError::Client)?;
    if allowed.status() != StatusCode::OK {
        return Err(DemoError::Check("allowed_request"));
    }
    let allowed_response = allowed.text().await.map_err(|_| DemoError::Client)?;
    if !allowed_response.contains("SAFE_SYNTHETIC_RESPONSE")
        || fake_state.request_count.load(Ordering::SeqCst) != 1
    {
        return Err(DemoError::Check("allowed_response"));
    }

    let tokenized = client
        .post(&responses_url)
        .header(LOCAL_SESSION_HEADER, local_session_token)
        .header("authorization", synthetic_authorization)
        .json(&codex_request(&raw, false))
        .send()
        .await
        .map_err(|_| DemoError::Client)?;
    if tokenized.status() != StatusCode::OK {
        return Err(DemoError::Check("tokenized_request"));
    }
    let tokenized_response = tokenized.text().await.map_err(|_| DemoError::Client)?;
    if contains_protected_value(&tokenized_response, &hard_secret)
        || tokenized_response.contains("[AV_")
        || !tokenized_response.contains("SAFE_SYNTHETIC_RESPONSE")
        || fake_state.request_count.load(Ordering::SeqCst) != 2
    {
        return Err(DemoError::Check("tokenized_response"));
    }

    let tool_output = client
        .post(&responses_url)
        .header(LOCAL_SESSION_HEADER, local_session_token)
        .json(&codex_request(&raw, true))
        .send()
        .await
        .map_err(|_| DemoError::Client)?;
    if tool_output.status() != StatusCode::OK {
        return Err(DemoError::Check("tool_output_status"));
    }
    let tool_output_response = tool_output.text().await.map_err(|_| DemoError::Client)?;
    if contains_protected_value(&tool_output_response, &hard_secret)
        || tool_output_response.contains("[AV_")
        || !tool_output_response.contains("SAFE_SYNTHETIC_RESPONSE")
        || fake_state.request_count.load(Ordering::SeqCst) != 3
    {
        return Err(DemoError::Check("tool_output_boundary"));
    }

    let bodies = fake_state.bodies.lock().await;
    let protected_bodies_are_safe = bodies.iter().skip(1).all(|body| {
        !contains_bytes(body, SYNTHETIC_EMAIL.as_bytes())
            && !contains_bytes(body, SYNTHETIC_PRIVATE_IP.as_bytes())
            && contains_bytes(body, b"[AV_EMAIL_")
            && contains_bytes(body, b"[AV_IPV4_")
    });
    if bodies.len() != 3
        || !protected_bodies_are_safe
        || bodies.iter().any(|body| {
            contains_bytes(body, SYNTHETIC_EMAIL.as_bytes())
                || contains_bytes(body, SYNTHETIC_PRIVATE_IP.as_bytes())
                || contains_bytes(body, hard_secret.as_bytes())
                || contains_bytes(body, local_session_token.as_bytes())
                || contains_bytes(body, synthetic_authorization.as_bytes())
        })
        || fake_state.authorization_seen.load(Ordering::SeqCst)
        || fake_state.forbidden_header_seen.load(Ordering::SeqCst)
    {
        return Err(DemoError::Check("capturing_upstream"));
    }
    drop(bodies);

    let dashboard = client
        .get(format!("http://{gateway_address}/dashboard"))
        .send()
        .await
        .map_err(|_| DemoError::Client)?;
    if dashboard.status() != StatusCode::OK
        || dashboard
            .headers()
            .get("content-security-policy")
            .and_then(|value| value.to_str().ok())
            .is_none_or(|value| !value.contains("default-src 'none'"))
        || dashboard
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok())
            != Some("no-store, max-age=0")
    {
        return Err(DemoError::Check("dashboard_headers"));
    }
    let dashboard_html = dashboard.text().await.map_err(|_| DemoError::Client)?;
    if contains_protected_value(&dashboard_html, &hard_secret)
        || dashboard_html.contains(local_session_token)
        || dashboard_html.contains(synthetic_authorization)
    {
        return Err(DemoError::Check("dashboard_html"));
    }

    let dashboard_state = client
        .get(format!("http://{gateway_address}/dashboard/state"))
        .send()
        .await
        .map_err(|_| DemoError::Client)?;
    if dashboard_state.status() != StatusCode::OK {
        return Err(DemoError::Check("dashboard_state_status"));
    }
    let dashboard_state = dashboard_state
        .text()
        .await
        .map_err(|_| DemoError::Client)?;
    if contains_protected_value(&dashboard_state, &hard_secret)
        || dashboard_state.contains("[AV_")
        || dashboard_state.contains(local_session_token)
        || dashboard_state.contains(synthetic_authorization)
    {
        return Err(DemoError::Check("dashboard_state_leak"));
    }
    let dashboard_json: Value =
        serde_json::from_str(&dashboard_state).map_err(|_| DemoError::DashboardState)?;
    if dashboard_json.get("wire_proof").and_then(Value::as_str) != Some("not_measured")
        || dashboard_json.get("request_count").and_then(Value::as_u64) != Some(4)
        || dashboard_json.get("binding").and_then(Value::as_str) != Some("loopback")
        || dashboard_json.get("upstream").and_then(Value::as_str) != Some("loopback_test")
        || dashboard_json.get("restoration").and_then(Value::as_str) != Some("disabled")
        || dashboard_json.get("protected").and_then(Value::as_bool) != Some(true)
        || dashboard_json.get("audit_healthy").and_then(Value::as_bool) != Some(true)
        || dashboard_json
            .get("activity")
            .and_then(Value::as_array)
            .is_none_or(|activity| !valid_dashboard_activity(activity))
    {
        return Err(DemoError::Check("dashboard_state_shape"));
    }

    let audit = std::fs::read_to_string(audit_path).map_err(|_| DemoError::AuditRead)?;
    if contains_protected_value(&audit, &hard_secret)
        || audit.contains("[AV_")
        || audit.contains(local_session_token)
        || audit.contains(synthetic_authorization)
        || !valid_audit(&audit)?
    {
        return Err(DemoError::Check("audit_value_free"));
    }

    synthetic_wire_proof.mark_passed();
    let proof_state = client
        .get(format!("http://{gateway_address}/dashboard/state"))
        .send()
        .await
        .map_err(|_| DemoError::Client)?;
    if proof_state.status() != StatusCode::OK {
        return Err(DemoError::Check("dashboard_proof_status"));
    }
    let proof_state = proof_state.text().await.map_err(|_| DemoError::Client)?;
    if contains_protected_value(&proof_state, &hard_secret)
        || proof_state.contains("[AV_")
        || proof_state.contains(local_session_token)
        || proof_state.contains(synthetic_authorization)
    {
        return Err(DemoError::Check("dashboard_proof_leak"));
    }
    let proof_json: Value =
        serde_json::from_str(&proof_state).map_err(|_| DemoError::DashboardState)?;
    if proof_json.get("wire_proof").and_then(Value::as_str) != Some("synthetic_passed") {
        return Err(DemoError::Check("dashboard_proof_state"));
    }
    Ok(())
}

async fn fake_responses(
    State(state): State<Arc<FakeUpstreamState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    state.request_count.fetch_add(1, Ordering::SeqCst);
    state.observe_headers(&headers);
    state.bodies.lock().await.push(body.to_vec());
    if serde_json::from_slice::<Value>(&body).is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let data = json!({
        "type": "response.output_text.delta",
        "sequence_number": 1,
        "item_id": "msg_synthetic",
        "output_index": 0,
        "content_index": 0,
        "delta": "SAFE_SYNTHETIC_RESPONSE"
    });
    let serialized = match serde_json::to_string(&data) {
        Ok(serialized) => serialized,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    (
        StatusCode::OK,
        [("content-type", "text/event-stream; charset=utf-8")],
        format!("event: response.output_text.delta\ndata: {serialized}\n\n"),
    )
        .into_response()
}

fn codex_request(text: &str, tool_output: bool) -> Value {
    let input = if tool_output {
        json!([{
            "type": "function_call_output",
            "call_id": "call_synthetic",
            "output": text
        }])
    } else {
        json!([{
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": text}]
        }])
    };
    json!({
        "model": "gpt-5.6-luna",
        "instructions": "Use the protected references.",
        "input": input,
        "tools": [],
        "tool_choice": "auto",
        "parallel_tool_calls": true,
        "reasoning": {"effort": "medium"},
        "store": false,
        "stream": true,
        "include": [],
        "client_metadata": {
            "session_id": "synthetic-session",
            "thread_id": "synthetic-thread"
        }
    })
}

fn valid_dashboard_activity(activity: &[Value]) -> bool {
    if activity.len() != 4 {
        return false;
    }
    let sequences = activity
        .iter()
        .filter_map(|event| event.get("request_sequence").and_then(Value::as_u64))
        .collect::<BTreeSet<_>>();
    if sequences.len() != activity.len() {
        return false;
    }
    let has_decision = |expected: &str| {
        activity
            .iter()
            .any(|event| event.get("decision").and_then(Value::as_str) == Some(expected))
    };
    let has_function_output = activity.iter().any(|event| {
        event
            .get("findings")
            .and_then(Value::as_array)
            .is_some_and(|findings| {
                findings.iter().any(|finding| {
                    finding.get("source_field").and_then(Value::as_str) == Some("function_output")
                })
            })
    });
    has_decision("allowed")
        && has_decision("rewritten")
        && has_decision("blocked")
        && has_function_output
}

fn valid_audit(audit: &str) -> Result<bool, DemoError> {
    let mut event_count = 0_usize;
    let mut blocked_not_started = false;
    let mut function_output = false;
    for line in audit.lines() {
        if line.is_empty() {
            return Ok(false);
        }
        let event: Value = serde_json::from_str(line).map_err(|_| DemoError::AuditFormat)?;
        event_count += 1;
        blocked_not_started |= event.get("decision").and_then(Value::as_str) == Some("blocked")
            && event.get("upstream_outcome").and_then(Value::as_str) == Some("not_started");
        function_output |=
            event
                .get("findings")
                .and_then(Value::as_array)
                .is_some_and(|findings| {
                    findings.iter().any(|finding| {
                        finding.get("source_field").and_then(Value::as_str)
                            == Some("function_output")
                    })
                });
    }
    Ok(event_count >= 7 && blocked_not_started && function_output)
}

fn contains_protected_value(content: &str, hard_secret: &str) -> bool {
    content.contains(SYNTHETIC_EMAIL)
        || content.contains(SYNTHETIC_PRIVATE_IP)
        || content.contains(hard_secret)
        || content.contains("PROJECT-VEIL-DEMO")
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn private_runtime_path() -> Result<PathBuf, DemoError> {
    let mut random = [0_u8; 12];
    getrandom::fill(&mut random).map_err(|_| DemoError::RandomUnavailable)?;
    let mut suffix = String::with_capacity(random.len() * 2);
    for byte in random {
        use std::fmt::Write as _;
        write!(&mut suffix, "{byte:02x}").map_err(|_| DemoError::RandomUnavailable)?;
    }
    Ok(std::env::temp_dir().join(format!("agentveil-demo-{suffix}")))
}

fn random_hex(bytes: usize) -> Result<Zeroizing<String>, DemoError> {
    let mut random = Zeroizing::new(vec![0_u8; bytes]);
    getrandom::fill(&mut random).map_err(|_| DemoError::RandomUnavailable)?;
    let mut encoded = Zeroizing::new(String::with_capacity(bytes * 2));
    for byte in random.iter() {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02X}").map_err(|_| DemoError::RandomUnavailable)?;
    }
    Ok(encoded)
}

async fn stop_server(server: &mut JoinHandle<Result<(), io::Error>>) -> Result<(), DemoError> {
    match tokio::time::timeout(SERVER_SHUTDOWN_TIMEOUT, &mut *server).await {
        Ok(result) => validate_server_result(result),
        Err(_) => {
            server.abort();
            let _ = server.await;
            Err(DemoError::ShutdownTimeout)
        }
    }
}

fn validate_server_result(
    result: Result<Result<(), io::Error>, tokio::task::JoinError>,
) -> Result<(), DemoError> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err(DemoError::ServerRuntime),
        Err(_) => Err(DemoError::ServerTask),
    }
}

struct RuntimeGuard {
    audit_path: PathBuf,
}

impl RuntimeGuard {
    fn new(runtime_path: PathBuf) -> Self {
        Self {
            audit_path: runtime_path.join("audit.jsonl"),
        }
    }

    fn audit_path(&self) -> &Path {
        &self.audit_path
    }
}

impl Drop for RuntimeGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.audit_path);
        if let Some(parent) = self.audit_path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

#[derive(Debug, Error)]
pub enum DemoError {
    #[error("offline demo loopback listener could not bind")]
    LoopbackBind,
    #[error("offline demo loopback URL could not be constructed")]
    LoopbackUrl,
    #[error("offline demo secure randomness is unavailable")]
    RandomUnavailable,
    #[error("offline demo client initialization or request failed")]
    Client,
    #[error("offline demo dashboard state was invalid")]
    DashboardState,
    #[error("offline demo audit could not be read")]
    AuditRead,
    #[error("offline demo audit structure was invalid")]
    AuditFormat,
    #[error("offline demo check failed: {0}")]
    Check(&'static str),
    #[error("offline demo signal handler failed")]
    Signal,
    #[error("offline demo server task failed")]
    ServerTask,
    #[error("offline demo server runtime failed")]
    ServerRuntime,
    #[error("offline demo server stopped unexpectedly")]
    ServerStopped,
    #[error("offline demo server shutdown timed out")]
    ShutdownTimeout,
    #[error("offline demo policy is invalid")]
    Policy(#[from] PolicyError),
    #[error("offline demo session scope is invalid")]
    Scope(#[from] LedgerError),
    #[error("offline demo gateway could not be configured")]
    Gateway(#[from] GatewayError),
}
