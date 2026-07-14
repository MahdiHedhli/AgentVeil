# AgentVeil — Devpost draft

> Draft basis: public release `v0.1.3`. Revalidate every claim against its
> clean release report before submission.

## Submission fields

- **Project name:** AgentVeil
- **Tagline:** Keep sensitive data out of Codex context.
- **Planned track:** Developer Tools
- **Repository:** [github.com/MahdiHedhli/AgentVeil](https://github.com/MahdiHedhli/AgentVeil) — public
- **Public YouTube demo:** `<ADD PUBLIC YOUTUBE URL>`
- **Demo:** Runs locally with `agentveil demo`; no hosted service or credential
  is required for the offline synthetic proof
- **Judge test:** [prebuilt release instructions](https://github.com/MahdiHedhli/AgentVeil/blob/main/docs/JUDGE_TEST.md)
- **Codex `/feedback` Session ID:** `<ADD SESSION ID FROM THIS MAIN TASK>`

AgentVeil is new Build Week work. Its first repository commit is dated July 14,
2026, after the July 13 submission-period opening; the final entry should retain
that public commit history and the main Codex Session ID.

## Short description

AgentVeil is a local, fail-closed privacy harness for Codex CLI. It inspects supported OpenAI Responses requests before remote egress, blocks high-risk credentials, and replaces approved lower-risk values with short-lived, session-scoped tokens so Codex can keep working without receiving those originals.

## Inspiration

AI coding agents are most useful when they can inspect real project context, but that context can contain credentials, personal information, private network addresses, and internal identifiers. Telling people to “be careful” is not a dependable boundary. We wanted a narrow, observable control at the point where Codex requests leave the machine.

## What it does

AgentVeil launches the verified Codex CLI through an authenticated loopback custom provider. Before a supported request can be forwarded, AgentVeil parses the complete JSON body, validates model-visible shapes, normalizes encoded or fragmented text, and applies a policy by data class.

- High-risk credentials and private keys are blocked before an upstream request starts.
- Approved lower-risk values, such as synthetic email or private-IP fixtures, can be replaced with opaque tokens scoped to one short-lived session.
- Tool and function outputs are inspected on the next model turn, not treated as trusted data.
- Audit records contain classifications, decisions, and safe source-field metadata—not request bodies, protected values, auth material, or token mappings.
- A loopback-only status surface exposes safe operational metadata without exposing a local authorization capability, and exact-Host validation limits browser DNS rebinding.

Unsupported or opaque model-visible shapes fail closed. AgentVeil does not install a local certificate authority, perform generic TLS interception, or require a Codex fork.

## How we built it

AgentVeil is a Rust application built around four boundaries:

1. A secure launcher pins Codex CLI `0.144.4`, creates a randomized invocation-scoped provider profile, and supplies a random per-session local authorization header.
2. An Axum gateway binds only to loopback, permits the verified Responses routes, uses fixed upstream destinations and an exact outbound-header allowlist, and rejects redirects and proxy-environment routing.
3. A schema-aware privacy engine classifies every JSON string value and object key, composes bounded normalization transforms, blocks S0 material, and tokenizes only explicitly restorable lower-risk classes.
4. A value-free audit sink is written successfully before remote forwarding; audit failure closes the route.

The packaged deterministic harness uses only synthetic fixtures and a capturing
loopback upstream. Its wire-level checks include a hard-block case where the
upstream request counter remains unchanged, outbound tokenization, tool-output
inspection, value-free dashboard state, and audit-content assertions. The
current Rust suite comprises 42 passing tests—37 library, 3 launcher, 1
offline-demo CLI, and 1 gateway integration test—and passes strict Clippy. The
release harness adds three Python regressions covering exact archive
construction, secure report-path failure handling, and descriptor cleanup. The
clean release report binds these counts to its exact commit; the gate must be
repeated after any subsequent source change.

## How Codex was used meaningfully

Codex was both the development environment and the integration target. The main Codex task coordinated the architecture, audited three Apache-2.0 donor projects without copying runtime state, inspected the matching open-source Codex `rust-v0.144.4` provider path, implemented the Rust boundary, ran adversarial reviews, fixed discovered bypasses, and exercised the real launcher and tool-result loop. The final submission must run `/feedback` in that same main task and incorporate or disposition the result.

At runtime, AgentVeil launches Codex through an invocation-scoped custom provider rather than asking users to persistently rewrite their configuration. This makes the privacy boundary explicit and reversible.

## How GPT-5.6 was used meaningfully

The verified live route uses `gpt-5.6-luna` through Codex's OpenAI Responses provider. A synthetic live validation exercised a multi-turn tool flow: Codex read synthetic local values, AgentVeil protected the subsequent `custom_tool_output`, and GPT-5.6 completed the next turn through the protected route. The audit recorded rewritten finding metadata without body values or token text; the capturing fake-upstream test remains the wire-boundary proof.

GPT-5.6 is therefore not a label on a standalone API call: it is the model on the actual protected Codex workflow the project is designed to enable.

## Challenges

The hardest part was defining the real security boundary. A string regex on a chat prompt was insufficient: sensitive data can appear in object keys, tool results, metadata, JSON escapes, percent encoding, invisible Unicode, or adjacent to another finding. We added schema validation, bounded composed normalization with source mapping, overlap-aware decisions, and fail-closed handling before considering the route protected.

A second challenge was integrating with normal Codex authentication without turning a loopback proxy into a credential relay. The launcher uses a random local header, a randomized provider identifier, a pinned executable and CLI version, strict passthrough-argument rejection, fixed upstream URLs, and a minimal header allowlist. Auth values are neither printed nor persisted by AgentVeil.

## What we learned

Privacy controls need wire evidence, not UI assurances. “Detected” is weaker than proving a raw fixture never reached the fake upstream. Tool output is also model input, restoration creates a separate local-data risk, and operational telemetry can itself become sensitive. Those observations shaped the fail-closed parser, zero-connect test, live restoration restriction, and value-free audit format.

## What's next

- Expand verified Codex payload coverage without weakening unknown-shape rejection.
- Prove safe restoration behavior against transcript persistence before enabling it on a live OpenAI route.
- Add concurrency and long-session pressure tests for ledger capacity and token lifetime.
- Evaluate additional local clients only as separate, explicitly verified adapters.

## Evidence-bounded limitations

AgentVeil is a hackathon prototype, not a production DLP product or a defense against a malicious local machine. The verified scope is local Codex CLI `0.144.4`, HTTP OpenAI Responses/SSE, supported text-bearing JSON shapes, and synthetic sensitive-data fixtures. Live OpenAI restoration is disabled. Codex IDE, cloud tasks, Responses WebSockets, browser ChatGPT, other AI clients, image/OCR input, binary bodies, and perfect detection are not claimed.

No demo, screenshot, log, issue, or submission text may contain real credentials or personal data.

Prebuilt judge artifacts support Ubuntu 24.04 x86_64 and macOS 14+ arm64 for
the offline proof. The dated live Codex evidence is narrower: macOS 26.4.1
arm64, Codex CLI `0.144.4`, HTTP Responses/SSE, and GPT-5.6 Luna.

## Built with

Rust, Axum, Tokio, Reqwest, Serde, Regex, Unicode normalization, OpenAI Responses API, Codex CLI, and GPT-5.6 Luna.
