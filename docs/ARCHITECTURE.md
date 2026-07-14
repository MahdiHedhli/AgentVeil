# Architecture

Evidence anchor: commit `ed09354`, Codex CLI `0.144.4`, model
`gpt-5.6-luna`, HTTP Responses/SSE.

AgentVeil is a narrow outbound enforcement point for one explicitly configured
Codex process. It is not a transparent system proxy, TLS interceptor, or remote
service.

## Request path

```text
┌──────────────────── local machine ────────────────────┐
│                                                       │
│  Codex CLI                                            │
│    │ POST /v1/responses                               │
│    │ X-AgentVeil-Session: random local capability     │
│    ▼                                                   │
│  AgentVeil loopback gateway                           │
│    1. authenticate local caller                       │
│    2. require uncompressed JSON                       │
│    3. buffer within policy size limit                 │
│    4. unique-key parse + strict shape validation      │
│    5. normalize and scan every accepted string/key    │
│    6. resolve overlap and policy action               │
│    7. block, or rewrite and reserialize               │
│    8. persist value-free pending audit event          │
│    │                                                   │
└────┼──────────────── egress boundary ─────────────────┘
     ▼
  fixed OpenAI Responses endpoint
     │ Responses SSE
     ▼
  Codex CLI (live stream is passed through unchanged)
```

A protected request is fully inspected and reserialized before the upstream
request builder calls `send`. Blocked, rejected, unauthenticated, unsupported,
and inspection-failure paths return locally. The deterministic fake-upstream
test uses request count as the physical zero-connect assertion.

## Components

### Secure launcher

`agentveil codex` owns the supported lifecycle:

- resolves and canonicalizes `codex` from `PATH`, requires an executable outside
  the current workspace, and then launches that pinned path;
- accepts only exact version `0.144.4` and checks `codex login status` without
  reading credential contents;
- accepts only `gpt-5.6-luna` and a bounded reasoning-effort enum;
- binds an ephemeral loopback port by default;
- generates an OS-random local header credential, session scope, and randomized
  provider identifier;
- supplies an invocation-scoped Codex configuration overlay for Responses HTTP,
  OpenAI authentication, and WebSockets disabled;
- rejects passthrough arguments that can replace the model, provider, profile,
  routing mode, or supported command path;
- gives the local gateway credential only to the Codex process, while setting
  that variable to an empty value in Codex-created shell environments; and
- shuts the gateway down after Codex exits, with a bounded grace period.

The generated provider identifier prevents accidental merging with a
same-named user provider. No persistent Codex configuration is written. The
provider URL and identifier may be visible in the local process command line;
the session credential is not placed there.

### Loopback gateway

The Axum gateway refuses non-loopback binds. Protected routes require an
alphanumeric 32–128 byte local credential and compare it in constant time.

Exposed routes in the current build are deliberately small:

| Route | Purpose | Controls |
|---|---|---|
| `GET /health` | value-free local health metadata | loopback binding; no protected values |
| `GET /dashboard` and local assets | read-only status UI | loopback, embedded assets, CSP/no-store/nosniff and related headers; deliberately no browser credential |
| `GET /dashboard/state` | value-free activity JSON | loopback, closed schema, maximum 64 deduplicated activities; deliberately no browser credential |
| `GET /v1/models` | Codex model discovery | local credential, one Bearer header for live mode, fixed endpoint, one allowlisted query |
| `POST /v1/responses` | protected model turn | local credential, live auth check, strict JSON/body rules, privacy engine, audit-before-egress |

All other routes return a local error. Live targets are compile-time fixed to
the OpenAI API or ChatGPT Codex endpoints. The client disables environment proxy
use and redirects. Request and response headers are copied through explicit
allowlists; the AgentVeil credential, cookies, and arbitrary caller headers are
not forwarded. Synthetic mode accepts only an unauthenticated loopback HTTP
upstream and strips authorization.

Dashboard routes are intentionally unauthenticated because they contain no
protected values or mutation, while putting the session capability in a URL,
DOM, cookie, or browser storage would create a secret-exposure risk. State is
derived only from value-free audit events. `originals_forwarded: 0` reports the
enforcement invariant; the capturing fake upstream remains the wire proof.

### Payload adapter

The adapter rejects duplicate JSON keys before semantic validation. It then
accepts only the top-level controls and input-item/content shapes listed in
[PAYLOAD_MAP.md](PAYLOAD_MAP.md). `store` must be `false`, `stream` must be
`true`, and `tool_choice` must be `auto`.

Every JSON string value in an accepted request is classified as either:

- **rewriteable model text**, where mask/tokenize actions may replace the exact
  original source span; or
- **block-only structural text**, where any non-allow finding blocks the whole
  request rather than changing protocol-integrity data.

Every JSON object key is also scanned as block-only structural text. Unknown
top-level fields, unknown input items, unsupported content parts, media, and
invalid typed controls reject locally.

### Normalization and detection

Detection runs over bounded, source-mapped views. Current transforms include
NFKC compatibility normalization, selected invisible-control removal,
line-continuation handling, JSON escape decoding, percent decoding, and
candidate-separator collapse. Decode composition is capped at two layers, eight
regular views, and twelve total scan views; limit or invalid-UTF-8 failures fail
closed.

Detectors cover the explicitly documented credential and identifier forms.
Overlapping or adjacent connected findings are resolved before rewriting, using
the strongest applicable policy action: `block > mask > tokenize > allow`.
Ledger entries are allocated only after the request has no blocking finding.
Any failure during rewrite/serialization rolls back mappings created for that
request.

### Policy engine

Policies are strict, versioned YAML. Unknown policy keys reject. Validation
enforces:

- raw logging is always `false`;
- unknown-sensitive behavior is always `block`;
- credentials, private keys, JWTs, credentialed database URLs, and unknown
  AgentVeil tokens cannot be tokenized or restored;
- tokenization requires explicit restorability and a TTL of at most 30 minutes;
  and
- request/ledger limits and configured detector terms are bounded.

The default policy masks email, contextual phone, home path, and custom term;
tokenizes private IPv4 and configured internal hostname for 15 minutes; and
blocks hard-secret classes. The demo policy is synthetic-only and cannot be
used with the live OpenAI upstream.

### Memory-only token ledger

Restorable mappings are held in the gateway process only. Tokens contain 128
bits of OS randomness and a non-secret type label. Lookup requires an exact
token, matching session scope, and unexpired entry. Values are wrapped in
zeroizing memory, capacity is bounded, and least-recently-used entries are
evicted when full. Hard-secret classes are rejected by the ledger API itself.

Unknown, expired, evicted, or cross-scope AgentVeil token literals are blocked
on outbound inspection. The ledger is lost when the gateway process exits.

### Audit sink

Audit records are constructed from closed enums and validated identifiers. They
contain event/session pseudonyms, sequence, decision, typed finding summaries,
upstream lifecycle state, scan latency, policy identity, and AgentVeil version.
They do not contain request/response bodies, matched context, original values,
replacement tokens, mappings, authorization, cookies, query strings, or
sensitive paths.

The sink creates its direct parent with mode `0700` and the append-only file
with mode `0600`, rejects symlink/permissive final paths, and flushes each
record. A `pending` record must persist before upstream send. A write failure
marks audit unhealthy and prevents later protected egress.

### SSE response path

Live OpenAI SSE is streamed back without restoration. Configuration refuses to
enable response restoration with the live upstream.

The synthetic-only restoration path buffers typed SSE frames, limits one frame
to 1 MiB, restores only exact owned tokens in selected assistant display-text
fields, and leaves tool arguments, commands, identifiers, and unrecognized
events unchanged. Malformed, truncated, or unsupported typed text shapes fail
the local stream. This path exists for deterministic proof, not as a live
product claim.

## Trust boundaries

1. **Local Codex to AgentVeil.** Codex is authenticated by a short-lived local
   capability. A random local process without it cannot use protected routes.
2. **AgentVeil to OpenAI.** Only a validated/rewritten Responses body and
   allowlisted Codex/OpenAI headers cross. Normal OpenAI authentication still
   crosses because it is required by the upstream service.
3. **Model response to local Codex.** Live responses are not privacy-filtered or
   restored. AgentVeil protects their content only if Codex later includes it in
   a supported outbound turn.
4. **Local audit boundary.** Typed enforcement metadata is persisted locally;
   originals and mappings remain outside the schema.

See [THREAT_MODEL.md](THREAT_MODEL.md) for assumptions and residual risk.

## Why a custom provider

The explicit Codex custom-provider seam is reversible and client-specific. It
requires no local CA, system trust-store change, TLS interception, generic
proxy, or Codex fork. That narrower boundary makes routing and wire evidence
auditable, but it also means unconfigured Codex workflows are outside the
claim.
