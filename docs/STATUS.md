# AgentVeil Status

## Current milestone

Put the verified core privacy engine behind the authenticated loopback gateway.

## Working now

- Loopback gateway with strict upstream/header allowlists.
- Responses SSE display-text restoration without unsafe restoration sinks.
- Fake-upstream end-to-end proof and value-free audit persistence.

## Verified claims

- Donor repositories were inspected without modifying tracked work.
- Codex CLI was updated from `0.140.0` to `0.144.4`, whose model catalog includes GPT-5.6 Sol, Terra, and Luna.
- A custom provider using `wire_api = "responses"`, normal Codex OpenAI auth, and loopback HTTP accepted a Codex request.
- A fake upstream returned valid Responses SSE that Codex rendered.
- A loopback passthrough preserved authenticated model discovery and a live GPT-5.6 Luna `codex exec` response.
- The route did not require a local CA, TLS interception, auth persistence, or a Codex fork.
- The focused Rust engine passes 23 unit tests covering policy hard invariants, secret detectors, compatibility/escape/percent normalization, original-span rewrites, typed message and tool-output fields, duplicate-key/unknown-shape rejection, session/TTL/capacity ledger behavior, exact restoration, unknown-token injection, and value-free audit serialization.
- The first in-memory vertical slice tokenizes a synthetic email and private IP, restores exact issued tokens in the originating scope, blocks an assembled synthetic credential before ledger allocation, and sanitizes a `function_call_output` item.

## Unverified claims

- Secret block before upstream connection/body transmission.
- PII/internal-data tokenization and local restoration at the HTTP/SSE boundary.
- Tool/function-output protection on a real next-model-turn route.
- Multi-turn ledger scope and transcript behavior.
- Value-free audit and generated-report invariants.
- Local dashboard and CLI/profile lifecycle.

## Blockers

None for the vertical slice. Live tool-call replay remains a required validation gate.

## Risks

- Restored text may enter the local Codex transcript; restoration stays restricted until tested.
- New or opaque Codex payload shapes can create a fail-open risk; unsupported model-visible shapes must reject.
- An authenticated loopback proxy could become a credential relay without per-session local authorization and strict route/header allowlists.
- Donor working copies contain ignored local security-sensitive state that must never be copied. See [risk register](RISK_REGISTER.md).

## Next three actions

1. Implement the authenticated loopback gateway and safe upstream forwarding.
2. Prove email/private-IP tokenization and hard-secret zero-connect behavior against a fake upstream.
3. Test fragmented SSE restoration and a real Codex tool-output replay.

## Demo readiness

Integration seam: ready. Core in-memory slice: ready. Deterministic wire proof: not ready. Dashboard/video: not started.

## Deadline health

Scope is aggressive but viable only with the Friday non-goals held. Core proof and tool-result replay take priority over dashboard polish.
