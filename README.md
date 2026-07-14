# AgentVeil

**Keep sensitive data out of Codex context.**

AgentVeil is a local privacy harness for ChatGPT Codex. It inspects supported Codex Responses requests before remote egress, blocks high-risk credentials, and replaces approved lower-risk values with short-lived session tokens so Codex can keep working without receiving the originals.

> Status: active OpenAI hackathon build. The Codex routing seam is verified; the privacy engine and deterministic no-leak proof are under construction. Do not treat this snapshot as a production security control.

## What is verified today

- Codex CLI `0.144.4` can route GPT-5.6 Responses traffic through a loopback custom provider.
- Normal ChatGPT authentication reaches the local provider without AgentVeil persisting credentials.
- The local provider can forward authenticated model discovery and live GPT-5.6 traffic.
- Responses SSE reaches Codex successfully through the local route.

Claims for secret blocking, PII tokenization, restoration, tool-result protection, and value-free audit will be promoted here only after their fake-upstream and live-route gates pass.

## Scope

Friday MVP: local Codex CLI, HTTP Responses/SSE, schema-aware outbound inspection, synthetic secret blocking, scoped lower-risk tokenization, value-free audit, deterministic fake-upstream proof, and a live synthetic Codex demonstration.

Not claimed: Codex IDE, WebSockets, cloud tasks, browser ChatGPT, other AI clients, image/OCR or binary inspection, perfect DLP, or protection against a malicious local machine.

See [MVP](docs/MVP.md), [integration decision](docs/decisions/001-codex-integration-seam.md), [status](docs/STATUS.md), and [risk register](docs/RISK_REGISTER.md).

## License and security

Apache-2.0. See [donor attribution](docs/DONOR_ATTRIBUTION.md). Report security issues privately using the contact in [SECURITY.md](SECURITY.md); do not include secrets in a report.
