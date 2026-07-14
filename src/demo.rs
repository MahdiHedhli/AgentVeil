use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use reqwest::Url;
use serde_json::{Value, json};
use thiserror::Error;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;
use zeroize::Zeroizing;

use crate::gateway::{
    GatewayClient, GatewayConfig, GatewayError, RestorationMode, SyntheticWireProof, UpstreamMode,
    build_router,
};
use crate::ledger::{LedgerError, SessionScope};
use crate::policy::{PolicyError, ValidatedPolicy};

const LOCAL_SESSION_HEADER: &str = "x-agentveil-session";
const SYNTHETIC_EMAIL: &str = "ava.agentveil@example.test";
const SYNTHETIC_PRIVATE_IP: &str = "10.24.8.15";
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const SERVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const CODEX_DEMO_RESPONSE_DELAY: Duration = Duration::from_secs(6);
const CODEX_DEMO_MODEL: &str = "agentveil-synthetic-fixture";
const CODEX_DEMO_MODEL_CATALOG: &str = r#"{
  "models": [{
    "slug": "agentveil-synthetic-fixture",
    "display_name": "AgentVeil Synthetic Fixture",
    "description": "Local deterministic demonstration fixture.",
    "default_reasoning_level": "none",
    "supported_reasoning_levels": [{"effort": "none", "description": "Deterministic fixture"}],
    "shell_type": "disabled",
    "visibility": "list",
    "supported_in_api": true,
    "priority": 1,
    "availability_nux": null,
    "upgrade": null,
    "base_instructions": "Return only the deterministic synthetic fixture response.",
    "supports_reasoning_summaries": false,
    "default_reasoning_summary": "none",
    "support_verbosity": false,
    "default_verbosity": null,
    "apply_patch_tool_type": null,
    "truncation_policy": {"mode": "tokens", "limit": 10000},
    "supports_parallel_tool_calls": false,
    "context_window": 128000,
    "max_context_window": 128000,
    "experimental_supported_tools": [],
    "input_modalities": ["text"]
  }]
}"#;
const CODEX_DEMO_PROMPT: &str = "Return this explicitly synthetic test config unchanged: const SUPPORT_EMAIL = \"ava.agentveil@example.test\"; const BUILD_HOST = \"10.24.8.15\";";
const CODEX_DEMO_REPLAY_PROMPT: &str = "Repeat the synthetic configuration exactly.";

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
        client: GatewayClient::Generic,
        restoration_mode: RestorationMode::Disabled,
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

struct CodexDemoFakeState {
    request_count: AtomicUsize,
    forbidden_header_seen: AtomicBool,
    session_token_hash: blake3::Hash,
    session_token_len: usize,
    wire_proof: SyntheticWireProof,
    response_delay: Duration,
}

impl CodexDemoFakeState {
    fn new(
        wire_proof: SyntheticWireProof,
        response_delay: Duration,
        local_session_token: &[u8],
    ) -> Self {
        Self {
            request_count: AtomicUsize::new(0),
            forbidden_header_seen: AtomicBool::new(false),
            session_token_hash: blake3::hash(local_session_token),
            session_token_len: local_session_token.len(),
            wire_proof,
            response_delay,
        }
    }
}

struct CodexDemoServers {
    gateway_address: SocketAddr,
    local_session_token: Zeroizing<String>,
    fake_state: Arc<CodexDemoFakeState>,
    fake_shutdown_tx: oneshot::Sender<()>,
    gateway_shutdown_tx: oneshot::Sender<()>,
    fake_task: JoinHandle<Result<(), io::Error>>,
    gateway_task: JoinHandle<Result<(), io::Error>>,
}

impl CodexDemoServers {
    async fn stop(self) -> Result<(), CodexDemoError> {
        let _ = self.gateway_shutdown_tx.send(());
        let _ = self.fake_shutdown_tx.send(());
        let mut gateway_task = self.gateway_task;
        let mut fake_task = self.fake_task;
        stop_codex_demo_server(&mut gateway_task)
            .await
            .and(stop_codex_demo_server(&mut fake_task).await)
    }
}

struct CodexDemoRuntime {
    root: PathBuf,
    codex_home: PathBuf,
    workspace: PathBuf,
    temp: PathBuf,
    audit_path: PathBuf,
    model_catalog_path: PathBuf,
    cleaned: bool,
}

impl CodexDemoRuntime {
    fn new() -> Result<Self, CodexDemoError> {
        let root = private_runtime_path_named("agentveil-codex-demo")
            .map_err(|_| CodexDemoError::Runtime)?;
        create_private_directory(&root)?;
        let codex_home = root.join("codex-home");
        let workspace = root.join("workspace");
        let temp = root.join("tmp");
        for directory in [&codex_home, &workspace, &temp] {
            if let Err(error) = create_private_directory(directory) {
                let _ = fs::remove_dir_all(&root);
                return Err(error);
            }
        }
        let audit_path = root.join("audit.jsonl");
        let model_catalog_path = root.join("model-catalog.json");
        if let Err(error) =
            write_private_file(&model_catalog_path, CODEX_DEMO_MODEL_CATALOG.as_bytes())
        {
            let _ = fs::remove_dir_all(&root);
            return Err(error);
        }
        Ok(Self {
            root,
            codex_home,
            workspace,
            temp,
            audit_path,
            model_catalog_path,
            cleaned: false,
        })
    }

    fn cleanup(&mut self) -> Result<(), CodexDemoError> {
        if !self.safe_root() {
            return Err(CodexDemoError::RuntimeCleanup);
        }
        fs::remove_dir_all(&self.root).map_err(|_| CodexDemoError::RuntimeCleanup)?;
        if self
            .root
            .try_exists()
            .map_err(|_| CodexDemoError::RuntimeCleanup)?
        {
            return Err(CodexDemoError::RuntimeCleanup);
        }
        self.cleaned = true;
        Ok(())
    }

    fn safe_root(&self) -> bool {
        self.root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("agentveil-codex-demo-"))
    }
}

impl Drop for CodexDemoRuntime {
    fn drop(&mut self) {
        if !self.cleaned && self.safe_root() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

pub async fn run_codex(codex_path: PathBuf) -> Result<(), CodexDemoError> {
    let mut runtime = CodexDemoRuntime::new()?;
    let servers = start_codex_demo_servers(&runtime, CODEX_DEMO_RESPONSE_DELAY).await?;
    let provider_id = format!(
        "agentveil-demo-{}",
        random_hex(12)
            .map_err(|_| CodexDemoError::RandomUnavailable)?
            .as_str()
            .to_ascii_lowercase()
    );
    let provider_key = format!("model_providers.{provider_id}");
    let base_url = format!("http://{}/v1", servers.gateway_address);
    let canonical_workspace =
        fs::canonicalize(&runtime.workspace).map_err(|_| CodexDemoError::Runtime)?;
    let canonical_model_catalog =
        fs::canonicalize(&runtime.model_catalog_path).map_err(|_| CodexDemoError::Runtime)?;
    let trusted_workspace_key = serde_json::to_string(
        canonical_workspace
            .to_str()
            .ok_or(CodexDemoError::Runtime)?,
    )
    .map_err(|_| CodexDemoError::Runtime)?;
    let codex_config = format!("[projects.{trusted_workspace_key}]\ntrust_level = \"trusted\"\n");
    write_private_file(
        &runtime.codex_home.join("config.toml"),
        codex_config.as_bytes(),
    )?;

    let mut codex = tokio::process::Command::new(codex_path);
    codex
        .arg("-m")
        .arg(CODEX_DEMO_MODEL)
        .arg("-c")
        .arg("history.persistence=\"none\"")
        .arg("-c")
        .arg(format!(
            "model_catalog_json=\"{}\"",
            canonical_model_catalog.display()
        ))
        .arg("-c")
        .arg("project_root_markers=[]")
        .arg("-c")
        .arg("model_context_window=128000")
        .arg("-c")
        .arg("model_reasoning_summary=\"none\"")
        .arg("-c")
        .arg(format!("model_provider=\"{provider_id}\""))
        .arg("-c")
        .arg(format!("{provider_key}.name=\"AgentVeil Synthetic Fixture\""))
        .arg("-c")
        .arg(format!("{provider_key}.base_url=\"{base_url}\""))
        .arg("-c")
        .arg(format!("{provider_key}.wire_api=\"responses\""))
        .arg("-c")
        .arg(format!("{provider_key}.requires_openai_auth=false"))
        .arg("-c")
        .arg(format!("{provider_key}.supports_websockets=false"))
        .arg("-c")
        .arg(format!(
            "{provider_key}.env_http_headers={{ \"X-AgentVeil-Session\" = \"AGENTVEIL_SESSION_TOKEN\" }}"
        ))
        .arg("-c")
        .arg("shell_environment_policy.ignore_default_excludes=false")
        .arg("-c")
        .arg("shell_environment_policy.set.AGENTVEIL_SESSION_TOKEN=\"\"")
        .arg("--disable")
        .arg("plugins")
        .arg("--disable")
        .arg("remote_plugin")
        .arg("--disable")
        .arg("apps")
        .arg("--disable")
        .arg("browser_use")
        .arg("--disable")
        .arg("computer_use")
        .arg("--sandbox")
        .arg("read-only")
        .arg("--no-alt-screen")
        .current_dir(&runtime.workspace)
        .env_clear()
        .env("AGENTVEIL_SESSION_TOKEN", servers.local_session_token.as_str())
        .env("CODEX_HOME", &runtime.codex_home)
        .env("HOME", &runtime.codex_home)
        .env("TMPDIR", &runtime.temp)
        .env("TERM", "xterm-256color")
        .env("LANG", "en_US.UTF-8")
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("SHELL", "/bin/zsh")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    println!(
        "AgentVeil synthetic Codex demo active: client=codex_cli upstream=capturing_loopback model_route=loopback_only restoration=synthetic_display_delta_only"
    );
    println!("private_runtime=isolated_temporary_home cleanup=destroyed_on_clean_exit");
    println!("dashboard=http://{}/dashboard", servers.gateway_address);

    let mut child = match codex.spawn() {
        Ok(child) => child,
        Err(_) => {
            servers.stop().await?;
            return Err(CodexDemoError::CodexLaunch);
        }
    };
    let CodexDemoServers {
        fake_shutdown_tx,
        gateway_shutdown_tx,
        mut fake_task,
        mut gateway_task,
        ..
    } = servers;
    enum Stop {
        Codex(Result<std::process::ExitStatus, io::Error>),
        Gateway(Result<Result<(), io::Error>, tokio::task::JoinError>),
        Fake(Result<Result<(), io::Error>, tokio::task::JoinError>),
    }
    let stop = tokio::select! {
        status = child.wait() => Stop::Codex(status),
        result = &mut gateway_task => Stop::Gateway(result),
        result = &mut fake_task => Stop::Fake(result),
    };
    match stop {
        Stop::Codex(status) => {
            let _ = gateway_shutdown_tx.send(());
            let _ = fake_shutdown_tx.send(());
            let gateway_result = stop_codex_demo_server(&mut gateway_task).await;
            let fake_result = stop_codex_demo_server(&mut fake_task).await;
            gateway_result.and(fake_result)?;
            let status = status.map_err(|_| CodexDemoError::CodexWait)?;
            if !status.success() {
                return Err(CodexDemoError::CodexFailed);
            }
            runtime.cleanup()?;
            Ok(())
        }
        Stop::Gateway(result) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            let _ = fake_shutdown_tx.send(());
            let fake_result = stop_codex_demo_server(&mut fake_task).await;
            validate_codex_demo_server_result(result)?;
            fake_result.and(Err(CodexDemoError::ServerStopped))
        }
        Stop::Fake(result) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            let _ = gateway_shutdown_tx.send(());
            let gateway_result = stop_codex_demo_server(&mut gateway_task).await;
            validate_codex_demo_server_result(result)?;
            gateway_result.and(Err(CodexDemoError::ServerStopped))
        }
    }
}

pub async fn run_codex_check() -> Result<(), CodexDemoError> {
    let mut runtime = CodexDemoRuntime::new()?;
    let servers = start_codex_demo_servers(&runtime, Duration::ZERO).await?;
    let check_result = codex_demo_check(
        servers.gateway_address,
        servers.local_session_token.as_str(),
        servers.fake_state.as_ref(),
        &runtime.audit_path,
    )
    .await;
    let stop_result = servers.stop().await;
    check_result.and(stop_result)?;
    runtime.cleanup()?;
    println!(
        "codex-demo-check: status=pass route=synthetic_client_codex_compatible block=pass tokenize=pass restore_delta=pass replay=pass wire_proof=pass audit=pass output=value_free"
    );
    Ok(())
}

async fn start_codex_demo_servers(
    runtime: &CodexDemoRuntime,
    response_delay: Duration,
) -> Result<CodexDemoServers, CodexDemoError> {
    let local_session_token = random_hex(32).map_err(|_| CodexDemoError::RandomUnavailable)?;
    let scope = SessionScope::parse(format!(
        "avscope_{}",
        random_hex(16)
            .map_err(|_| CodexDemoError::RandomUnavailable)?
            .as_str()
    ))?;
    let policy = ValidatedPolicy::from_yaml(include_bytes!("../policies/demo.yaml"))?;
    let wire_proof = SyntheticWireProof::unmeasured();
    let fake_state = Arc::new(CodexDemoFakeState::new(
        wire_proof.clone(),
        response_delay,
        local_session_token.as_bytes(),
    ));
    let fake_app = Router::new()
        .route("/v1/models", get(codex_demo_models))
        .route("/v1/responses", post(codex_demo_responses))
        .with_state(fake_state.clone());
    let fake_listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|_| CodexDemoError::LoopbackBind)?;
    let fake_address = fake_listener
        .local_addr()
        .map_err(|_| CodexDemoError::LoopbackBind)?;
    let gateway_listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|_| CodexDemoError::LoopbackBind)?;
    let gateway_address = gateway_listener
        .local_addr()
        .map_err(|_| CodexDemoError::LoopbackBind)?;
    let fake_url =
        Url::parse(&format!("http://{fake_address}")).map_err(|_| CodexDemoError::LoopbackUrl)?;
    let gateway_app = build_router(GatewayConfig {
        bind: gateway_address,
        policy,
        scope,
        local_session_token: local_session_token.clone(),
        audit_path: runtime.audit_path.clone(),
        upstream: UpstreamMode::loopback_test(fake_url)?,
        client: GatewayClient::CodexCli,
        restoration_mode: RestorationMode::SyntheticDisplayDeltaOnly,
        synthetic_wire_proof: wire_proof,
    })?;
    let (fake_shutdown_tx, fake_shutdown_rx) = oneshot::channel::<()>();
    let fake_task = tokio::spawn(async move {
        axum::serve(fake_listener, fake_app)
            .with_graceful_shutdown(async {
                let _ = fake_shutdown_rx.await;
            })
            .await
    });
    let (gateway_shutdown_tx, gateway_shutdown_rx) = oneshot::channel::<()>();
    let gateway_task = tokio::spawn(async move {
        axum::serve(gateway_listener, gateway_app)
            .with_graceful_shutdown(async {
                let _ = gateway_shutdown_rx.await;
            })
            .await
    });
    Ok(CodexDemoServers {
        gateway_address,
        local_session_token,
        fake_state,
        fake_shutdown_tx,
        gateway_shutdown_tx,
        fake_task,
        gateway_task,
    })
}

async fn codex_demo_models() -> Result<Json<Value>, StatusCode> {
    serde_json::from_str(CODEX_DEMO_MODEL_CATALOG)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn codex_demo_responses(
    State(state): State<Arc<CodexDemoFakeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    state.request_count.fetch_add(1, Ordering::SeqCst);
    let session_token_in_headers = headers
        .values()
        .any(|value| blake3::hash(value.as_bytes()) == state.session_token_hash);
    let session_token_in_body =
        contains_hashed_value(&body, state.session_token_len, &state.session_token_hash);
    if headers.contains_key("authorization")
        || headers.contains_key(LOCAL_SESSION_HEADER)
        || session_token_in_headers
        || session_token_in_body
    {
        state.forbidden_header_seen.store(true, Ordering::SeqCst);
        state.wire_proof.mark_failed();
        return codex_demo_rejection();
    }
    if contains_bytes(&body, SYNTHETIC_EMAIL.as_bytes())
        || contains_bytes(&body, SYNTHETIC_PRIVATE_IP.as_bytes())
    {
        state.wire_proof.mark_failed();
        return codex_demo_rejection();
    }
    let Ok(request) = serde_json::from_slice::<Value>(&body) else {
        state.wire_proof.mark_failed();
        return codex_demo_rejection();
    };
    if codex_demo_contains_original(&request) {
        state.wire_proof.mark_failed();
        return codex_demo_rejection();
    }
    let Some((email_token, private_ip_token)) = codex_demo_tokens(&request) else {
        state.wire_proof.mark_failed();
        return codex_demo_rejection();
    };
    let tokenized_text = format!(
        "const SUPPORT_EMAIL = \"{email_token}\";\nconst BUILD_HOST = \"{private_ip_token}\";"
    );
    let Ok(sse) = codex_demo_sse(&tokenized_text) else {
        state.wire_proof.mark_failed();
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let response_delay = state.response_delay;
    let body = Body::from_stream(async_stream::stream! {
        if !response_delay.is_zero() {
            tokio::time::sleep(response_delay).await;
        }
        yield Ok::<Bytes, io::Error>(Bytes::from(sse));
    });
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream; charset=utf-8")
        .header("cache-control", "no-store")
        .body(body);
    match response {
        Ok(response) => {
            state.wire_proof.mark_passed();
            response
        }
        Err(_) => {
            state.wire_proof.mark_failed();
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn codex_demo_rejection() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        "AgentVeil synthetic fixture rejected the request.",
    )
        .into_response()
}

fn codex_demo_tokens(request: &Value) -> Option<(String, String)> {
    let mut email_tokens = BTreeSet::new();
    let mut private_ip_tokens = BTreeSet::new();
    collect_codex_demo_tokens(request, &mut email_tokens, &mut private_ip_tokens);
    if email_tokens.len() != 1 || private_ip_tokens.len() != 1 {
        return None;
    }
    Some((
        email_tokens.into_iter().next()?,
        private_ip_tokens.into_iter().next()?,
    ))
}

fn codex_demo_contains_original(value: &Value) -> bool {
    match value {
        Value::String(text) => {
            text.contains(SYNTHETIC_EMAIL) || text.contains(SYNTHETIC_PRIVATE_IP)
        }
        Value::Array(values) => values.iter().any(codex_demo_contains_original),
        Value::Object(values) => values.iter().any(|(key, value)| {
            key.contains(SYNTHETIC_EMAIL)
                || key.contains(SYNTHETIC_PRIVATE_IP)
                || codex_demo_contains_original(value)
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn collect_codex_demo_tokens(
    value: &Value,
    email_tokens: &mut BTreeSet<String>,
    private_ip_tokens: &mut BTreeSet<String>,
) {
    match value {
        Value::String(text) => {
            collect_tokens_with_prefix(text, "[AV_EMAIL_", email_tokens);
            collect_tokens_with_prefix(text, "[AV_IPV4_", private_ip_tokens);
        }
        Value::Array(values) => {
            for value in values {
                collect_codex_demo_tokens(value, email_tokens, private_ip_tokens);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                collect_tokens_with_prefix(key, "[AV_EMAIL_", email_tokens);
                collect_tokens_with_prefix(key, "[AV_IPV4_", private_ip_tokens);
                collect_codex_demo_tokens(value, email_tokens, private_ip_tokens);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn collect_tokens_with_prefix(text: &str, prefix: &str, tokens: &mut BTreeSet<String>) {
    const RANDOM_HEX_LENGTH: usize = 32;
    for (start, _) in text.match_indices(prefix) {
        let end = start + prefix.len() + RANDOM_HEX_LENGTH + 1;
        let Some(candidate) = text.get(start..end) else {
            continue;
        };
        let hex_start = prefix.len();
        let hex_end = hex_start + RANDOM_HEX_LENGTH;
        if candidate.ends_with(']')
            && candidate[hex_start..hex_end]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
        {
            tokens.insert(candidate.to_string());
        }
    }
}

fn codex_demo_sse(tokenized_text: &str) -> Result<String, serde_json::Error> {
    let response_id = "resp_agentveil_synthetic";
    let item_id = "msg_agentveil_synthetic";
    let empty_item = json!({
        "id": item_id,
        "type": "message",
        "role": "assistant",
        "content": []
    });
    let output_part = json!({"type": "output_text", "text": tokenized_text});
    let completed_item = json!({
        "id": item_id,
        "type": "message",
        "role": "assistant",
        "content": [output_part.clone()]
    });
    let events = [
        json!({
            "type": "response.created",
            "sequence_number": 0,
            "response": {"id": response_id, "output": []}
        }),
        json!({
            "type": "response.output_item.added",
            "sequence_number": 1,
            "output_index": 0,
            "item": empty_item
        }),
        json!({
            "type": "response.content_part.added",
            "sequence_number": 2,
            "item_id": item_id,
            "output_index": 0,
            "content_index": 0,
            "part": {"type": "output_text", "text": ""}
        }),
        json!({
            "type": "response.output_text.delta",
            "sequence_number": 3,
            "item_id": item_id,
            "output_index": 0,
            "content_index": 0,
            "delta": tokenized_text
        }),
        json!({
            "type": "response.output_text.done",
            "sequence_number": 4,
            "item_id": item_id,
            "output_index": 0,
            "content_index": 0,
            "text": tokenized_text
        }),
        json!({
            "type": "response.content_part.done",
            "sequence_number": 5,
            "item_id": item_id,
            "output_index": 0,
            "content_index": 0,
            "part": output_part
        }),
        json!({
            "type": "response.output_item.done",
            "sequence_number": 6,
            "output_index": 0,
            "item": completed_item.clone()
        }),
        json!({
            "type": "response.completed",
            "sequence_number": 7,
            "response": {
                "id": response_id,
                "status": "completed",
                "output": [completed_item]
            }
        }),
    ];
    let mut sse = String::new();
    for event in events {
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("response.failed");
        sse.push_str("event: ");
        sse.push_str(event_type);
        sse.push_str("\ndata: ");
        sse.push_str(&serde_json::to_string(&event)?);
        sse.push_str("\n\n");
    }
    Ok(sse)
}

async fn codex_demo_check(
    gateway_address: SocketAddr,
    local_session_token: &str,
    fake_state: &CodexDemoFakeState,
    audit_path: &Path,
) -> Result<(), CodexDemoError> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(10))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| CodexDemoError::Client)?;
    let responses_url = format!("http://{gateway_address}/v1/responses");
    let hard_secret = format!("{}{}", "sk-proj-", "A1b2C3d4E5f6G7h8I9j0");
    let blocked = client
        .post(&responses_url)
        .header(LOCAL_SESSION_HEADER, local_session_token)
        .json(&codex_request(&hard_secret, false))
        .send()
        .await
        .map_err(|_| CodexDemoError::Client)?;
    if blocked.status() != StatusCode::UNPROCESSABLE_ENTITY
        || fake_state.request_count.load(Ordering::SeqCst) != 0
        || fake_state.wire_proof.label() != "not_measured"
    {
        return Err(CodexDemoError::Check("hard_secret_block"));
    }

    let first = client
        .post(&responses_url)
        .header(LOCAL_SESSION_HEADER, local_session_token)
        .json(&codex_request(CODEX_DEMO_PROMPT, false))
        .send()
        .await
        .map_err(|_| CodexDemoError::Client)?;
    if first.status() != StatusCode::OK {
        return Err(CodexDemoError::Check("first_status"));
    }
    let first = first.text().await.map_err(|_| CodexDemoError::Client)?;
    if first.matches(SYNTHETIC_EMAIL).count() != 1
        || first.matches(SYNTHETIC_PRIVATE_IP).count() != 1
    {
        return Err(CodexDemoError::Check("delta_restoration"));
    }
    let first_json = Value::String(first.clone());
    let Some((email_token, private_ip_token)) = codex_demo_tokens(&first_json) else {
        return Err(CodexDemoError::Check("snapshot_tokens"));
    };
    let replay = format!(
        "Previous protected response: const SUPPORT_EMAIL = \"{email_token}\"; const BUILD_HOST = \"{private_ip_token}\"; {CODEX_DEMO_REPLAY_PROMPT}"
    );
    let second = client
        .post(&responses_url)
        .header(LOCAL_SESSION_HEADER, local_session_token)
        .json(&codex_request(&replay, false))
        .send()
        .await
        .map_err(|_| CodexDemoError::Client)?;
    if second.status() != StatusCode::OK {
        return Err(CodexDemoError::Check("replay_status"));
    }
    let second = second.text().await.map_err(|_| CodexDemoError::Client)?;
    if second.matches(SYNTHETIC_EMAIL).count() != 1
        || second.matches(SYNTHETIC_PRIVATE_IP).count() != 1
        || fake_state.request_count.load(Ordering::SeqCst) != 2
    {
        return Err(CodexDemoError::Check("token_replay"));
    }

    let dashboard = client
        .get(format!("http://{gateway_address}/dashboard/state"))
        .send()
        .await
        .map_err(|_| CodexDemoError::Client)?;
    let dashboard = dashboard.text().await.map_err(|_| CodexDemoError::Client)?;
    let dashboard_json: Value =
        serde_json::from_str(&dashboard).map_err(|_| CodexDemoError::Check("dashboard_shape"))?;
    if !dashboard.contains("\"wire_proof\":\"synthetic_passed\"")
        || dashboard_json.get("client").and_then(Value::as_str) != Some("codex_cli")
        || dashboard.contains(SYNTHETIC_EMAIL)
        || dashboard.contains(SYNTHETIC_PRIVATE_IP)
        || dashboard.contains("[AV_")
    {
        return Err(CodexDemoError::Check("dashboard_proof"));
    }
    let audit = fs::read_to_string(audit_path).map_err(|_| CodexDemoError::AuditRead)?;
    if audit.contains(SYNTHETIC_EMAIL)
        || audit.contains(SYNTHETIC_PRIVATE_IP)
        || audit.contains(&hard_secret)
        || audit.contains("[AV_")
        || audit.contains(local_session_token)
        || fake_state.forbidden_header_seen.load(Ordering::SeqCst)
    {
        return Err(CodexDemoError::Check("value_free_evidence"));
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> Result<(), CodexDemoError> {
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder.create(path).map_err(|_| CodexDemoError::Runtime)?;
    Ok(())
}

fn write_private_file(path: &Path, content: &[u8]) -> Result<(), CodexDemoError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    let mut file = options.open(path).map_err(|_| CodexDemoError::Runtime)?;
    std::io::Write::write_all(&mut file, content).map_err(|_| CodexDemoError::Runtime)
}

async fn stop_codex_demo_server(
    server: &mut JoinHandle<Result<(), io::Error>>,
) -> Result<(), CodexDemoError> {
    match tokio::time::timeout(SERVER_SHUTDOWN_TIMEOUT, &mut *server).await {
        Ok(result) => validate_codex_demo_server_result(result),
        Err(_) => {
            server.abort();
            let _ = server.await;
            Err(CodexDemoError::ShutdownTimeout)
        }
    }
}

fn validate_codex_demo_server_result(
    result: Result<Result<(), io::Error>, tokio::task::JoinError>,
) -> Result<(), CodexDemoError> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err(CodexDemoError::ServerRuntime),
        Err(_) => Err(CodexDemoError::ServerTask),
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
        || dashboard_json.get("client").and_then(Value::as_str) != Some("generic")
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

fn contains_hashed_value(haystack: &[u8], needle_len: usize, expected: &blake3::Hash) -> bool {
    needle_len != 0
        && haystack.len() >= needle_len
        && haystack
            .windows(needle_len)
            .any(|window| blake3::hash(window) == *expected)
}

fn private_runtime_path() -> Result<PathBuf, DemoError> {
    private_runtime_path_named("agentveil-demo")
}

fn private_runtime_path_named(prefix: &str) -> Result<PathBuf, DemoError> {
    let mut random = [0_u8; 12];
    getrandom::fill(&mut random).map_err(|_| DemoError::RandomUnavailable)?;
    let mut suffix = String::with_capacity(random.len() * 2);
    for byte in random {
        use std::fmt::Write as _;
        write!(&mut suffix, "{byte:02x}").map_err(|_| DemoError::RandomUnavailable)?;
    }
    Ok(std::env::temp_dir().join(format!("{prefix}-{suffix}")))
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
pub enum CodexDemoError {
    #[error("synthetic Codex demo private runtime could not be created")]
    Runtime,
    #[error("synthetic Codex demo secure randomness is unavailable")]
    RandomUnavailable,
    #[error("synthetic Codex demo loopback listener could not bind")]
    LoopbackBind,
    #[error("synthetic Codex demo loopback URL could not be constructed")]
    LoopbackUrl,
    #[error("synthetic Codex demo client initialization or request failed")]
    Client,
    #[error("synthetic Codex demo audit could not be read")]
    AuditRead,
    #[error("synthetic Codex demo private runtime cleanup failed")]
    RuntimeCleanup,
    #[error("synthetic Codex demo check failed: {0}")]
    Check(&'static str),
    #[error("synthetic Codex demo could not launch the pinned Codex CLI")]
    CodexLaunch,
    #[error("synthetic Codex demo could not read the Codex CLI process status")]
    CodexWait,
    #[error("synthetic Codex demo Codex CLI process exited unsuccessfully")]
    CodexFailed,
    #[error("synthetic Codex demo server task failed")]
    ServerTask,
    #[error("synthetic Codex demo server runtime failed")]
    ServerRuntime,
    #[error("synthetic Codex demo server stopped unexpectedly")]
    ServerStopped,
    #[error("synthetic Codex demo server shutdown timed out")]
    ShutdownTimeout,
    #[error("synthetic Codex demo policy is invalid")]
    Policy(#[from] PolicyError),
    #[error("synthetic Codex demo session scope is invalid")]
    Scope(#[from] LedgerError),
    #[error("synthetic Codex demo gateway could not be configured")]
    Gateway(#[from] GatewayError),
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use axum::Json;
    use axum::body::Bytes;
    use axum::extract::State;
    use axum::http::{HeaderMap, HeaderValue, StatusCode};
    use serde_json::Value;

    use super::{
        CODEX_DEMO_MODEL, CODEX_DEMO_PROMPT, CodexDemoFakeState, LOCAL_SESSION_HEADER,
        codex_demo_models, codex_demo_responses, codex_request,
    };
    use crate::gateway::SyntheticWireProof;

    fn request_bytes(text: &str) -> Result<Bytes, &'static str> {
        serde_json::to_vec(&codex_request(text, false))
            .map(Bytes::from)
            .map_err(|_| "synthetic request should serialize")
    }

    fn fake_state(proof: SyntheticWireProof) -> Arc<CodexDemoFakeState> {
        Arc::new(CodexDemoFakeState::new(
            proof,
            Duration::ZERO,
            b"0123456789ABCDEF0123456789ABCDEF",
        ))
    }

    fn tokenized_request() -> String {
        format!(
            "const SUPPORT_EMAIL = \"[AV_EMAIL_{}]\"; const BUILD_HOST = \"[AV_IPV4_{}]\";",
            "A".repeat(32),
            "B".repeat(32)
        )
    }

    #[tokio::test]
    async fn codex_demo_models_route_returns_complete_fixture_metadata() -> Result<(), &'static str>
    {
        let Json(catalog) = codex_demo_models()
            .await
            .map_err(|_| "synthetic model catalog should be valid")?;
        let model = catalog
            .get("models")
            .and_then(Value::as_array)
            .and_then(|models| models.first())
            .ok_or("synthetic model catalog should contain one model")?;
        assert_eq!(
            model.get("slug").and_then(Value::as_str),
            Some(CODEX_DEMO_MODEL)
        );
        for required in [
            "display_name",
            "supported_reasoning_levels",
            "shell_type",
            "visibility",
            "base_instructions",
            "truncation_policy",
            "context_window",
        ] {
            assert!(model.get(required).is_some(), "missing {required}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn codex_fake_boundary_failure_is_sticky_after_a_pass() -> Result<(), &'static str> {
        let proof = SyntheticWireProof::unmeasured();
        let state = fake_state(proof.clone());
        let safe_body = request_bytes(&tokenized_request())?;

        let passed =
            codex_demo_responses(State(state.clone()), HeaderMap::new(), safe_body.clone()).await;
        assert_eq!(passed.status(), StatusCode::OK);
        assert_eq!(proof.label(), "synthetic_passed");

        let rejected = codex_demo_responses(
            State(state.clone()),
            HeaderMap::new(),
            request_bytes(CODEX_DEMO_PROMPT)?,
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(proof.label(), "synthetic_failed");

        let later_pass = codex_demo_responses(State(state), HeaderMap::new(), safe_body).await;
        assert_eq!(later_pass.status(), StatusCode::OK);
        assert_eq!(proof.label(), "synthetic_failed");
        Ok(())
    }

    #[tokio::test]
    async fn codex_fake_marks_every_observable_boundary_rejection_failed()
    -> Result<(), &'static str> {
        let malformed = Bytes::from_static(b"{");
        let missing_tokens = request_bytes("synthetic safe text without protected tokens")?;
        let invalid_tokens = request_bytes(
            "const SUPPORT_EMAIL = \"[AV_EMAIL_SHORT]\"; const BUILD_HOST = \"[AV_IPV4_SHORT]\";",
        )?;
        let escaped_original = String::from_utf8(request_bytes(CODEX_DEMO_PROMPT)?.to_vec())
            .map_err(|_| "synthetic request should be UTF-8")?
            .replace('@', "\\u0040")
            .replace('.', "\\u002e");

        for body in [
            malformed,
            missing_tokens,
            invalid_tokens,
            Bytes::from(escaped_original),
        ] {
            let proof = SyntheticWireProof::unmeasured();
            let response =
                codex_demo_responses(State(fake_state(proof.clone())), HeaderMap::new(), body)
                    .await;
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(proof.label(), "synthetic_failed");
        }

        for header_name in ["authorization", LOCAL_SESSION_HEADER] {
            let proof = SyntheticWireProof::unmeasured();
            let mut headers = HeaderMap::new();
            headers.insert(header_name, HeaderValue::from_static("synthetic-canary"));
            let response = codex_demo_responses(
                State(fake_state(proof.clone())),
                headers,
                request_bytes(&tokenized_request())?,
            )
            .await;
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(proof.label(), "synthetic_failed");
        }
        Ok(())
    }
}
