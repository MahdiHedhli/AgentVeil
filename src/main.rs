use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use agentveil::VERSION;
use agentveil::gateway::{GatewayConfig, UpstreamMode, serve};
use agentveil::ledger::SessionScope;
use agentveil::policy::ValidatedPolicy;
use clap::{Parser, Subcommand};
use reqwest::Url;
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Parser)]
#[command(name = "agentveil", version = VERSION, about = "Keep sensitive data out of Codex context.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
    let version = Command::new("codex")
        .arg("--version")
        .output()
        .map_err(|_| CliError::CodexMissing)?;
    if !version.status.success() {
        return Err(CliError::CodexMissing);
    }
    let version = String::from_utf8(version.stdout).map_err(|_| CliError::CodexVersion)?;
    let login = Command::new("codex")
        .args(["login", "status"])
        .output()
        .map_err(|_| CliError::CodexStatus)?;
    let auth_status = if login.status.success() {
        "available"
    } else {
        "unavailable"
    };
    let loopback = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 48741);
    let port_status = if std::net::TcpListener::bind(loopback).is_ok() {
        "available"
    } else {
        "in_use"
    };
    println!(
        "agentveil={} codex={} auth={} bind=loopback port_48741={}",
        VERSION,
        version.trim(),
        auth_status,
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
    #[error("session scope is invalid")]
    Scope(#[from] agentveil::ledger::LedgerError),
    #[error("gateway configuration or runtime failed")]
    Gateway(#[from] agentveil::gateway::GatewayError),
}
