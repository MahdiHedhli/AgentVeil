# Risk register

| ID | Risk | Current control | Status |
|---|---|---|---|
| AV-001 | Unsupported Codex field carries model-visible sensitive text | Schema-aware allowlist; reject unknown text-bearing/integrity-bound shapes | Open until contract tests pass |
| AV-002 | Local gateway becomes an authenticated OpenAI relay | Loopback binding, per-session local credential, fixed upstream and route allowlists | Implementation pending |
| AV-003 | Audit, dashboard, report, or logs contain protected values | Typed value-free events plus synthetic canary scan | Implementation pending |
| AV-004 | Restoration crosses sessions, survives TTL, or enters unsafe fields | Authenticated session scope, exact token lookup, bounded memory, display-text-only allowlist | Implementation pending |
| AV-005 | Restored text is persisted in Codex local transcripts | No live restoration claim until transcript replay is tested; deterministic local display only | Open |
| AV-006 | SSE/UTF-8 fragmentation corrupts or leaks tokens | Framing-aware parser and every-split tests | Implementation pending |
| AV-007 | Detector/normalizer failure silently bypasses protection | Required detectors fail closed before any upstream body bytes | Implementation pending |
| AV-008 | Donor runtime state is copied or exposed | New clean repo; broad ignore rules; donor state excluded | Controlled |
| AV-009 | PromptFence ignored archives include secret-shaped local state and some permissive file modes | Values were not read; never copy; owner authorization required before permission remediation | Awaiting owner decision |
| AV-010 | PromptGuard ignored `.env` is mode `0644` | File was not read; never copy; owner authorization required before permission remediation | Awaiting owner decision |

No donor permission or content remediation is authorized by the AgentVeil build itself.
