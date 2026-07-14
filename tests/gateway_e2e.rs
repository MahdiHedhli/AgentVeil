#![allow(clippy::expect_used, clippy::panic)]

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use agentveil::gateway::{
    GatewayClient, GatewayConfig, RestorationMode, SyntheticWireProof, UpstreamMode, build_router,
};
use agentveil::ledger::SessionScope;
use agentveil::policy::ValidatedPolicy;
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use reqwest::Url;
use serde_json::{Value, json};
use tokio::sync::{Mutex, oneshot};
use zeroize::Zeroizing;

const EMAIL: &str = "ava.agentveil@example.test";
const PRIVATE_IP: &str = "10.24.8.15";
const LOCAL_TOKEN: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

fn synthetic_authorization() -> String {
    format!("{}{}", "Bearer ", "SYNTHETIC_NOT_FOR_UPSTREAM")
}

#[derive(Default)]
struct FakeState {
    request_count: AtomicUsize,
    authorization_seen: AtomicBool,
    forbidden_header_seen: AtomicBool,
    bodies: Mutex<Vec<Vec<u8>>>,
}

async fn fake_responses(
    State(state): State<Arc<FakeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    state.request_count.fetch_add(1, Ordering::SeqCst);
    state
        .authorization_seen
        .fetch_or(headers.contains_key("authorization"), Ordering::SeqCst);
    let forbidden_value_seen = headers.values().any(|value| {
        value.as_bytes() == LOCAL_TOKEN.as_bytes()
            || value.as_bytes() == synthetic_authorization().as_bytes()
    });
    state.forbidden_header_seen.fetch_or(
        headers.contains_key("x-agentveil-session") || forbidden_value_seen,
        Ordering::SeqCst,
    );
    state.bodies.lock().await.push(body.to_vec());
    let parsed: Value = serde_json::from_slice(&body).expect("gateway body should parse");
    let echo = first_model_text(&parsed).unwrap_or_else(|| "SAFE_EMPTY".to_string());
    (
        StatusCode::OK,
        [("content-type", "text/event-stream; charset=utf-8")],
        complete_sse(&echo),
    )
}

fn sse_event(value: Value) -> String {
    let event_type = value["type"]
        .as_str()
        .expect("fake event type should be a string");
    format!(
        "event: {event_type}\ndata: {}\n\n",
        serde_json::to_string(&value).expect("fake event should serialize")
    )
}

fn complete_sse(text: &str) -> String {
    let message_in_progress = json!({
        "id": "msg_synthetic",
        "type": "message",
        "status": "in_progress",
        "role": "assistant",
        "content": []
    });
    let content_in_progress = json!({
        "type": "output_text",
        "annotations": [],
        "logprobs": [],
        "text": ""
    });
    let content_done = json!({
        "type": "output_text",
        "annotations": [],
        "logprobs": [],
        "text": text
    });
    let message_done = json!({
        "id": "msg_synthetic",
        "type": "message",
        "status": "completed",
        "role": "assistant",
        "content": [content_done.clone()]
    });
    [
        json!({
            "type": "response.created",
            "sequence_number": 0,
            "response": {"id": "resp_synthetic", "output": []}
        }),
        json!({
            "type": "response.in_progress",
            "sequence_number": 1,
            "response": {"id": "resp_synthetic", "output": []}
        }),
        json!({
            "type": "response.output_item.added",
            "sequence_number": 2,
            "output_index": 0,
            "item": message_in_progress
        }),
        json!({
            "type": "response.content_part.added",
            "sequence_number": 3,
            "item_id": "msg_synthetic",
            "output_index": 0,
            "content_index": 0,
            "part": content_in_progress
        }),
        json!({
            "type": "response.output_text.delta",
            "sequence_number": 4,
            "item_id": "msg_synthetic",
            "output_index": 0,
            "content_index": 0,
            "delta": text,
            "logprobs": []
        }),
        json!({
            "type": "response.output_text.done",
            "sequence_number": 5,
            "item_id": "msg_synthetic",
            "output_index": 0,
            "content_index": 0,
            "text": text,
            "logprobs": []
        }),
        json!({
            "type": "response.content_part.done",
            "sequence_number": 6,
            "item_id": "msg_synthetic",
            "output_index": 0,
            "content_index": 0,
            "part": content_done
        }),
        json!({
            "type": "response.output_item.done",
            "sequence_number": 7,
            "output_index": 0,
            "item": message_done.clone()
        }),
        json!({
            "type": "response.completed",
            "sequence_number": 8,
            "response": {"id": "resp_synthetic", "output": [message_done]}
        }),
    ]
    .into_iter()
    .map(sse_event)
    .collect()
}

fn parse_sse_events(body: &str) -> Vec<Value> {
    body.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str(data).expect("SSE data should parse"))
        .collect()
}

fn first_model_text(value: &Value) -> Option<String> {
    let input = value.get("input")?.as_array()?;
    for item in input {
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                let content = item.get("content")?.as_array()?;
                for part in content {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        return Some(text.to_string());
                    }
                }
            }
            Some("function_call_output") => {
                if let Some(text) = item.get("output").and_then(Value::as_str) {
                    return Some(text.to_string());
                }
            }
            _ => {}
        }
    }
    None
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

fn codex_replay_request(tokenized_assistant_text: &str) -> Value {
    json!({
        "model": "gpt-5.6-luna",
        "instructions": "Use the protected references.",
        "input": [
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": tokenized_assistant_text}]
            },
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Repeat the synthetic configuration exactly."}]
            }
        ],
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

fn temporary_audit_path() -> PathBuf {
    let mut random = [0_u8; 8];
    getrandom::fill(&mut random).expect("test randomness should be available");
    let suffix = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    std::env::temp_dir()
        .join(format!("agentveil-e2e-{suffix}"))
        .join("audit.jsonl")
}

async fn spawn(app: Router) -> (SocketAddr, oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("listener should bind");
    spawn_on(listener, app).await
}

async fn spawn_on(
    listener: tokio::net::TcpListener,
    app: Router,
) -> (SocketAddr, oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
    let address = listener.local_addr().expect("listener should have address");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let result = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
        assert!(result.is_ok());
    });
    (address, shutdown_tx, task)
}

#[tokio::test]
async fn wire_proof_tokenizes_blocks_restores_and_keeps_audit_value_free() {
    let fake_state = Arc::new(FakeState::default());
    let fake_app = Router::new()
        .route("/v1/responses", post(fake_responses))
        .with_state(fake_state.clone());
    let (fake_address, fake_shutdown, fake_task) = spawn(fake_app).await;

    let gateway_listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("gateway port should reserve");
    let gateway_address = gateway_listener
        .local_addr()
        .expect("gateway should have address");
    let fake_url =
        Url::parse(&format!("http://{fake_address}")).expect("fake upstream URL should parse");
    let audit_path = temporary_audit_path();
    let policy = ValidatedPolicy::from_yaml(include_bytes!("../policies/demo.yaml"))
        .expect("demo policy should validate");
    let gateway_app = build_router(GatewayConfig {
        bind: gateway_address,
        policy,
        scope: SessionScope::parse("scope_e2e_123456").expect("scope should validate"),
        local_session_token: Zeroizing::new(LOCAL_TOKEN.to_string()),
        audit_path: audit_path.clone(),
        upstream: UpstreamMode::loopback_test(fake_url).expect("test upstream should validate"),
        client: GatewayClient::Generic,
        restoration_mode: RestorationMode::SyntheticDisplayText,
        synthetic_wire_proof: SyntheticWireProof::unmeasured(),
    })
    .expect("gateway should build");
    let (gateway_address, gateway_shutdown, gateway_task) =
        spawn_on(gateway_listener, gateway_app).await;
    let endpoint = format!("http://{gateway_address}/v1/responses");
    let client = reqwest::Client::new();

    let raw = format!("Contact {EMAIL} on {PRIVATE_IP}");
    let synthetic_authorization = synthetic_authorization();
    let response = client
        .post(&endpoint)
        .header("x-agentveil-session", LOCAL_TOKEN)
        .header("authorization", &synthetic_authorization)
        .json(&codex_request(&raw, false))
        .send()
        .await
        .expect("tokenization request should complete");
    assert_eq!(response.status(), StatusCode::OK);
    let local_response = response.text().await.expect("SSE should read");
    assert!(local_response.contains(EMAIL));
    assert!(local_response.contains(PRIVATE_IP));
    assert_eq!(fake_state.request_count.load(Ordering::SeqCst), 1);
    assert!(!fake_state.authorization_seen.load(Ordering::SeqCst));
    assert!(!fake_state.forbidden_header_seen.load(Ordering::SeqCst));
    let first_body = fake_state.bodies.lock().await[0].clone();
    let first_body = String::from_utf8(first_body).expect("capture should be UTF-8");
    assert!(!first_body.contains(EMAIL));
    assert!(!first_body.contains(PRIVATE_IP));
    assert!(first_body.contains("[AV_EMAIL_"));
    assert!(first_body.contains("[AV_IPV4_"));

    let secret = format!("{}{}", "sk-proj-", "A1b2C3d4E5f6G7h8I9j0");
    let blocked = client
        .post(&endpoint)
        .header("x-agentveil-session", LOCAL_TOKEN)
        .json(&codex_request(&format!("{EMAIL} {secret}"), false))
        .send()
        .await
        .expect("blocked request should return local error");
    assert_eq!(blocked.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let blocked_body = blocked.text().await.expect("block response should read");
    assert!(blocked_body.contains("\"protected_request_bytes_forwarded\":0"));
    assert!(!blocked_body.contains(&secret));
    assert_eq!(fake_state.request_count.load(Ordering::SeqCst), 1);

    let encoded = format!("%73%6B%2D%70%72%6F%6A%2D{}", "Z9y8X7w6V5u4T3s2R1q0");
    let bypass = client
        .post(&endpoint)
        .header("x-agentveil-session", LOCAL_TOKEN)
        .json(&codex_request(&encoded, false))
        .send()
        .await
        .expect("encoded request should return local error");
    assert_eq!(bypass.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(fake_state.request_count.load(Ordering::SeqCst), 1);

    let encrypted_private_key = format!(
        "{}{}",
        "-----BEGIN ENCRYPTED PRIVATE ", "KEY-----\nSYNTHETIC"
    );
    let composed_bypasses = [
        r"\uFF53\uFF4B-proj-A1b2C3d4E5f6G7h8I9j0",
        r"sk-proj-A1\u002Eb2\u002EC3\u002Ed4\u002EE5\u002Ef6\u002EG7\u002Eh8\u002EI9\u002Ej0",
        "%73%6B%2D%70%72%6F%6A%2D%41%2E%31%2E%62%2E%32%2E%43%2E%33%2E%64%2E%34%2E%45%2E%35%2E%66%2E%36%2E%47%2E%68%2E%38%2E%49%2E%39%2E%6A%2E%30",
        r"\\u0073k-proj-A1b2C3d4E5f6G7h8I9j0",
        "Xsk-proj-A1b2C3d4E5f6G7h8I9j0",
        encrypted_private_key.as_str(),
    ];
    for candidate in composed_bypasses {
        let response = client
            .post(&endpoint)
            .header("x-agentveil-session", LOCAL_TOKEN)
            .json(&codex_request(candidate, false))
            .send()
            .await
            .expect("bypass request should return local error");
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(fake_state.request_count.load(Ordering::SeqCst), 1);
    }

    let invalid_percent =
        "%73%6B%2D%70%72%6F%6A%2D%41%31%62%32%43%33%64%34%45%35%66%36%47%37%68%38%49%39%6A%30%FF";
    let invalid_percent_response = client
        .post(&endpoint)
        .header("x-agentveil-session", LOCAL_TOKEN)
        .json(&codex_request(invalid_percent, false))
        .send()
        .await
        .expect("invalid percent request should fail closed");
    assert_eq!(
        invalid_percent_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(fake_state.request_count.load(Ordering::SeqCst), 1);

    let structural_secret = format!("{}{}", "ghp_", "Q1w2E3r4T5y6U7i8O9p0A1s2D3f4G5");
    let mut structural_payload = codex_request("safe", false);
    structural_payload["text"] = json!({
        "format": {
            "type": "json_schema",
            "strict": true,
            "name": "synthetic",
            "schema": {
                "type": "object",
                "properties": {
                    structural_secret.clone(): {"type": "string"}
                }
            }
        }
    });
    let structural_response = client
        .post(&endpoint)
        .header("x-agentveil-session", LOCAL_TOKEN)
        .json(&structural_payload)
        .send()
        .await
        .expect("structural secret should return local error");
    assert_eq!(
        structural_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(fake_state.request_count.load(Ordering::SeqCst), 1);

    let mut metadata_payload = codex_request("safe", false);
    metadata_payload["input"][0]["internal_chat_message_metadata_passthrough"] =
        json!({"turn_id": structural_secret});
    let metadata_response = client
        .post(&endpoint)
        .header("x-agentveil-session", LOCAL_TOKEN)
        .json(&metadata_payload)
        .send()
        .await
        .expect("metadata secret should return local error");
    assert_eq!(metadata_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(fake_state.request_count.load(Ordering::SeqCst), 1);

    let tool_response = client
        .post(&endpoint)
        .header("x-agentveil-session", LOCAL_TOKEN)
        .json(&codex_request(&raw, true))
        .send()
        .await
        .expect("tool-output request should complete");
    assert_eq!(tool_response.status(), StatusCode::OK);
    assert_eq!(fake_state.request_count.load(Ordering::SeqCst), 2);
    assert!(!fake_state.authorization_seen.load(Ordering::SeqCst));
    assert!(!fake_state.forbidden_header_seen.load(Ordering::SeqCst));
    let second_body = fake_state.bodies.lock().await[1].clone();
    let second_body = String::from_utf8(second_body).expect("capture should be UTF-8");
    assert!(!second_body.contains(EMAIL));
    assert!(!second_body.contains(PRIVATE_IP));

    let unsupported = json!({
        "model": "gpt-5.6-luna",
        "input": [],
        "store": false,
        "stream": true,
        "future_context": EMAIL
    });
    let rejected = client
        .post(&endpoint)
        .header("x-agentveil-session", LOCAL_TOKEN)
        .json(&unsupported)
        .send()
        .await
        .expect("unsupported request should return local error");
    assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(fake_state.request_count.load(Ordering::SeqCst), 2);

    let unauthorized = client
        .post(&endpoint)
        .json(&codex_request("safe", false))
        .send()
        .await
        .expect("unauthorized request should return local error");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(fake_state.request_count.load(Ordering::SeqCst), 2);

    let dashboard = client
        .get(format!("http://{gateway_address}/dashboard"))
        .send()
        .await
        .expect("dashboard should load from loopback");
    assert_eq!(dashboard.status(), StatusCode::OK);
    let dashboard_headers = dashboard.headers().clone();
    assert_eq!(
        dashboard_headers
            .get("x-content-type-options")
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );
    assert!(
        dashboard_headers
            .get("content-security-policy")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("default-src 'none'"))
    );
    let dashboard_body = dashboard.text().await.expect("dashboard should be text");
    assert!(!dashboard_body.contains(EMAIL));
    assert!(!dashboard_body.contains(PRIVATE_IP));
    assert!(!dashboard_body.contains(LOCAL_TOKEN));

    let dashboard_state = client
        .get(format!("http://{gateway_address}/dashboard/state"))
        .send()
        .await
        .expect("dashboard state should load from loopback");
    assert_eq!(dashboard_state.status(), StatusCode::OK);
    let dashboard_state = dashboard_state
        .text()
        .await
        .expect("dashboard state should be text");
    assert!(!dashboard_state.contains(EMAIL));
    assert!(!dashboard_state.contains(PRIVATE_IP));
    assert!(!dashboard_state.contains(LOCAL_TOKEN));
    assert!(!dashboard_state.contains("[AV_"));
    let dashboard_state: Value =
        serde_json::from_str(&dashboard_state).expect("dashboard state should parse");
    assert_eq!(dashboard_state["schema_version"], 2);
    assert_eq!(dashboard_state["wire_proof"], "not_measured");
    assert_eq!(dashboard_state["binding"], "loopback");
    assert_eq!(dashboard_state["transport"], "responses_sse");
    assert_eq!(dashboard_state["client"], "generic");
    assert_eq!(dashboard_state["restoration"], "synthetic_full");
    let activity = dashboard_state["activity"]
        .as_array()
        .expect("dashboard activity should be an array");
    let mut sequences = activity
        .iter()
        .filter_map(|event| event["request_sequence"].as_u64())
        .collect::<Vec<_>>();
    let activity_count = sequences.len();
    sequences.sort_unstable();
    sequences.dedup();
    assert_eq!(sequences.len(), activity_count);

    let rebound = client
        .get(format!("http://{gateway_address}/dashboard/state"))
        .header("host", "attacker.example")
        .send()
        .await
        .expect("hostile dashboard authority should return local rejection");
    assert_eq!(rebound.status(), StatusCode::MISDIRECTED_REQUEST);

    let audit = std::fs::read_to_string(&audit_path).expect("audit should be readable");
    for forbidden in [
        EMAIL,
        PRIVATE_IP,
        &secret,
        &encoded,
        "[AV_EMAIL_",
        "[AV_IPV4_",
    ] {
        assert!(!audit.contains(forbidden));
    }
    assert!(audit.contains("\"upstream_outcome\":\"not_started\""));
    assert!(audit.contains("\"source_field\":\"function_output\""));

    let _ = gateway_shutdown.send(());
    let _ = fake_shutdown.send(());
    gateway_task.await.expect("gateway task should join");
    fake_task.await.expect("fake task should join");
    if let Some(parent) = audit_path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

#[tokio::test]
async fn delta_only_restores_live_display_but_keeps_completed_items_tokenized() {
    let fake_state = Arc::new(FakeState::default());
    let fake_app = Router::new()
        .route("/v1/responses", post(fake_responses))
        .with_state(fake_state.clone());
    let (fake_address, fake_shutdown, fake_task) = spawn(fake_app).await;

    let gateway_listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("gateway port should reserve");
    let gateway_address = gateway_listener
        .local_addr()
        .expect("gateway should have address");
    let fake_url =
        Url::parse(&format!("http://{fake_address}")).expect("fake upstream URL should parse");
    let audit_path = temporary_audit_path();
    let policy = ValidatedPolicy::from_yaml(include_bytes!("../policies/demo.yaml"))
        .expect("demo policy should validate");
    let gateway_app = build_router(GatewayConfig {
        bind: gateway_address,
        policy,
        scope: SessionScope::parse("scope_delta_123456").expect("scope should validate"),
        local_session_token: Zeroizing::new(LOCAL_TOKEN.to_string()),
        audit_path: audit_path.clone(),
        upstream: UpstreamMode::loopback_test(fake_url).expect("test upstream should validate"),
        client: GatewayClient::Generic,
        restoration_mode: RestorationMode::SyntheticDisplayDeltaOnly,
        synthetic_wire_proof: SyntheticWireProof::unmeasured(),
    })
    .expect("gateway should build");
    let (gateway_address, gateway_shutdown, gateway_task) =
        spawn_on(gateway_listener, gateway_app).await;
    let client = reqwest::Client::new();
    let raw = format!("Contact {EMAIL} on {PRIVATE_IP}");

    let response = client
        .post(format!("http://{gateway_address}/v1/responses"))
        .header("x-agentveil-session", LOCAL_TOKEN)
        .json(&codex_request(&raw, false))
        .send()
        .await
        .expect("protected request should complete");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("SSE should read");
    let events = parse_sse_events(&body);
    let event = |event_type: &str| {
        events
            .iter()
            .find(|event| event["type"] == event_type)
            .expect("expected event should exist")
    };

    let delta = event("response.output_text.delta")["delta"]
        .as_str()
        .expect("delta should be text");
    assert!(delta.contains(EMAIL));
    assert!(delta.contains(PRIVATE_IP));
    assert!(!delta.contains("[AV_"));

    let done = event("response.output_text.done")["text"]
        .as_str()
        .expect("completed text should be text");
    assert!(!done.contains(EMAIL));
    assert!(!done.contains(PRIVATE_IP));
    assert!(done.contains("[AV_EMAIL_"));
    assert!(done.contains("[AV_IPV4_"));

    let item_done = &event("response.output_item.done")["item"]["content"][0]["text"];
    let item_done = item_done
        .as_str()
        .expect("completed item text should be text");
    assert_eq!(item_done, done);
    let response_done = &event("response.completed")["response"]["output"][0]["content"][0]["text"];
    assert_eq!(response_done.as_str(), Some(done));

    let captured = fake_state.bodies.lock().await[0].clone();
    let captured = String::from_utf8(captured).expect("captured body should be UTF-8");
    assert!(!captured.contains(EMAIL));
    assert!(!captured.contains(PRIVATE_IP));
    assert!(captured.contains("[AV_EMAIL_"));
    assert!(captured.contains("[AV_IPV4_"));

    let replay_response = client
        .post(format!("http://{gateway_address}/v1/responses"))
        .header("x-agentveil-session", LOCAL_TOKEN)
        .json(&codex_replay_request(done))
        .send()
        .await
        .expect("owned-token replay should complete");
    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay_body = replay_response
        .text()
        .await
        .expect("replay SSE should read");
    let replay_events = parse_sse_events(&replay_body);
    let replay_event = |event_type: &str| {
        replay_events
            .iter()
            .find(|event| event["type"] == event_type)
            .expect("expected replay event should exist")
    };
    let replay_delta = replay_event("response.output_text.delta")["delta"]
        .as_str()
        .expect("replay delta should be text");
    assert!(replay_delta.contains(EMAIL));
    assert!(replay_delta.contains(PRIVATE_IP));
    assert!(!replay_delta.contains("[AV_"));
    let replay_done = replay_event("response.output_item.done")["item"]["content"][0]["text"]
        .as_str()
        .expect("replay completion should be text");
    assert_eq!(replay_done, done);

    let captured_replay = fake_state.bodies.lock().await[1].clone();
    let captured_replay =
        String::from_utf8(captured_replay).expect("captured replay should be UTF-8");
    assert!(!captured_replay.contains(EMAIL));
    assert!(!captured_replay.contains(PRIVATE_IP));
    assert!(captured_replay.contains(done));

    let dashboard_state = client
        .get(format!("http://{gateway_address}/dashboard/state"))
        .send()
        .await
        .expect("dashboard state should load");
    let dashboard_state: Value = dashboard_state
        .json()
        .await
        .expect("dashboard state should parse");
    assert_eq!(
        dashboard_state["restoration"],
        "synthetic_display_delta_only"
    );
    assert_eq!(dashboard_state["client"], "generic");
    let serialized_dashboard =
        serde_json::to_string(&dashboard_state).expect("dashboard state should serialize");
    assert!(!serialized_dashboard.contains(EMAIL));
    assert!(!serialized_dashboard.contains(PRIVATE_IP));
    assert!(!serialized_dashboard.contains("[AV_"));

    let _ = gateway_shutdown.send(());
    let _ = fake_shutdown.send(());
    gateway_task.await.expect("gateway task should join");
    fake_task.await.expect("fake task should join");
    if let Some(parent) = audit_path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

#[test]
fn every_synthetic_restoration_mode_is_rejected_for_live_openai() {
    for restoration_mode in [
        RestorationMode::SyntheticDisplayText,
        RestorationMode::SyntheticDisplayDeltaOnly,
    ] {
        let result = build_router(GatewayConfig {
            bind: SocketAddr::from((Ipv4Addr::LOCALHOST, 48741)),
            policy: ValidatedPolicy::from_yaml(include_bytes!("../policies/default.yaml"))
                .expect("default policy should validate"),
            scope: SessionScope::parse("scope_live_123456").expect("scope should validate"),
            local_session_token: Zeroizing::new(LOCAL_TOKEN.to_string()),
            audit_path: temporary_audit_path(),
            upstream: UpstreamMode::openai(),
            client: GatewayClient::Generic,
            restoration_mode,
            synthetic_wire_proof: SyntheticWireProof::unmeasured(),
        });
        assert!(matches!(
            result,
            Err(agentveil::gateway::GatewayError::LiveRestorationNotVerified)
        ));
    }
}
