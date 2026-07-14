# Security invariants

These are mechanical release gates, not aspirations.

1. S0 credentials, private keys, JWTs, credentialed database URLs, and unknown AgentVeil token literals are always whole-request blocks.
2. A blocked or rejected request opens no upstream connection and sends no request-body bytes.
3. The complete bounded JSON body is parsed, shape-validated, scanned, and reserialized before upstream forwarding begins.
4. Duplicate JSON keys, unknown top-level context fields, unsupported input items, unsupported content parts, and media payloads reject locally.
5. Detector, normalizer, policy, ledger, or rewrite failure fails closed.
6. Overlaps are resolved before ledger allocation with precedence `block > mask > tokenize > allow`.
7. Token mappings are memory-only, at least 128 bits of OS randomness, authenticated-session scoped, TTL-bound, capacity-bound, and cleared on session end.
8. A hard-secret value can never enter the token ledger.
9. Restoration accepts only an exact AgentVeil-issued token in the same live session and an approved assistant display-text event.
10. Tokens are never restored into tool arguments, commands, URLs, headers, file writes, patches, IDs, errors, reasoning metadata, logs, audit, dashboard, or unknown events.
11. Audit events contain typed metadata only: no body, finding value, context, token, mapping, auth header, cookie, query string, response text, or sensitive path.
12. Gateway and dashboard bind to loopback. Protected Responses/model routes require an unguessable per-session local credential; the read-only dashboard exposes only value-free typed metadata, requires the exact listener Host authority, and receives no browser-visible credential.
13. Upstream destination, path, method, and forwarded headers are allowlisted; AgentVeil is not a generic authenticated relay.
14. Synthetic canaries are absent from audit, dashboard state, generated reports, stdout/stderr, crash output, and fake-upstream evidence except explicitly approved in-memory fixture assertions.
15. Protection labels are tied to tested Codex version, transport, route, and payload fields. IDE, cloud, WebSocket, image, binary, and opaque paths remain unsupported until independently verified.

The current 42-test suite covers the engine, policy, ledger, audit, dashboard assets and Host boundary, launcher boundary, packaged offline demo, Responses/SSE wire path, zero-connect blocking, tool-result re-entry, and value-free dashboard/audit state. Browser QA, the clean release/leak run, the default benchmark report, the fake-upstream proof, and a renewed live synthetic proof passed for the current candidate. These remain recurring release gates: any source change before tagging requires a fresh pass.
