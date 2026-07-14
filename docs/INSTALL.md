# Installation

Generated release reports bind evidence to their exact commit and clean-tree
observations. AgentVeil is pre-production hackathon software. Public GitHub
releases provide checksummed judge binaries, but no artifact is Apple-notarized
or a signed production package.

## Requirements

- a Unix-like host (the current secure file/executable checks use Unix APIs);
- Git;
- a current stable Rust toolchain with Cargo and Rust 2024 edition support;
- Codex CLI exactly `0.144.4`; and
- a valid normal Codex login for the live route.

The no-build offline judge proof supports Ubuntu 24.04 x86_64 and macOS 14+
arm64. The live protected Codex route is evidence-bounded to macOS 26.4.1 arm64
and Codex CLI exactly `0.144.4`. Other Unix source builds are not a verified
submission-platform claim.

## Test without rebuilding

Download a prebuilt archive and its SHA-256 file from the
[latest release](https://github.com/MahdiHedhli/AgentVeil/releases/latest), then
follow [JUDGE_TEST.md](JUDGE_TEST.md). The resulting one-command proof is:

```sh
./agentveil demo --check
```

It needs no Codex installation, login, API key, or OpenAI request.

Inside an extracted release archive the executable is `./agentveil`. Commands
below that use `./target/release/agentveil` apply after the source-build step.
The packaged [demo walkthrough](DEMO.md) selects the correct path for either
layout.

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

For a tagged release, verify that the checked-out revision matches the release
page and its documented evidence anchor before using the claims in this guide.

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
- prints version/status metadata without the executable path or credential
  contents.

If Codex is missing, logged out, or a different version, use the official Codex
installation/login flow and rerun `doctor`. Do not copy authentication files or
put an access token in AgentVeil configuration.

## Verify the source before live use

```sh
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked --all-targets
```

The current build passes 51 Rust tests: 43 library, 3 binary CLI, 2 demo CLI
integration, and 3 gateway end-to-end tests. Four additional Python
regressions exercise archive construction, release-version consistency, report
path failures, and descriptor cleanup. The gateway end-to-end tests and
offline demos are fully local and synthetic; none contacts OpenAI. Re-run and
bind these counts to the final clean commit before publishing a new tag.

Before using live login state, run the packaged proof:

```sh
./target/release/agentveil demo --check
./target/release/agentveil demo
```

The second command prints a loopback dashboard URL and remains active until
Ctrl-C.

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

## Start the isolated synthetic Codex round trip

To run the real pinned Codex CLI against AgentVeil's capturing loopback fixture:

```sh
./target/release/agentveil codex-demo --check
./target/release/agentveil codex-demo
```

The `--check` form uses a synthetic client to exercise the Codex-compatible
request/stream contract; it does not launch Codex and emits only value-free
evidence. The interactive form requires no Codex login and keeps its model
route loopback-only, so it sends no model request to OpenAI. This is not a
categorical no-network claim about the Codex process. It
creates a private temporary `CODEX_HOME` and synthetic-only workspace. The TUI
keeps a resumable thread there while it runs; this is not an ephemeral Codex
session. AgentVeil recursively removes the entire runtime and verifies its
absence on clean exit. The demo policy tokenizes the known-fake email and
private IPv4 fixtures. AgentVeil transforms exact owned tokens only in
`response.output_text.delta`; done/item/completion/response snapshots stay
tokenized on the wire. Open the printed dashboard URL locally to see the
value-free route, wire-proof, activity, and restoration state. Only the Codex
TUI intentionally displays the documented synthetic originals.

Never substitute real personal data. `codex-demo` is a bounded evidence mode,
not permission to enable restoration on the live OpenAI route. A forced kill
or host crash can leave the resumable thread, raw synthetic prompt, and
restored synthetic display in temporary state behind.

## Audit-path guidance

Choose a local directory controlled by the current user and outside shared,
cloud-synchronized, or web-served paths. An existing direct parent must already
be private (`0700` or stricter), non-symlink, and a directory. If only the final
dedicated leaf is missing, the sink creates it as `0700`; it never changes a
pre-existing directory's permissions. The audit file is `0600`, opened with
no-follow semantics, flushed, and synced before protected egress. The sink does
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
demo policy or restoration with live OpenAI. That lower-level test option uses
full synthetic display restoration for supported typed assistant display
copies; `codex-demo` instead selects the narrower delta-only mode.

## Troubleshooting

| Error | Meaning / safe action |
|---|---|
| `currently requires ... 0.144.4` | install the verified Codex release or wait for a refreshed AgentVeil compatibility proof |
| `authentication is unavailable` | complete normal `codex login`; do not paste tokens into AgentVeil |
| `routing-bypass mode` | remove model/provider/profile/remote/resume/fork/cloud/server routing arguments |
| `audit file permissions are too broad` | choose a private dedicated parent/file and restrict it yourself before retrying; AgentVeil will not chmod an existing path |
| `could not safely inspect` | payload shape or detector failed closed; do not bypass the check |
| `audit sink is unavailable` | restore local disk/path health; protected egress remains blocked |
| `blocked ... before it reached the remote model` | remove the synthetic credential-shaped value or replace it with a safe non-secret reference |

Do not solve a fail-closed error by weakening the policy, bypassing the launcher,
or routing Codex around AgentVeil. Record an unsupported shape and add a typed
adapter/test before expanding the claim.
