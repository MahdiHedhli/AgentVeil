# AgentVeil contributor guardrails

Cybersecurity and evidence quality are release requirements.

- Use synthetic sensitive-data fixtures only. Never paste, print, capture, or commit real credentials or personal data.
- A route is protected only after wire-level verification shows the raw fixture did not cross the local egress boundary.
- Fail closed when a model-visible request shape cannot be inspected safely.
- Never log request or response bodies, authorization material, cookies, protected values, token mappings, or sensitive filesystem paths.
- S0 credentials and private keys are blocked, never tokenized, and never restorable.
- Restoration is exact-token, session-scoped, TTL-bound, and limited to explicitly verified local display text.
- Keep the gateway, dashboard, and verification harness loopback-only.
- Preserve donor licenses and attribution. Do not copy ignored runtime state from donor repositories.
- Do not weaken a security invariant merely to make a demo pass. Record unsupported behavior honestly.
