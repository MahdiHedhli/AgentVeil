#![allow(clippy::expect_used, clippy::panic)]

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use agentveil::gateway::{GatewayConfig, UpstreamMode, build_router};
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

#[derive(Default)]
struct FakeState {
    request_count: AtomicUsize,
    authorization_seen: AtomicBool,
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
        .store(headers.contains_key("authorization"), Ordering::SeqCst);
    state.bodies.lock().await.push(body.to_vec());
    let parsed: Value = serde_json::from_slice(&body).expect("gateway body should parse");
    let echo = first_model_text(&parsed).unwrap_or_else(|| "SAFE_EMPTY".to_string());
    let data = json!({
        "type": "response.output_text.delta",
        "sequence_number": 1,
        "item_id": "msg_synthetic",
        "output_index": 0,
        "content_index": 0,
        "delta": echo
    });
    (
        StatusCode::OK,
        [("content-type", "text/event-stream; charset=utf-8")],
        format!(
            "event: response.output_text.delta\ndata: {}\n\n",
            serde_json::to_string(&data).expect("fake event should serialize")
        ),
    )
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
    drop(gateway_listener);
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
        restore_display_text: true,
    })
    .expect("gateway should build");
    let (gateway_address, gateway_shutdown, gateway_task) = spawn(gateway_app).await;
    let endpoint = format!("http://{gateway_address}/v1/responses");
    let client = reqwest::Client::new();

    let raw = format!("Contact {EMAIL} on {PRIVATE_IP}");
    let response = client
        .post(&endpoint)
        .header("x-agentveil-session", LOCAL_TOKEN)
        .header("authorization", "Bearer SYNTHETIC_NOT_FOR_UPSTREAM")
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

    let tool_response = client
        .post(&endpoint)
        .header("x-agentveil-session", LOCAL_TOKEN)
        .json(&codex_request(&raw, true))
        .send()
        .await
        .expect("tool-output request should complete");
    assert_eq!(tool_response.status(), StatusCode::OK);
    assert_eq!(fake_state.request_count.load(Ordering::SeqCst), 2);
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
