# Privacy and data handling

Evidence anchor: commit `ed09354`. AgentVeil is local software, but “local” does
not mean “no data is sent to OpenAI.” Its purpose is to change or block supported
values before a normal Codex model request is sent.

## Data AgentVeil receives locally

For the supported route, the gateway receives:

- the complete uncompressed JSON body of each Codex Responses request, up to the
  active policy limit;
- normal Codex/OpenAI request headers, including authorization and selected
  account/project/session metadata;
- an invocation-scoped AgentVeil local-session credential;
- model-discovery requests; and
- the streamed upstream response.

AgentVeil must briefly hold the raw outbound body in process memory to parse and
classify it. Request buffers, normalization views, and rewrite working strings
are not guaranteed to be securely erased from allocator memory. Restorable
ledger values use zeroizing wrappers, but that is not a whole-process memory
sanitization claim.

## Data sent to OpenAI

When a request is allowed, OpenAI receives:

- the validated and reserialized body after the configured block/mask/tokenize
  actions;
- the standard authorization needed by Codex; and
- only allowlisted operational headers, which can include ChatGPT account ID,
  organization/project ID, session/thread/request identifiers, Codex feature
  metadata, originator, and user agent when Codex supplied them.

AgentVeil does not anonymize or replace those operational headers. It strips its
own local-session credential, cookies, and headers outside the allowlist. In
synthetic loopback-test mode it strips authorization and sends a fixed
`X-AgentVeil-Synthetic: true` marker instead.

If the request is blocked, rejected, unauthenticated, unsupported, or cannot be
audited, AgentVeil does not send its request body upstream. The wire test also
asserts that the fake upstream receives no additional request for its blocked
and rejected cases.

## Local mappings

Token mappings exist only in the gateway process:

- 128-bit random tokens;
- scoped to one generated session;
- exact-token lookup only;
- TTL-bound (15 minutes for tokenized classes in the default policy);
- capacity-bound (4,096 entries in the default policy); and
- removed on expiry, LRU eviction, explicit scope clear, or process exit.

The ledger is not written to disk, synchronized, uploaded, or exposed through
an enumeration API. Credentials, private keys, JWTs, credentialed database URLs,
and unknown AgentVeil tokens are never valid ledger classes.

An evicted or expired token cannot be restored. If such a token later appears in
a supported outbound request, the unknown-token rule blocks it rather than
forwarding an unowned token literal.

## Local audit records

The default audit destination is `runtime/agentveil.audit.jsonl`. `runtime/` and
`*.audit.jsonl` are ignored by Git. The launcher accepts an alternate local path;
the operator remains responsible for choosing a private location outside shared
or synchronized directories.

Each Responses audit row contains only:

- schema version, random event ID, timestamp, and pseudonymous session ID;
- request sequence and route;
- decision (`allowed`, `rewritten`, `blocked`, or `rejected`);
- typed finding summaries: detector, data class, confidence, action, source
  field, and count;
- upstream lifecycle state and scan latency;
- policy name/hash; and
- AgentVeil version.

One request can produce multiple lifecycle rows with the same event ID (for
example, `pending` before send and `started` after the upstream accepts the
request). The current implementation does not write model-discovery audit rows.

The audit schema has no field for request/response bodies, matched values,
surrounding context, replacement tokens, mappings, authorization, cookies,
queries, or filesystem paths. The sink rejects symlink, non-directory, or
group/other-accessible direct parents without changing their permissions. It
can create one missing dedicated leaf as `0700`, opens the file `0600` with
no-follow semantics from the canonical private parent, and flushes and syncs
each record. Audit write or sync failure blocks protected egress.

Audit metadata can still reveal that a category was detected at a time and in a
field. Treat the file as private operational data. Retention and deletion are
manual at this anchor.

## Logs and telemetry

The anchor contains no remote AgentVeil telemetry, analytics service, cloud
control plane, crash uploader, or update checker. Runtime logging is limited to
value-free lifecycle messages such as the loopback address and audit-health
warning. AgentVeil does not intentionally log bodies, headers, mappings,
originals, tokens, or upstream response text.

This does not control logging by Codex, OpenAI, the operating system, a shell
tool, terminal, debugger, endpoint-security product, or process supervisor.

## Responses and restoration

Live OpenAI Responses SSE is passed through to Codex without restoration. This
means a private-IP/internal-hostname token that reached the model remains a token
in the live response.

Exact-token restoration is available only when the gateway is explicitly using
an unauthenticated loopback synthetic upstream. It is limited to supported
assistant display-text event fields and the originating session/TTL. AgentVeil
refuses to enable it with the live upstream because restored originals might be
stored in or replayed from local Codex transcripts.

AgentVeil does not inspect live response text as a DLP boundary. If Codex later
includes response or tool output in a supported outbound request, that next
request is inspected at egress.

## Health endpoint

`GET /health` is available only through the loopback listener and returns
value-free status labels: binding, transport, upstream mode, restoration mode,
and audit health. It does not expose the local credential, policy values,
mappings, findings, request history, or authentication material.

## Dashboard

The current build serves a read-only dashboard on the same loopback listener.
It uses embedded local assets, makes no remote asset requests, and sets CSP,
no-store, nosniff, frame/referrer/permissions, and same-origin headers. State is
limited to status labels, a session pseudonym, policy name, request/count
metadata, and up to 64 typed value-free audit activities.

The dashboard is deliberately unauthenticated: it has no mutation, and exposing
the gateway capability to a browser would create a new credential surface. Do
not port-forward or reverse-proxy the listener. Every dashboard request must
use the listener's exact Host authority, limiting browser DNS rebinding without
placing a capability in browser state. `wire_proof` is `synthetic_passed` only
inside the capturing offline demo; live mode reports `not_measured`. Use the
fake upstream, not UI state alone, for wire proof.

## Deletion

- Stop the AgentVeil gateway to drop in-memory mappings.
- Delete the chosen local audit JSONL file when its operational evidence is no
  longer needed.
- Review Codex, shell, terminal, and OS retention separately; AgentVeil does not
  delete their state.

During pre-release testing, use only the synthetic fixtures shipped in this
repository. If a real credential is ever exposed, revoke it at its issuer first
and report the issue privately under [../SECURITY.md](../SECURITY.md).
