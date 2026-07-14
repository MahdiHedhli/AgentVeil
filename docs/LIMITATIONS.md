# Limitations

Evidence anchor: commit `ed09354`. These limits are part of the product
contract, not a backlog of implied protection.

## Verified client and route only

- Exact Codex CLI version `0.144.4` only. The launcher rejects other versions.
- Exact model `gpt-5.6-luna` only. The launcher rejects another model name.
- Local CLI custom-provider traffic over HTTP Responses/SSE only.
- The dated live proof used normal Codex ChatGPT authentication. The code also
  contains fixed OpenAI API endpoints, but that is not a broader client claim.
- No Responses WebSocket, Codex IDE, Codex cloud task, remote-control/app-server,
  browser ChatGPT, or other AI client/provider path.
- Unix-like hosts only at this anchor. Executable permission checks and secure
  audit-file flags use Unix APIs. No Windows support claim.

Running Codex outside `agentveil codex`, or directly using another provider,
bypasses the boundary. A future Codex request shape may reject even if the CLI
version string is unchanged.

## Payload coverage is an allowlist

AgentVeil supports only the exact top-level fields, item types, content parts,
and controls in [PAYLOAD_MAP.md](PAYLOAD_MAP.md). Unknown or wrong-typed shapes
reject rather than pass through.

Unsupported at the anchor:

- images, image-generation items, input files, audio, video, OCR, and binary
  bodies;
- compressed request bodies;
- multipart/form-data and non-JSON requests;
- arbitrary encrypted/opaque payload interpretation (accepted encrypted strings
  are only type-checked and scanned as block-only text, not decrypted);
- unknown tool/content/compaction formats; and
- requests with `store: true`, non-streaming Responses, or a non-`auto`
  `tool_choice`.

Model discovery is proxied through a separate fixed GET route. It does not
contain a protected body and is not represented in the current audit JSONL.

## Detection is not perfect DLP

The shipped patterns cover selected API-key families, credential assignments,
Bearer tokens, JWTs, credentialed database URLs, PEM private-key material,
email, contextual US-style phone numbers, RFC1918 IPv4, and configured internal
hostnames/home paths/custom terms.

Notable gaps include:

- credential formats not encoded in the current detectors;
- IPv6 and many public/internal addressing conventions;
- international phone formats and identifiers beyond the documented classes;
- secrets represented as hashes, encryption, base64 fragments, arithmetic,
  multiple remote turns, or semantic descriptions;
- values split across distinct JSON fields; and
- arbitrary nested or repeated encoding beyond the bounded transform graph.

Normalization is deliberately bounded to avoid CPU/memory amplification: two
decode layers, eight regular views, and twelve total scan views. Exceeding a
bound or producing invalid decoded UTF-8 fails closed. This favors privacy over
availability but does not establish exhaustive decoding.

Pattern systems can produce both false negatives and false positives. A
high-specificity shape in ordinary text may block a safe request. Mask/tokenize
findings in structural fields also become a whole-request block to preserve
protocol integrity.

## Local execution remains local risk

AgentVeil mediates the Codex model request, not every action Codex or a tool can
perform.

- A shell command, MCP server, browser, or other tool can send data over its own
  network connection outside AgentVeil.
- Raw tool output can appear in a terminal or local Codex state before its next
  model turn reaches the gateway.
- AgentVeil does not remove values already stored in repositories, shell
  history, transcripts, caches, swap, backups, logs, or screenshots.
- A privileged or compromised same-user process can inspect process memory or
  authentication through channels outside the local header design.
- The launcher clears its session-token variable for Codex-created shells, but
  this is not a general OS sandbox.

Use normal Codex sandbox and approval controls. This pre-release build must be
tested with synthetic values only.

## Live restoration is disabled

Tokenized private IPs/internal hostnames remain AgentVeil tokens in live model
responses. AgentVeil refuses `restore_display_text` when the upstream is OpenAI.

Synthetic restoration is evidence code with narrower guarantees:

- exact owned token, same session, and unexpired mapping only;
- selected typed assistant display-text fields only;
- no tool argument, command, URL, patch, identifier, error, reasoning metadata,
  or unknown-event restoration;
- framing across every transport byte split is tested, but a token split across
  multiple semantic delta events is not reassembled; and
- malformed/truncated or multiple-data-line SSE frames fail the local stream.

The synthetic adapter does not establish Codex transcript retention safety.
That is why the functionality is not enabled live.

## Token ledger tradeoffs

- Mappings are process-local and disappear on restart/crash.
- Default token TTL is 15 minutes; maximum policy TTL is 30 minutes.
- The default capacity is 4,096 entries. When full, least-recently-used entries
  are evicted, including potentially still-unexpired entries.
- An old/evicted legitimate token cannot restore and will block if replayed
  outbound as an unowned AgentVeil token.
- Values are zeroized on ledger drop, but Rust/allocator/OS behavior prevents a
  guarantee that all copies created during scanning are erased from memory.

## Gateway and audit tradeoffs

- Loopback plus an unguessable local capability prevents casual unauthenticated
  use; it is not isolation from a malicious local administrator or capable
  same-user process.
- Live authorization and selected account/session headers still pass to the
  fixed OpenAI endpoint. AgentVeil is not an identity anonymizer.
- The entire accepted request is buffered and scanned before send. Near-limit or
  concurrent requests consume memory, and the privacy engine is serialized by a
  mutex.
- The default body limit is 4 MiB; an incomplete synthetic-restoration SSE frame
  is capped at 1 MiB.
- Audit writes and flushes are synchronous. Slow storage can delay requests.
- Audit rotation, retention, export, and deletion are manual.
- The sink canonicalizes and verifies its private direct parent, never chmods a
  pre-existing directory, and uses no-follow for the final file. It still does
  not provide descriptor-relative proof for every intermediate path component;
  choose a trusted local base directory.
- The default relative audit path depends on the current working directory. It
  is Git-ignored in this repository, but an alternate/shared directory can leak
  enforcement metadata.
- A pre-send `pending` audit row proves that the gate opened; it does not prove
  the model received or completed the request. A later `started`/`failed` row is
  best-effort lifecycle evidence.

## Packaging and operations

- The anchor is source distribution, not a signed/notarized release package.
- The launcher trusts the first suitable external `codex` executable resolved
  from `PATH` after version/login checks; it does not verify a code signature or
  artifact hash.
- Exact version pinning intentionally makes Codex upgrades unavailable until
  AgentVeil’s payload map and live proof are refreshed.
- There is no automatic update, policy distribution, key management, remote
  telemetry, enterprise admin control, multi-user service, or cloud ledger.
- The dashboard is read-only, loopback-only, exact-Host validated, and value-
  free by construction, but intentionally unauthenticated. A local process can
  still read its typed metadata; never port-forward it. “Synthetic wire proof”
  is shown as passed only in the capturing demo and is not a live network
  sensor.
- The packaged demo proves an offline synthetic route, not compatibility with a
  future Codex release or a broader live payload shape.

Open security and evidence work is tracked in
[RISK_REGISTER.md](RISK_REGISTER.md).
