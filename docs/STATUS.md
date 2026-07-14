# AgentVeil Status

## Current milestone

Lock the repository and implement the first privacy-boundary vertical slice.

## Working now

- Initial public-repository source tree and security documentation.
- Focused Rust privacy engine and typed Codex Responses adapter.
- Deterministic tokenization/blocking proof design.

## Verified claims

- Donor repositories were inspected without modifying tracked work.
- Codex CLI was updated from `0.140.0` to `0.144.4`, whose model catalog includes GPT-5.6 Sol, Terra, and Luna.
- A custom provider using `wire_api = "responses"`, normal Codex OpenAI auth, and loopback HTTP accepted a Codex request.
- A fake upstream returned valid Responses SSE that Codex rendered.
- A loopback passthrough preserved authenticated model discovery and a live GPT-5.6 Luna `codex exec` response.
- The route did not require a local CA, TLS interception, auth persistence, or a Codex fork.

## Unverified claims

- Secret block before upstream body transmission.
- PII/internal-data tokenization and local restoration.
- Tool/function-output protection on the next model turn.
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

1. Implement the typed request walker, detector actions, scoped ledger, and audit event model.
2. Prove email/private-IP tokenization and hard-secret blocking against a fake upstream.
3. Put the engine behind the authenticated loopback gateway and test fragmented SSE restoration.

## Demo readiness

Integration seam: ready. Deterministic privacy proof: not ready. Dashboard/video: not started.

## Deadline health

Scope is aggressive but viable only with the Friday non-goals held. Core proof and tool-result replay take priority over dashboard polish.
