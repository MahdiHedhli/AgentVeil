# Threat model

Generated release reports bind evidence to the exact Git commit and clean-tree
observations. This model covers the verified Codex CLI `0.144.4` HTTP
Responses/SSE route only.

## Security objective

For supported outbound request shapes, prevent supported raw sensitive values
from entering the remote model request by blocking the whole request or
replacing exact source spans before any request body is sent upstream.

This objective is intentionally narrower than “protect every secret used by an
agent.” AgentVeil does not control filesystem reads, terminal display, local
transcripts, arbitrary tool network access, or Codex paths that do not use its
generated provider.

## Assets

- originals detected in supported request text;
- in-memory token-to-original mappings;
- Codex/OpenAI authorization and account context;
- the unguessable local gateway credential and session scope;
- routing integrity: the assertion that a protected Codex process uses the
  verified gateway and model;
- audit confidentiality and integrity; and
- the distinction between tested evidence and unsupported product claims.

## Expected adversarial inputs

AgentVeil treats outbound prompt, prior-turn, tool-result, and selected control
text as untrusted. Sources include accidental paste, a repository fixture,
untrusted tool output, prompt injection, and deliberately obfuscated strings.
The current test corpus includes Unicode compatibility characters, invisible
controls, JSON escapes, percent encoding, separator insertion, adjacent
credential prefixes, partial/encrypted PEM headers, structural JSON keys, and
metadata fields.

## In-scope actors and failures

1. **Accidental disclosure by the user or a local tool.** A supported value is
   included in a model-visible request field.
2. **Adversarial content.** Repository or tool output tries to evade a supported
   detector or smuggle a value through a structural field.
3. **Unauthorized local caller.** Another process tries to use AgentVeil as an
   authenticated OpenAI relay without the session capability.
4. **Protocol drift.** A future Codex payload introduces a model-visible or
   opaque shape that the adapter does not understand.
5. **Implementation/runtime failure.** Parsing, normalization, randomness,
   rewriting, serialization, audit persistence, or upstream selection fails.
6. **Model-controlled replay.** The model emits an AgentVeil-looking token or
   puts a token in an unsafe tool/command field.

## Out-of-scope actors and systems

- a malicious administrator, root process, debugger, kernel, or compromised
  user account on the local machine;
- malware able to read process memory, inherited authorization, local files,
  terminal buffers, or Codex state;
- OpenAI compromise, transport-layer compromise outside normal TLS assumptions,
  or a compromised Codex binary/dependency;
- network egress performed directly by a Codex tool, shell command, browser,
  MCP server, or other process instead of the Responses request;
- secrets already persisted in source control, shell history, terminal output,
  local Codex transcripts, crash reports, swap, backups, or screenshots;
- unconfigured Codex clients, IDE/cloud tasks, WebSockets, browser ChatGPT,
  other providers, images/OCR, files, audio, binary data, or compressed bodies;
  and
- perfect classification of every sensitive value or encoding.

## Attack analysis

| Threat | Primary control at the anchor | Evidence | Residual risk |
|---|---|---|---|
| Direct supported credential in message/tool output | hard-block policy before upstream request creation | engine and gateway tests; live synthetic tool replay | unsupported token formats may not match |
| Encoded or split detector bypass | bounded composed normalization with source mapping; failure closes | normalization/unit tests and zero-connect gateway cases | composition is bounded and not every encoding is supported |
| Secret in unknown top-level field or duplicate key | strict allowlist and unique-key parser | payload and gateway tests | a newly allowed but misclassified field would require review |
| Secret in schema key, identifier, encrypted metadata, or other structural string | every accepted string and object key is scanned; protected structural finding blocks | structural-key/metadata gateway tests | opaque bytes that cannot be inspected are unsupported, not decoded |
| Partial overlap leaks part of a value | overlap-connected findings use union span and strongest action | engine overlap test | detector must still identify part of the value |
| Hard secret enters restoration ledger | policy validation and ledger API reject hard-block classes | policy, engine, and ledger tests | none within current typed class boundary |
| Forged/expired/cross-session AgentVeil token | exact token grammar, ownership, scope, TTL; unknown token blocks outbound | engine and ledger tests | LRU eviction can make a legitimate old token unavailable |
| Local gateway used as credential relay | loopback bind, random session capability, constant-time comparison, fixed routes/hosts, header allowlist, no redirects/proxy | launcher tests and unauthorized gateway assertion | a privileged/same-user process with memory inspection is out of scope |
| Session capability leaks through a Codex shell tool | launcher overrides shell environment value to empty | manual synthetic launcher proof | other local process-inspection channels are out of scope |
| Caller selects another model/provider or bypass mode | launcher pins model/provider/version and rejects routing overrides | CLI tests | direct use of `agentveil serve` is an advanced interface and requires correct environment setup |
| Audit leaks originals/tokens/auth | value-free type schema; no body logging; private path modes | audit unit/file tests, gateway leak assertions, live audit scan | alternate unsafe parent path and surrounding host logging remain operator risks |
| Audit fails after request protection | a pending event must flush before send; unhealthy state blocks future protected requests | code path and audit tests | disk failure after pending write can still leave only partial lifecycle evidence |
| Redirect/proxy exfiltrates authenticated request | redirects disabled, environment proxies disabled, compile-time fixed URLs | code inspection | DNS/TLS/platform trust remains an external dependency |
| Response restoration inserts original into command/tool argument | restoration allowlists typed display events; Codex-compatible mode permits only `response.output_text.delta`; non-display events remain tokenized | SSE and gateway end-to-end tests | live restoration is disabled because Codex transcript semantics remain unverified |
| Synthetic Codex display persists after the demo | private isolated `CODEX_HOME`; clean exit recursively deletes and verifies the entire root | real-Codex synthetic smoke and cleanup check | forced kill, terminal loss, process crash, or host crash can leave a resumable thread with the raw synthetic prompt and restored display |
| Oversized request/SSE exhausts memory | policy body limit; 1 MiB incomplete SSE-frame cap; bounded views/ledger | unit/config validation | concurrent requests and accepted near-limit bodies still consume local resources |

## Fail-closed decisions

The following do not degrade to passthrough:

- invalid or duplicate-key JSON;
- request over the policy limit;
- unsupported top-level, item, content, media, encoding, or control shape;
- normalizer/view/source-map failure;
- random token/event identifier failure;
- invalid rewrite span or serialization failure;
- hard-secret or protected block-only finding;
- inability to persist the pre-egress audit event;
- missing/duplicate malformed live authorization;
- unsupported upstream route/query/destination; and
- restoration requested with the live OpenAI upstream.

## Local-data caveat

AgentVeil receives raw supported values locally in order to classify them. A
tool may already have printed those values to Codex or a terminal before the
gateway sees the next model turn. The gateway prevents supported raw bytes from
crossing its OpenAI request boundary; it does not erase or retroactively secure
local copies.

The interactive `codex-demo` intentionally exercises that local-data caveat
with only the two documented lower-risk synthetic fixtures. Its Codex thread is
resumable while running. AgentVeil verifies recursive deletion on clean exit,
but forced termination can leave synthetic temporary state. Never substitute
real PII or credentials.

Likewise, a model can ask a shell command to upload data through another
network path. AgentVeil does not sandbox or mediate tool network activity. Use
normal Codex sandbox/approval controls and avoid real sensitive data during this
pre-release evaluation.

## Release assumptions

- The user invokes `agentveil codex`, not Codex directly, for the claimed path.
- The resolved Codex binary is the intended official `0.144.4` executable.
- The local OS, user account, certificate store, DNS, and OpenAI endpoint are
  trusted to their normal operational level.
- Policies are reviewed and validated before launch.
- Only synthetic values are used while AgentVeil remains pre-release.
- Any Codex or Responses schema upgrade invalidates the evidence label until the
  payload map and wire tests are refreshed.

Known open items are tracked in [RISK_REGISTER.md](RISK_REGISTER.md).
