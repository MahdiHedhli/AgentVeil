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
9. Synthetic restoration accepts only an exact AgentVeil-issued token in the same live session and an approved assistant display-text event. The Codex-compatible mode restores only `response.output_text.delta`; structured done/item/completion/response snapshots remain tokenized.
10. Tokens are never restored into tool arguments, commands, URLs, headers, file writes, patches, IDs, errors, reasoning metadata, logs, audit, dashboard, or unknown events.
11. Audit events contain typed metadata only: no body, finding value, context, token, mapping, auth header, cookie, query string, response text, or sensitive path.
12. Gateway and dashboard bind to loopback. Protected Responses/model routes require an unguessable per-session local credential; the read-only dashboard exposes only value-free typed metadata, requires the exact listener Host authority, and receives no browser-visible credential.
13. Upstream destination, path, method, and forwarded headers are allowlisted; AgentVeil is not a generic authenticated relay.
14. Synthetic canaries are absent from audit, dashboard state, generated reports, automated-check stdout/stderr, crash output, and fake-upstream evidence except explicitly approved in-memory fixture assertions. Only the explicitly interactive `codex-demo` TUI may display the documented synthetic lower-risk fixtures; it may never display or restore an S0 credential/private key fixture.
15. Protection labels are tied to tested Codex version, transport, route, and payload fields. IDE, cloud, WebSocket, image, binary, and opaque paths remain unsupported until independently verified.

The current candidate's 51-test Rust suite (43 library, 3 binary CLI, 2 demo CLI integration, and 3 gateway end-to-end tests) covers the engine, policy, ledger, audit, dashboard assets and Host boundary, launcher boundary, both packaged demos, Responses/SSE wire path, delta-only history behavior, sticky synthetic-proof failure, zero-connect blocking, tool-result re-entry, and value-free dashboard/audit state. Four Python regressions cover exact archive construction and inspection, tag/binary version mismatch rejection, release-report directory diagnostics, and descriptor cleanup. These are working-tree results until a clean release report binds them to a tag. Browser QA, release/leak gates, benchmark, fake-upstream proof, and live synthetic proof remain recurring release gates after any source change.
