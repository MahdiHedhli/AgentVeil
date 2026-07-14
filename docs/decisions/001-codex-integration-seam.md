# 001: Codex integration seam

- Status: accepted for MVP
- Date: 2026-07-14
- Required client: local Codex CLI

## Decision

Use an explicit Codex custom model provider over loopback HTTP, with OpenAI Responses wire format and SSE. AgentVeil accepts the normal Codex request, inspects the complete outbound JSON body before opening the upstream request, then forwards only to a fixed OpenAI destination. The profile disables Responses WebSockets for the MVP.

Illustrative verified configuration shape:

```toml
model = "gpt-5.6-luna"
model_provider = "agentveil"

[model_providers.agentveil]
name = "AgentVeil"
base_url = "http://127.0.0.1:48741/v1"
wire_api = "responses"
requires_openai_auth = true
supports_websockets = false
env_http_headers = { "X-AgentVeil-Session" = "AGENTVEIL_SESSION_TOKEN" }
```

The final launcher will generate a random local session credential in the environment and require it at the gateway. The header is stripped before upstream forwarding.

## Candidates

1. **Custom provider** — chosen. Explicit, reversible, Codex-specific, no CA or generic interception.
2. **Documented base-URL override** — fallback only if a future Codex release breaks the named provider profile.
3. **PromptFence-derived TLS interception** — rejected for MVP; too broad and introduces certificate/trust-store risk.
4. **Codex source hook/fork** — rejected for MVP; unnecessary maintenance and distribution burden.

## Tests performed

- Inspected installed Codex help/config behavior and the matching open-source `rust-v0.144.4` source at commit `8c68d4c87dc54d38861f5114e920c3de2efa5876`.
- Checked current official Codex custom-provider/auth documentation and official Responses streaming documentation.
- Sent an ephemeral `codex exec` request with GPT-5.6 Luna through a loopback fake provider.
- Returned a minimal typed Responses SSE stream; Codex rendered `AGENTVEIL_SPIKE_OK`.
- Observed only structural request metadata: `POST /v1/responses`, JSON, `stream: true`, message/additional-tool items, and declared tool names.
- Forwarded live authenticated Codex model discovery and a synthetic `codex exec` through a loopback passthrough to the Codex ChatGPT upstream; GPT-5.6 Luna returned the expected safe marker.

## Authentication observations

With `requires_openai_auth = true`, Codex sent authorization plus ChatGPT account context to the custom provider. AgentVeil does not need to read auth files or persist a bearer token. No credential value was logged or captured. The gateway must still treat inherited authorization as reusable sensitive material: never log it, never expose it to the dashboard, strip cookies and local session headers, use a fixed upstream allowlist, and reject unauthenticated local callers.

## Streaming, tools, and conversations

- Responses SSE transport is verified through the custom provider.
- Codex supplied its tool definitions to the provider, proving the request seam includes tool capability metadata.
- At the original decision point, full tool-call execution/output replay and
  multi-turn restoration were not verified. Candidate `v0.1.5` now verifies a
  real-Codex, two-turn synthetic replay against a capturing loopback fixture,
  with restoration limited to `response.output_text.delta`. This does not
  expand the live OpenAI restoration claim.
- WebSockets are intentionally disabled.

## Failure cases and security implications

- The initial fake provider did not implement model discovery, causing harmless refresh warnings; the real gateway must proxy the allowlisted models route.
- A custom provider without local authentication would be an authenticated relay. The production route requires a random per-session local header and loopback binding.
- Restoring text before Codex consumes it may persist originals in local
  threads. Live OpenAI restoration remains disabled until transcript behavior
  is verified. The synthetic real-Codex demo uses a private temporary home,
  verifies deletion on clean exit, and documents forced-kill/crash residue.
- Unknown model-visible payload shapes fail closed; binary, image, encrypted, compressed, and opaque content are unsupported.

## Unsupported paths

Codex IDE, cloud tasks, WebSocket Responses, browser ChatGPT, other AI clients, image/OCR, binary bodies, and any local Codex feature that bypasses the configured provider.

## Why this is not PromptFence

PromptFence is a multi-client interception project. AgentVeil uses a narrow, explicit, user-selected Codex provider profile and a schema-aware Responses adapter. It avoids system trust changes and does not inherit the donor proxy's generic HTTP/WebSocket rewriting or restoration behavior.
