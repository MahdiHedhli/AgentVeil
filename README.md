# AgentVeil

**Keep supported sensitive data out of Codex model context.**

AgentVeil is a local privacy harness for the Codex CLI. It places a narrow,
authenticated gateway between one verified Codex client and the OpenAI
Responses API. Before any supported request body is sent upstream, AgentVeil
parses the complete JSON payload, rejects unsupported shapes, blocks supported
credential classes, and masks or tokenizes configured lower-risk values.

> AgentVeil is pre-release hackathon software, not a universal DLP product or a
> production security boundary. The current evidence applies only to Codex CLI
> `0.144.4`, model `gpt-5.6-luna`, HTTP Responses/SSE, and the payload shapes in
> [the payload map](docs/PAYLOAD_MAP.md). Use synthetic data only.

## Evidence snapshot

The core route claims were first anchored on July 14, 2026. Release `v0.1.4`
includes the deterministic offline demo, value-free dashboard, hardened audit
sink, benchmark harness, release evidence gates, and a self-contained judge
archive with its synthetic live-route fixture.

- The secure launcher routes Codex CLI `0.144.4` and `gpt-5.6-luna` through an
  ephemeral loopback provider without changing persistent Codex configuration.
- The gateway buffers and validates the complete supported request before
  creating the upstream Responses request.
- Supported credentials, JWTs, credentialed database URLs, and PEM private-key
  material block locally. The deterministic wire test proves the fake upstream
  connection count does not increase for blocked and rejected requests.
- The default policy masks email, phone, home-path, and custom-term findings;
  tokenizes private IPv4 and configured internal-hostname findings; and never
  puts hard-secret classes in the token ledger.
- Message text, instructions, function/custom-tool inputs and outputs, and the
  other explicitly mapped text fields are inspected. Unknown top-level fields,
  duplicate JSON keys, media, unsupported content parts, compressed bodies, and
  unsafe structural findings fail closed.
- Audit records contain typed metadata rather than body values, mappings,
  tokens, authorization material, or response text. Audit persistence is a
  pre-egress gate for protected Responses requests.
- A synthetic live run exercised Codex tool output on a next GPT-5.6 Luna turn.
  The audit recorded rewritten email and private-IPv4 finding metadata without
  body values or tokens. This is route evidence; the capturing fake-upstream
  demo remains the wire-boundary proof.
- The read-only dashboard serves only typed enforcement metadata from loopback,
  requires the exact loopback authority on every dashboard request, uses local
  assets and restrictive browser headers, and exposes no browser credential.
  Its synthetic proof state is set only by the capturing demo harness.
- Release `v0.1.4` passes 42 Rust tests: 37 library tests, 3 launcher tests, 1
  offline-demo CLI test, and 1 gateway wire-level integration test. Three
  focused Python regressions cover exact archive packaging, report-directory
  failure handling, and descriptor cleanup.

Live OpenAI response restoration is deliberately disabled. Exact-token
restoration exists only for the loopback synthetic-test mode while local Codex
transcript persistence remains unverified.

See [STATUS.md](STATUS.md) for the claim matrix and remaining submission work.

## How it works

```text
Codex CLI 0.144.4
  -> random, invocation-scoped loopback provider
  -> AgentVeil authenticated Responses gateway
  -> parse + validate + normalize + detect + resolve policy
       -> block/reject: no upstream request body
       -> allow/rewrite: value-free audit record persisted first
  -> fixed OpenAI Responses endpoint with allowlisted headers
  -> Responses SSE returned to Codex (live restoration disabled)
```

The same loopback listener serves `/dashboard`. Its read-only routes are
deliberately unauthenticated so no gateway credential enters browser state; the
JSON contains only typed, value-free status and activity metadata. Exact Host
validation prevents a browser from reaching those routes through another
authority.

The launcher pins the verified Codex executable, model, wire format, and
provider configuration. It rejects passthrough options that could change model
routing, use an unverified transport, or bypass the generated provider. It also
removes the local gateway credential from shell environments created by Codex.

## Safe quick start

Prerequisites and exact installation steps are in [docs/INSTALL.md](docs/INSTALL.md).
After building, run the offline proof before contacting OpenAI:

```sh
agentveil demo --check
agentveil demo
```

The second command prints a loopback dashboard URL and remains active until
Ctrl-C. It uses only embedded synthetic fixtures and a capturing loopback fake
upstream. For the separately scoped live route:

```sh
agentveil doctor
agentveil policy-validate policies/default.yaml
agentveil codex --reasoning-effort none -- \
  exec --ephemeral --skip-git-repo-check \
  'Reply with ROUTE_OK. This prompt contains synthetic data only.'
```

`agentveil doctor` verifies an external executable, version, and login-status
metadata without printing its path or reading credential contents. The `codex`
command uses normal Codex login state in-process and does not persist a bearer
token.

Never test this pre-release build with a real credential or personal record. A
deterministic fake-upstream demonstration is documented in
[docs/DEMO.md](docs/DEMO.md).

## Judge-ready release

The [latest GitHub release](https://github.com/MahdiHedhli/AgentVeil/releases/latest)
publishes prebuilt, SHA-256-checksummed archives for Ubuntu 24.04 x86_64 and
macOS 14+ arm64. The offline proof requires no source build, Codex login, API
key, or OpenAI request. Follow [the judge test path](docs/JUDGE_TEST.md) and run:

```sh
./agentveil demo --check
```

The separately scoped live Codex route was verified on macOS 26.4.1 arm64 with
Codex CLI exactly `0.144.4`. Release archives are pre-release judge artifacts,
not signed/notarized production packages.

## Built with Codex and GPT-5.6

Codex accelerated repository analysis, payload-seam tracing, Rust
implementation, parallel adversarial review, browser QA, and repetitive release
verification. The human decisions remained explicit: choose a narrow custom-
provider seam instead of TLS interception, fail closed on unknown model-visible
shapes, block rather than tokenize S0 material, keep live restoration disabled,
and require wire evidence before a privacy claim.

GPT-5.6 Luna is the model on the dated live protected workflow, not a label on a
separate demo call. In that synthetic validation, Codex read a local tool
fixture, AgentVeil protected the resulting tool output on the next Responses
turn, and GPT-5.6 completed the guarded turn. The offline judge path uses a fake
upstream so anyone can verify the security boundary without credentials.

## Policy classes

| Class | Examples in the current detector set | Default action | Restorable |
|---|---|---|---:|
| Hard block | supported API credentials, JWTs, credentialed database URLs, PEM private keys, unknown AgentVeil tokens | block whole request | no |
| Mask | email, contextual phone, home path, configured custom term | replace with typed redaction | no |
| Tokenize | private IPv4, configured internal hostname | replace with 128-bit random session token | synthetic display mode only |

Policy validation mechanically forbids raw logging, requires unknown-sensitive
findings to block, and prevents hard-block classes from being tokenized or
restored. See [docs/PRIVACY.md](docs/PRIVACY.md) and the shipped YAML policies
for the precise behavior.

## Deliberate non-claims

AgentVeil does not currently claim protection for Codex IDE or cloud tasks,
Responses WebSockets, browser ChatGPT, other clients or providers, images/OCR,
files or binary bodies, compressed requests, arbitrary future payload shapes,
every encoding, every sensitive-data class, or a compromised local machine.
Detection can produce false positives and false negatives. Full limitations are
in [docs/LIMITATIONS.md](docs/LIMITATIONS.md).

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Privacy and retention](docs/PRIVACY.md)
- [Supported payload map](docs/PAYLOAD_MAP.md)
- [Installation](docs/INSTALL.md)
- [Judge test path](docs/JUDGE_TEST.md)
- [Deterministic and live demo](docs/DEMO.md)
- [Known limitations](docs/LIMITATIONS.md)
- [Risk register](docs/RISK_REGISTER.md)
- [Donor attribution](docs/DONOR_ATTRIBUTION.md)

## Development verification

```sh
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked --all-targets
scripts/demo-check
```

The repository forbids unsafe Rust and denies common panic/debug placeholders
through project lint settings.

## License and security reports

AgentVeil is Apache-2.0. Donor licenses and revisions are recorded in
[docs/DONOR_ATTRIBUTION.md](docs/DONOR_ATTRIBUTION.md). Report suspected
vulnerabilities privately using [SECURITY.md](SECURITY.md), with synthetic
reproduction data only.
