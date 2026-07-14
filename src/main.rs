use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use agentveil::VERSION;
use agentveil::gateway::{GatewayConfig, UpstreamMode, build_router, serve};
use agentveil::ledger::SessionScope;
use agentveil::policy::ValidatedPolicy;
use clap::{Parser, Subcommand, ValueEnum};
use reqwest::Url;
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Parser)]
#[command(name = "agentveil", version = VERSION, about = "Keep sensitive data out of Codex context.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ReasoningEffort {
    None,
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Check the local Codex and AgentVeil prerequisites without reading credentials.
    Doctor,
    /// Validate a policy and print its value-free identity.
    PolicyValidate {
        #[arg(default_value = "policies/default.yaml")]
        policy: PathBuf,
    },
    /// Launch Codex through a temporary authenticated AgentVeil session.
    Codex {
        #[arg(long, default_value = "127.0.0.1:0")]
        bind: SocketAddr,
        #[arg(long, default_value = "policies/default.yaml")]
        policy: PathBuf,
        #[arg(long, default_value = "runtime/agentveil.audit.jsonl")]
        audit: PathBuf,
        #[arg(long, default_value = "gpt-5.6-luna")]
        model: String,
        #[arg(long, value_enum, default_value_t = ReasoningEffort::Medium)]
        reasoning_effort: ReasoningEffort,
        #[arg(last = true, allow_hyphen_values = true)]
        codex_args: Vec<String>,
    },
    /// Start the authenticated loopback Responses gateway.
    Serve {
        #[arg(long, default_value = "127.0.0.1:48741")]
        bind: SocketAddr,
        #[arg(long, default_value = "policies/default.yaml")]
        policy: PathBuf,
        #[arg(long, default_value = "runtime/agentveil.audit.jsonl")]
        audit: PathBuf,
        /// Synthetic-only upstream. Must be an unauthenticated loopback HTTP URL.
        #[arg(long)]
        test_upstream: Option<Url>,
        /// Restore exact issued tokens only in supported synthetic SSE display text.
        #[arg(long, requires = "test_upstream")]
        restore_display_text: bool,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter("agentveil=info")
        .init();
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("AgentVeil: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Doctor => doctor(),
        Commands::PolicyValidate { policy } => {
            let policy = load_policy(&policy)?;
            println!(
                "policy={} hash={} status=valid",
                policy.name(),
                policy.hash()
            );
            Ok(())
        }
        Commands::Codex {
            bind,
            policy,
            audit,
            model,
            reasoning_effort,
            codex_args,
        } => launch_codex(bind, policy, audit, model, reasoning_effort, codex_args).await,
        Commands::Serve {
            bind,
            policy,
            audit,
            test_upstream,
            restore_display_text,
        } => {
            let policy = load_policy(&policy)?;
            let local_session_token = required_secret_env("AGENTVEIL_SESSION_TOKEN")?;
            let scope = SessionScope::parse(required_env("AGENTVEIL_SESSION_SCOPE")?)?;
            let upstream = match test_upstream {
                Some(url) => UpstreamMode::loopback_test(url)?,
                None => UpstreamMode::OpenAi,
            };
            serve(GatewayConfig {
                bind,
                policy,
                scope,
                local_session_token,
                audit_path: audit,
                upstream,
                restore_display_text,
            })
            .await?;
            Ok(())
        }
    }
}

async fn launch_codex(
    bind: SocketAddr,
    policy_path: PathBuf,
    audit_path: PathBuf,
    model: String,
    reasoning_effort: ReasoningEffort,
    codex_args: Vec<String>,
) -> Result<(), CliError> {
    if !supported_model(&model) {
        return Err(CliError::UnsafeModelName);
    }
    reject_routing_overrides(&codex_args)?;
    if !bind.ip().is_loopback() {
        return Err(CliError::Gateway(
            agentveil::gateway::GatewayError::NonLoopbackBind,
        ));
    }
    let codex_path = resolve_codex()?;
    preflight_codex(&codex_path)?;
    let policy = load_policy(&policy_path)?;
    let local_session_token = random_hex(32)?;
    let session_scope = format!("avscope_{}", random_hex(16)?.as_str());
    let provider_id = format!(
        "agentveil-session-{}",
        random_hex(12)?.as_str().to_ascii_lowercase()
    );
    let display_hash = blake3::hash(session_scope.as_bytes()).to_hex().to_string();
    let display_session = format!("av_session_{}", &display_hash[..12]);
    let scope = SessionScope::parse(session_scope)?;
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|_| CliError::GatewayBind)?;
    let actual_bind = listener.local_addr().map_err(|_| CliError::GatewayBind)?;
    let router = build_router(GatewayConfig {
        bind: actual_bind,
        policy,
        scope,
        local_session_token: local_session_token.clone(),
        audit_path,
        upstream: UpstreamMode::OpenAi,
        restore_display_text: false,
    })?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let mut gateway_task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
    });

    let base_url = format!("http://{actual_bind}/v1");
    let provider_key = format!("model_providers.{provider_id}");
    let mut codex = tokio::process::Command::new(&codex_path);
    codex
        .arg("-m")
        .arg(&model)
        .arg("-c")
        .arg(format!("model_provider=\"{provider_id}\""))
        .arg("-c")
        .arg(format!("{provider_key}.name=\"AgentVeil\""))
        .arg("-c")
        .arg(format!("{provider_key}.base_url=\"{base_url}\""))
        .arg("-c")
        .arg(format!("{provider_key}.wire_api=\"responses\""))
        .arg("-c")
        .arg(format!("{provider_key}.requires_openai_auth=true"))
        .arg("-c")
        .arg(format!("{provider_key}.supports_websockets=false"))
        .arg("-c")
        .arg(format!(
            "{provider_key}.env_http_headers={{ \"X-AgentVeil-Session\" = \"AGENTVEIL_SESSION_TOKEN\" }}"
        ))
        .arg("-c")
        .arg(format!(
            "model_reasoning_effort=\"{}\"",
            reasoning_effort.as_str()
        ))
        .arg("-c")
        .arg("shell_environment_policy.ignore_default_excludes=false")
        .arg("-c")
        .arg("shell_environment_policy.set.AGENTVEIL_SESSION_TOKEN=\"\"")
        .args(codex_args)
        .env("AGENTVEIL_SESSION_TOKEN", local_session_token.as_str())
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true);
    println!(
        "AgentVeil session active: session={display_session} transport=responses_sse binding={actual_bind} model={model}"
    );
    let mut child = codex.spawn().map_err(|_| CliError::CodexLaunch)?;
    let status = tokio::select! {
        status = child.wait() => status.map_err(|_| CliError::CodexWait)?,
        gateway = &mut gateway_task => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            gateway.map_err(|_| CliError::GatewayTask)?
                .map_err(|_| CliError::GatewayRuntime)?;
            return Err(CliError::GatewayStopped);
        }
    };
    let _ = shutdown_tx.send(());
    match tokio::time::timeout(std::time::Duration::from_secs(5), &mut gateway_task).await {
        Ok(gateway_result) => gateway_result
            .map_err(|_| CliError::GatewayTask)?
            .map_err(|_| CliError::GatewayRuntime)?,
        Err(_) => {
            gateway_task.abort();
            let _ = gateway_task.await;
            return Err(CliError::GatewayShutdownTimeout);
        }
    }
    if !status.success() {
        return Err(CliError::CodexFailed);
    }
    Ok(())
}

fn random_hex(bytes: usize) -> Result<Zeroizing<String>, CliError> {
    let mut random = Zeroizing::new(vec![0_u8; bytes]);
    getrandom::fill(&mut random).map_err(|_| CliError::RandomUnavailable)?;
    let mut encoded = Zeroizing::new(String::with_capacity(bytes * 2));
    for byte in random.iter() {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02X}").map_err(|_| CliError::RandomUnavailable)?;
    }
    Ok(encoded)
}

fn supported_model(model: &str) -> bool {
    model == "gpt-5.6-luna"
}

fn reject_routing_overrides(arguments: &[String]) -> Result<(), CliError> {
    if arguments.iter().any(|argument| {
        matches!(
            argument.as_str(),
            "-c" | "--config"
                | "-m"
                | "--model"
                | "-p"
                | "--profile"
                | "--oss"
                | "--local-provider"
                | "--remote"
                | "--remote-auth-token-env"
                | "--ignore-user-config"
                | "resume"
                | "fork"
                | "cloud"
                | "app"
                | "app-server"
                | "remote-control"
                | "mcp-server"
                | "exec-server"
                | "sandbox"
                | "login"
                | "logout"
                | "update"
        ) || argument.starts_with("--config=")
            || argument.starts_with("--model=")
            || argument.starts_with("--profile=")
            || argument.starts_with("--oss=")
            || argument.starts_with("--local-provider=")
            || argument.starts_with("--remote=")
            || argument.starts_with("--remote-auth-token-env=")
            || (argument.starts_with("-c") && !argument.starts_with("--"))
            || (argument.starts_with("-m") && !argument.starts_with("--"))
            || (argument.starts_with("-p") && !argument.starts_with("--"))
    }) {
        return Err(CliError::RoutingOverride);
    }
    Ok(())
}

fn resolve_codex() -> Result<PathBuf, CliError> {
    let path = env::var_os("PATH").ok_or(CliError::CodexMissing)?;
    let workspace = env::current_dir()
        .ok()
        .and_then(|path| std::fs::canonicalize(path).ok());
    for directory in env::split_paths(&path) {
        let candidate = directory.join("codex");
        let Ok(canonical) = std::fs::canonicalize(&candidate) else {
            continue;
        };
        let Ok(metadata) = std::fs::metadata(&canonical) else {
            continue;
        };
        if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
            continue;
        }
        if workspace
            .as_ref()
            .is_some_and(|workspace| canonical.starts_with(workspace))
        {
            continue;
        }
        return Ok(canonical);
    }
    Err(CliError::CodexMissing)
}

fn codex_version(codex: &PathBuf) -> Result<String, CliError> {
    let output = Command::new(codex)
        .arg("--version")
        .output()
        .map_err(|_| CliError::CodexMissing)?;
    if !output.status.success() {
        return Err(CliError::CodexMissing);
    }
    let version = String::from_utf8(output.stdout).map_err(|_| CliError::CodexVersion)?;
    let version = version.trim().to_string();
    if !matches!(version.as_str(), "codex-cli 0.144.4" | "codex 0.144.4") {
        return Err(CliError::UnsupportedCodexVersion);
    }
    Ok(version)
}

fn preflight_codex(codex: &PathBuf) -> Result<String, CliError> {
    let version = codex_version(codex)?;
    let login = Command::new(codex)
        .args(["login", "status"])
        .output()
        .map_err(|_| CliError::CodexStatus)?;
    if !login.status.success() {
        return Err(CliError::CodexAuthUnavailable);
    }
    Ok(version)
}

fn load_policy(path: &PathBuf) -> Result<ValidatedPolicy, CliError> {
    let bytes = std::fs::read(path).map_err(|_| CliError::PolicyRead)?;
    ValidatedPolicy::from_yaml(&bytes).map_err(CliError::Policy)
}

fn required_env(name: &'static str) -> Result<String, CliError> {
    env::var(name).map_err(|_| CliError::MissingEnvironment(name))
}

fn required_secret_env(name: &'static str) -> Result<Zeroizing<String>, CliError> {
    required_env(name).map(Zeroizing::new)
}

fn doctor() -> Result<(), CliError> {
    let codex = resolve_codex()?;
    let version = preflight_codex(&codex)?;
    let loopback = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 48741);
    let port_status = if std::net::TcpListener::bind(loopback).is_ok() {
        "available"
    } else {
        "in_use"
    };
    println!(
        "agentveil={} codex={} auth=available executable={} bind=loopback port_48741={}",
        VERSION,
        version,
        codex.display(),
        port_status
    );
    Ok(())
}

#[derive(Debug, Error)]
enum CliError {
    #[error("policy file could not be read")]
    PolicyRead,
    #[error("policy is invalid")]
    Policy(#[source] agentveil::policy::PolicyError),
    #[error("required environment variable {0} is missing")]
    MissingEnvironment(&'static str),
    #[error("Codex CLI is not installed or did not report a version")]
    CodexMissing,
    #[error("Codex version output was invalid")]
    CodexVersion,
    #[error("Codex authentication status could not be checked")]
    CodexStatus,
    #[error("Codex authentication is unavailable")]
    CodexAuthUnavailable,
    #[error("AgentVeil currently requires the verified Codex CLI version 0.144.4")]
    UnsupportedCodexVersion,
    #[error("model name is unsafe")]
    UnsafeModelName,
    #[error("Codex passthrough arguments request an unverified or routing-bypass mode")]
    RoutingOverride,
    #[error("secure session randomness is unavailable")]
    RandomUnavailable,
    #[error("gateway listener could not bind to the requested loopback address")]
    GatewayBind,
    #[error("Codex could not be launched")]
    CodexLaunch,
    #[error("Codex process status could not be read")]
    CodexWait,
    #[error("Codex exited unsuccessfully")]
    CodexFailed,
    #[error("gateway task failed")]
    GatewayTask,
    #[error("gateway runtime failed")]
    GatewayRuntime,
    #[error("gateway stopped while Codex was still running")]
    GatewayStopped,
    #[error("gateway did not shut down within the bounded grace period")]
    GatewayShutdownTimeout,
    #[error("session scope is invalid")]
    Scope(#[from] agentveil::ledger::LedgerError),
    #[error("gateway configuration or runtime failed")]
    Gateway(#[from] agentveil::gateway::GatewayError),
}

#[cfg(test)]
mod tests {
    use super::{reject_routing_overrides, supported_model};

    #[test]
    fn rejects_every_supported_routing_override_form() {
        for argument in [
            "-c",
            "-cmodel_provider=\"other\"",
            "-c=model_provider=\"other\"",
            "--config",
            "--config=model_provider=\"other\"",
            "-m",
            "-mgpt-5.6",
            "--model",
            "--model=gpt-5.6",
            "-p",
            "-pother",
            "--profile",
            "--profile=other",
            "--oss",
            "--local-provider=ollama",
            "--remote",
            "--remote-auth-token-env=TOKEN",
            "--ignore-user-config",
            "resume",
            "fork",
            "cloud",
            "app-server",
            "mcp-server",
        ] {
            assert!(
                reject_routing_overrides(&[argument.to_string()]).is_err(),
                "override should be rejected: {argument}"
            );
        }
    }

    #[test]
    fn permits_non_routing_codex_arguments() {
        let arguments = [
            "--sandbox",
            "read-only",
            "exec",
            "--ephemeral",
            "--skip-git-repo-check",
            "prompt",
        ]
        .map(str::to_string);
        assert!(reject_routing_overrides(&arguments).is_ok());
    }

    #[test]
    fn model_name_is_a_narrow_identifier() {
        assert!(supported_model("gpt-5.6-luna"));
        assert!(!supported_model("gpt-5.6"));
        assert!(!supported_model("GPT-5.6"));
    }
}
