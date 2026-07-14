# Installation

Evidence anchor: commit `ed09354`. AgentVeil is source-distributed pre-release
software. There is no signed package at this anchor.

## Requirements

- a Unix-like host (the current secure file/executable checks use Unix APIs);
- Git;
- a current stable Rust toolchain with Cargo and Rust 2024 edition support;
- Codex CLI exactly `0.144.4`; and
- a valid normal Codex login for the live route.

Do not disable the exact Codex version check to make another release work. A
client upgrade changes the protocol evidence boundary and requires a refreshed
payload map, fake-upstream proof, and live synthetic proof.

## Build from the public repository

```sh
git clone https://github.com/MahdiHedhli/AgentVeil.git
cd AgentVeil
git rev-parse --short HEAD
cargo build --release --locked
```

For the documented evidence snapshot, the short revision begins with
`ed09354`. Later release commits should publish their own evidence anchor.

The binary is created at `target/release/agentveil`. The shipped policy paths
are relative to the current directory, so the simplest pre-release workflow is
to run it from the repository root:

```sh
./target/release/agentveil --version
./target/release/agentveil policy-validate policies/default.yaml
```

If you copy/install the binary elsewhere, pass an absolute `--policy` path and
an explicit private `--audit` path. `cargo install --path . --locked` is also
supported by Cargo, but it does not install the repository’s policy files.

## Verify the local Codex prerequisite

```sh
codex --version
codex login status
./target/release/agentveil doctor
```

Expected Codex version output is either `codex-cli 0.144.4` or
`codex 0.144.4`. `agentveil doctor`:

- resolves the first executable Codex binary outside the current workspace;
- verifies the exact version;
- asks Codex only for login status;
- verifies loopback binding availability; and
- prints executable/version/status metadata, never credential contents.

If Codex is missing, logged out, or a different version, use the official Codex
installation/login flow and rerun `doctor`. Do not copy authentication files or
put an access token in AgentVeil configuration.

## Verify the source before live use

```sh
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
```

The current build passes 38 tests. The gateway integration test is fully local
and synthetic; it does not contact OpenAI.

## Start a protected Codex invocation

From the repository root:

```sh
./target/release/agentveil codex \
  --reasoning-effort none \
  --audit "$HOME/.local/state/agentveil/audit.jsonl" \
  -- \
  exec --ephemeral --skip-git-repo-check \
  'Reply with ROUTE_OK. This prompt contains synthetic data only.'
```

For an interactive Codex session, omit the arguments after `--`:

```sh
./target/release/agentveil codex \
  --audit "$HOME/.local/state/agentveil/audit.jsonl"
```

The launcher defaults to an ephemeral `127.0.0.1` port, the default policy,
`gpt-5.6-luna`, and medium reasoning effort. It generates the local session
credential and provider overlay automatically. It does not modify persistent
Codex configuration. While the session runs, open
`http://<binding-from-the-status-line>/dashboard` locally for the read-only
value-free view. Do not port-forward or reverse-proxy the listener.

Use synthetic content only. This build is not approved for real credentials or
personal/customer records.

## Audit-path guidance

Choose a local directory controlled by the current user and outside shared,
cloud-synchronized, or web-served paths. The sink makes its direct parent mode
`0700` and the file mode `0600`, and rejects a permissive existing file. It does
not rotate or delete the file.

The repository default `runtime/agentveil.audit.jsonl` is Git-ignored. An
absolute state path is preferable when launching from arbitrary directories.
Do not point audit output at a donor repository or include it in a demo capture.

## Direct `serve` command

`agentveil serve` is an advanced integration/test surface. It requires
`AGENTVEIL_SESSION_TOKEN` and `AGENTVEIL_SESSION_SCOPE` to already exist in the
environment. Manually managing those values can expose them through shell
history or process tooling; use `agentveil codex` for the supported live path.

`--test-upstream` accepts only an unauthenticated loopback HTTP URL.
`--restore-display-text` requires that synthetic upstream. The gateway refuses
demo policy or restoration with live OpenAI.

## Troubleshooting

| Error | Meaning / safe action |
|---|---|
| `currently requires ... 0.144.4` | install the verified Codex release or wait for a refreshed AgentVeil compatibility proof |
| `authentication is unavailable` | complete normal `codex login`; do not paste tokens into AgentVeil |
| `routing-bypass mode` | remove model/provider/profile/remote/resume/fork/cloud/server routing arguments |
| `audit file permissions are too broad` | move to a private file or restrict it to the current user before retrying |
| `could not safely inspect` | payload shape or detector failed closed; do not bypass the check |
| `audit sink is unavailable` | restore local disk/path health; protected egress remains blocked |
| `blocked ... before it reached the remote model` | remove the synthetic credential-shaped value or replace it with a safe non-secret reference |

Do not solve a fail-closed error by weakening the policy, bypassing the launcher,
or routing Codex around AgentVeil. Record an unsupported shape and add a typed
adapter/test before expanding the claim.
