# Risk register

Generated release reports bind evidence to the exact Git commit and clean-tree
observations. “Controlled” below means controlled only inside the exact
client/transport/payload boundary in [PAYLOAD_MAP.md](PAYLOAD_MAP.md), not
universally eliminated.

## Product and implementation risks

| ID | Risk and consequence | Current control / evidence | Status and next gate |
|---|---|---|---|
| AV-001 | A future/unsupported Codex field carries model-visible sensitive text. | Unknown top-level/item/content shapes reject; every string/key in an accepted payload is rewrite or block-only; duplicate keys reject. | **Controlled for the current release; recurring.** Any Codex/schema upgrade requires adapter, map, fake-wire, and live-proof refresh. |
| AV-002 | The loopback gateway becomes an authenticated OpenAI relay. | Loopback-only bind, OS-random local capability, constant-time check, fixed routes/hosts, single-Bearer validation, explicit header/query allowlists, redirects and environment proxies disabled. | **Controlled for the current release.** Add negative route/header regression cases to the recurring release suite. |
| AV-003 | Audit, dashboard, generated report, logs, or terminal output contains originals, mappings, auth, or tokens. | Closed typed audit schema; no body logging; audit unit/file/wire/live scans; dashboard consumes typed audit metadata and has local-asset/no-secret tests; desktop browser and responsive mobile DOM checks passed. | **Controlled for the current release; recurring.** The clean release/report scan passed and remains a gate after any change. |
| AV-004 | Restoration crosses sessions, outlives TTL, or enters unsafe fields. | Exact token, session scope, TTL/capacity, unknown-token block, display-field allowlist; unit/SSE tests. | **Controlled in synthetic mode.** Live restoration remains disabled. |
| AV-005 | Restored originals persist in or replay from local Codex transcripts. | Gateway rejects restoration with OpenAI; live responses remain tokenized. | **Open research / safely disabled.** Do not enable live restoration until transcript lifecycle and replay are independently proven. |
| AV-006 | An AgentVeil token is split across multiple semantic SSE delta events and fails restoration. | Every transport-byte split of a complete event is tested; unsafe events never restore. | **Open functionality limit.** No privacy regression while live restoration is disabled; semantic reassembly needs bounded state before expansion. |
| AV-007 | Detector/normalizer/source-map failure silently bypasses protection. | Bounded composed transforms; invalid percent UTF-8/view limit/span/rewrite failures close; tests cover composed bypasses and invalid UTF-8. | **Controlled for tested failures.** Fuzz/property testing remains a release-hardening opportunity. |
| AV-008 | Donor runtime state, captures, environments, or archives are copied into the public repository. | New clean implementation; ignored runtime patterns; donor attribution; policy forbids donor-state copying. | **Controlled in AgentVeil.** Release leak scan must include ignored/generated artifacts, not only tracked source. |
| AV-011 | Raw tool output is displayed or persisted locally before AgentVeil protects the next model turn. | Tool-output field is protected at the Responses boundary; live synthetic replay verified. | **Accepted scope limit.** AgentVeil prevents supported OpenAI egress, not local terminal/transcript retention. |
| AV-012 | LRU capacity evicts a still-live mapping, breaking later restoration or replay. | Bounded ledger, deterministic LRU, expiry purge; unowned token blocks outbound rather than passing. | **Open availability/usability risk.** Consider reject-on-capacity or request-lifetime pinning before live restoration. |
| AV-013 | Hostile/interposed audit path exploits an unchecked intermediate path component or AgentVeil changes an unrelated directory's permissions. | Pre-existing parents are never chmodded; direct parent must be private/non-symlink, is canonicalized, and the final file uses `0600`, `O_NOFOLLOW`, metadata verification, flush, and sync. | **P0 permission-mutation flaw fixed; residual filesystem hardening open.** Prefer a trusted absolute base; descriptor-relative component traversal remains future work. |
| AV-014 | Relative audit path lands in an unexpected/shared current directory. | Repository runtime/audit patterns are Git-ignored; installer docs recommend an explicit private absolute path. | **Open operator risk.** Change packaged default to an OS state directory before production use. |
| AV-015 | A malicious or substituted `codex` binary passes only the version-string check. | Launcher canonicalizes an executable outside the workspace and pins the resolved path; exact version/login required. | **Open supply-chain risk.** Signed/package-hash verification is absent at anchor. |
| AV-016 | Exact Codex pin becomes stale, encouraging users to bypass AgentVeil after an update. | Other versions fail closed with an explicit error; docs forbid bypassing the pin. | **Accepted safety tradeoff.** Publish compatibility only after full evidence refresh. |
| AV-017 | A privileged/same-user malicious process reads gateway memory, auth, or the local capability. | Capability is random, not in provider command-line config, and blanked for Codex-created shell tools. | **Out of threat scope.** Loopback capability is not a boundary against root/debugger/process-memory access. |
| AV-018 | A Codex tool or MCP server exfiltrates data through its own network route. | No false claim; normal Codex sandbox/approval controls remain necessary. | **Out of product scope.** AgentVeil mediates Responses egress only. |
| AV-019 | Detector false negative sends an unsupported secret form, or false positive blocks safe work. | High-specificity patterns, typed fields, strict structural blocking, bounded normalization, documented classes. | **Residual/open.** Expand only with synthetic fixtures, source-mapped tests, and zero-connect evidence. |
| AV-020 | Large/concurrent requests or synchronous audit flush create local denial of service. | Body/SSE/view/ledger limits; serialized engine; errors fail closed; offline benchmark harness measures full release-test processes without relabeling them as request latency. | **Open availability risk.** Add request-level concurrency/resource measurements before production claims. |
| AV-021 | Upstream response contains sensitive/model-generated content that AgentVeil does not inspect. | Live response is passed through; later supported outbound replay is scanned. | **Accepted scope limit.** AgentVeil is an outbound request boundary, not response DLP. |
| AV-022 | Audit lifecycle is mistaken for delivery proof. | `pending`, `started`, and `failed` states are explicit and reuse an event ID; pre-send pending must flush. | **Documentation-controlled.** A started row does not prove model receipt/completion. |
| AV-023 | Normal Codex operational headers expose account/project/session metadata despite body protection. | Only an explicit header allowlist is forwarded; cookies/local capability are stripped; values are not logged. | **Accepted functional requirement.** AgentVeil is not an identity anonymizer; minimize allowlist as Codex evidence permits. |
| AV-024 | Direct `agentveil serve` setup leaks its manually managed environment credential or is misconfigured. | Launcher owns the supported lifecycle; serve validates token/scope, loopback, live/demo separation, and upstream URL. | **Open advanced-interface risk.** Documentation directs live users to `agentveil codex`. |
| AV-025 | Release dashboard/report/demo claims outrun the core evidence. | Packaged demo asserts capturing-upstream state; live dashboard says wire proof is not measured; browser QA passed; release reports bind status to commit and clean-tree state. | **Controlled for the current release.** Clean release/leak, benchmark, fake-wire, and live synthetic gates passed; repeat them after any source change. |
| AV-026 | A browser reaches the unauthenticated dashboard through DNS rebinding and reads typed local activity. | Every dashboard/state/asset request requires the exact loopback listener Host authority; hostile Host integration test returns `421`; no credential enters browser state. | **Controlled for the tested HTTP dashboard.** Keep loopback-only and do not reverse-proxy or port-forward. |

## External donor-workspace risks

These observations concern separate local donor working copies, not files in the
AgentVeil repository. Their contents were not read and must not be copied. The
AgentVeil task does not authorize modifying those repositories.

| ID | Observation | Current control | Status |
|---|---|---|---|
| AV-009 | PromptFence ignored runtime archives have secret-shaped names/content classification and some were observed with mode `0644`; its `.claude.env` was mode `0600`. No value was opened or copied. | Keep all ignored state out of AgentVeil; values remain uninspected. | **Awaiting owner authorization.** Permission/content remediation must occur in the donor workspace, not during AgentVeil build. |
| AV-010 | PromptGuard ignored `.env` was observed with mode `0644`; untracked `PromptGuard.zip` must be preserved. The environment file was not read or copied. | Keep both outside AgentVeil; do not delete or inspect the archive/environment as part of this project. | **Awaiting owner authorization.** Restricting the donor `.env` requires explicit approval. |

Until the owner authorizes donor remediation, release checks should verify only
that no donor runtime state entered AgentVeil. They must not traverse, print,
archive, upload, chmod, delete, or otherwise alter the donor files.

## Release interpretation

A risk marked controlled can reopen when any of these change: Codex version,
model, provider configuration, Responses schema, detector/policy, upstream
allowlist, audit/dashboard/report surface, restoration mode, or packaging. The
release commit must rerun deterministic wire, leak, lint/test, and live synthetic
proofs before moving the evidence anchor.
