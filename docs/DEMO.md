# Demo and verification walkthrough

Generated release reports bind evidence to the exact Git commit and clean-tree
observations. All fixtures below are intentionally synthetic. Never substitute
a real credential, personal record, customer name, internal hostname, or
production address.

The strongest automated evidence is the packaged deterministic wire proof. Its
terminal output, dashboard, audit, and generated reports are value-free. The
presentation path is a separate real-Codex TUI whose model route points only to
a capturing synthetic loopback fixture; it intentionally displays the
documented synthetic email and private IPv4 values after local delta
restoration. Neither path makes an OpenAI model request. This does not claim
the Codex process makes no other network request. The separately scoped live
route remains useful evidence, but it is not a replacement for the capturing-
upstream zero-connect test.

## 1. Preflight

From an extracted release archive or the repository root, select the binary for
that layout. A source checkout builds first; an archive does not need Cargo:

```sh
if [ -x ./agentveil ]; then
  AGENTVEIL_BIN=./agentveil
else
  cargo build --release --locked
  AGENTVEIL_BIN=./target/release/agentveil
fi
"$AGENTVEIL_BIN" doctor
"$AGENTVEIL_BIN" policy-validate policies/default.yaml
"$AGENTVEIL_BIN" policy-validate policies/demo.yaml
```

`doctor` must report Codex `0.144.4`, available authentication, and loopback
binding. The policy commands print only name/hash/status metadata.

## 2. Run the packaged offline demo

For the release-safe, value-free check:

```sh
"$AGENTVEIL_BIN" demo --check
```

Exact expected output:

```text
demo-check: status=pass route=synthetic_loopback allow=pass tokenize=pass zero_connect=pass tool_reentry=pass dashboard=pass audit=pass output=value_free
```

To keep its dashboard open:

```sh
"$AGENTVEIL_BIN" demo
```

Open the printed loopback URL, then stop the process with Ctrl-C. The command
creates one ephemeral fake Responses server and one ephemeral AgentVeil
listener, runs the synthetic proof, and removes its private audit directory on
exit. It clears no user state, reads no Codex login, uses no DNS or environment
proxy, and makes no OpenAI request.

The dashboard's “Synthetic wire proof: Passed” state is available only in this
capturing harness. A normal live session truthfully displays “Not measured.”

## 3. Run the real Codex synthetic round trip

This is the presentation path for transparent, lower-risk token restoration:

```sh
"$AGENTVEIL_BIN" codex-demo --check
"$AGENTVEIL_BIN" codex-demo
```

The `--check` form is non-interactive: a synthetic client exercises the
Codex-compatible request and stream contract, emits only a value-free pass/fail
summary, and does not launch Codex. Its exact output is:

```text
codex-demo-check: status=pass route=synthetic_client_codex_compatible block=pass tokenize=pass restore_delta=pass replay=pass wire_proof=pass audit=pass output=value_free
```

In a source checkout, `scripts/demo-check` runs both automated modes and emits
exactly the offline line from section 2 followed by this Codex-compatible line.
The release check captures both lines as value-free evidence.

The interactive form launches the real pinned Codex CLI with a private
temporary home and synthetic-only workspace. Its provider routes to an
authenticated AgentVeil gateway and then to a capturing loopback fixture. It
does not read Codex login state and makes no OpenAI model request because the
model route is loopback-only. This is not a categorical no-network claim about
the Codex process. Codex 0.144.4's interactive TUI keeps a resumable thread
inside that isolated home while it runs. It is not an ephemeral Codex session:
AgentVeil recursively removes the entire private runtime and verifies its
absence only after a clean exit.

Enter this exact one-line synthetic prompt:

```text
Return this explicitly synthetic test config unchanged: const SUPPORT_EMAIL = "ava.agentveil@example.test"; const BUILD_HOST = "10.24.8.15";
```

Open the printed dashboard URL in the same clean window. The first request must
show two protected lower-risk findings and `Synthetic wire proof: Passed`. The
capturing fixture receives issued tokens rather than the email or private IPv4
originals. It returns those tokens in a typed Responses stream. AgentVeil
restores only the live `response.output_text.delta`, so Codex displays the
original synthetic assignments without user-side token handling.

All done/item/completion/response snapshot events remain tokenized on the wire.
Only `response.output_text.delta` is restored. To demonstrate same-session
replay, enter:

```text
Repeat the synthetic configuration exactly.
```

Codex replays the owned assistant tokens and also resends the earlier raw user
prompt from its local thread. AgentVeil admits the exact tokens only for the
same unexpired session, independently re-detects and re-tokenizes the prior raw
values, and again restores only the live response delta. The dashboard should
show `Protected history replayed` and note that prior values were re-protected.
The capturing fixture still rejects either original on the wire. Stop if the
dashboard is not explicitly labeled `Synthetic display restoration · model
route loopback-only`, if an unknown token is accepted, or if the wire proof is
not passed.

Exit normally so AgentVeil can verify removal of the private runtime. A forced
kill, terminal loss, process crash, or host crash can leave a resumable Codex
thread containing the raw synthetic prompt and restored synthetic display in
temporary state. This demo is never a safe place for real PII or credentials.

## 4. Inspect the focused deterministic wire test

This source-only test requires the public repository checkout and Rust
toolchain. Archive users can skip to the live synthetic route after the
packaged proof.

```sh
cargo test --locked \
  --test gateway_e2e \
  wire_proof_tokenizes_blocks_restores_and_keeps_audit_value_free \
  -- --exact --nocapture
```

This test makes no OpenAI request. It starts two ephemeral loopback servers:

```text
test client -> AgentVeil -> capturing fake Responses upstream
```

The single test proves all of the following in one process:

1. A synthetic email/private IPv4 request succeeds.
2. The fake upstream body contains typed AgentVeil replacements and does not
   contain the originals.
3. Full synthetic-harness restoration returns the originals in its supported
   typed assistant display copies. This is broader than the Codex demo's
   delta-only restoration and remains synthetic-only.
4. A direct synthetic credential blocks with
   `protected_request_bytes_forwarded: 0`.
5. Percent-encoded, Unicode-compatible, JSON-escaped, separator-composed,
   adjacent-prefix, partial/encrypted-PEM, structural-key, and metadata bypass
   candidates do not increase the fake-upstream request count.
6. Invalid percent-decoded UTF-8 fails closed without a new upstream request.
7. A synthetic `function_call_output` is protected before the next fake model
   turn.
8. Unknown fields and missing local authentication reject locally.
9. The audit file contains typed decision/finding metadata but none of the
   originals, hard-secret fixture, percent-encoded fixture, or AgentVeil token
   strings.

Expected terminal result:

```text
test wire_proof_tokenizes_blocks_restores_and_keeps_audit_value_free ... ok
test result: ok. 1 passed; 0 failed
```

Do not claim “zero leak” from a screenshot of rewritten JSON. The meaningful
evidence is that the fake-upstream request count remains unchanged on every
block/reject path and that its captured accepted bodies exclude originals.

## 5. Run all verification tests

```sh
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked --all-targets
```

The current build reports 51 passing Rust tests: 43 library, 3 binary CLI, 2
demo CLI integration, and 3 gateway end-to-end tests. Four Python regressions
cover exact archive construction and inspection, tag/binary version mismatch
rejection, release-report directory diagnostics, and descriptor cleanup. Bind
these working-tree counts to the final clean release report before using them
in a public artifact.

## 6. Live synthetic Codex route

This step contacts OpenAI using existing Codex login state. It demonstrates one
dated live path only. Keep `--ephemeral` and use the synthetic fixture included
in both the repository and release archive exactly as shipped.

First inspect the fixture so the audience can see that every value is labeled
synthetic:

```sh
sed -n '1,20p' fixtures/demo/synthetic-context.txt
```

Then launch the protected turn:

```sh
AUDIT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/agentveil-live-demo.XXXXXX")"
chmod 700 "$AUDIT_DIR"
AUDIT_PATH="$AUDIT_DIR/audit.jsonl"
trap 'rm -f "$AUDIT_PATH"; rmdir "$AUDIT_DIR"' EXIT
"$AGENTVEIL_BIN" codex \
  --reasoning-effort none \
  --audit "$AUDIT_PATH" \
  -- \
  exec --ephemeral --skip-git-repo-check \
  'Use exactly one local shell tool call. Its command must be exactly:
sed -n "2,3p" fixtures/demo/synthetic-context.txt; if [ -z "${AGENTVEIL_SESSION_TOKEN:-}" ]; then printf "%s\n" TOKEN_ENV_EMPTY; else printf "%s\n" TOKEN_ENV_PRESENT; fi
Do not redirect, pipe, truncate, or suppress stdout. After the tool result arrives, do not quote it; reply exactly WRAPPER_ROUTE_OK.'
```

The launcher prints a pseudonymous session, Responses/SSE transport, ephemeral
loopback binding, and `gpt-5.6-luna`. The expected final model marker is:

```text
WRAPPER_ROUTE_OK
```

While the session is active, open
`http://<binding-from-the-launcher>/dashboard`. It is loopback-only, read-only,
and deliberately unauthenticated because its state is value-free and no gateway
credential should enter browser state. Show it as visibility, not wire proof.

The shell output visibly contains the known synthetic email and private-IP
lines plus `TOKEN_ENV_EMPTY` because it is local. `TOKEN_ENV_PRESENT` is a
failure signal: stop the demo if it appears. The protected classes are covered
by the live default policy; the marker confirms the child shell did not inherit
AgentVeil's gateway credential. The project label is configured only by the
loopback-only demo policy and is deliberately excluded from this live tool
output. The privacy assertion concerns the next model request: the gateway
scans the tool output before forwarding it. Live response restoration is
disabled, so the model receives the configured mask/token replacements, not
restored originals.

The first dashboard request row can be `Structure accepted`; it contains the
safe instruction. The next row must show protected finding metadata for the
tool output. If it also says `No protected class was present`, stop: the tool did
not return the fixture text in a supported output field.

Check the audit without printing its contents:

```sh
test -f "$AUDIT_PATH" && test -r "$AUDIT_PATH" || {
  echo 'FAIL: audit evidence is missing or unreadable'
  exit 1
}
set +e
rg -q 'ava\.agentveil@example\.test|10\.24\.8\.15|\[AV_' "$AUDIT_PATH"
scan_status=$?
set -e
case "$scan_status" in
  0) echo 'FAIL: synthetic original or token found in audit'; exit 1 ;;
  1) echo 'PASS: audit contains no synthetic originals or AgentVeil tokens' ;;
  *) echo 'FAIL: audit scan could not complete'; exit 1 ;;
esac

if rg -q '"decision":"rewritten"' "$AUDIT_PATH" \
  && rg -q '"data_class":"email"' "$AUDIT_PATH" \
  && rg -q '"data_class":"private_ipv4"' "$AUDIT_PATH" \
  && rg -q '"source_field":"(custom_tool_output|function_output)"' "$AUDIT_PATH"; then
  echo 'PASS: audit contains rewritten synthetic tool-output finding metadata'
else
  echo 'FAIL: expected rewritten tool-output metadata is missing'
  exit 1
fi
```

These audit checks confirm the dated route decision and audit-value invariant;
they do not capture the OpenAI wire. The fake-upstream test remains the proof of
what crossed the controlled egress boundary.

## 7. Killer-demo video plan

Use one clean 16:9 window for the core shot: the AgentVeil dashboard above and
the real Codex TUI below. No repository sidebar, unrelated task, auth state,
personal path, token, mapping, request body, or audit body may be visible.

A defensible 2:35 sequence is:

1. **Problem (0:00-0:12):** useful coding context can contain personal data,
   private infrastructure, and credentials.
2. **Boundary (0:12-0:25):** launch `agentveil codex-demo`; show the real Codex
   CLI, authenticated AgentVeil gateway, capturing synthetic loopback fixture,
   and explicit `model route loopback-only` label.
3. **One-shot transparent round trip (0:25-1:35):** with dashboard and TUI in
   the same frame, type the exact first synthetic prompt. Show the value-free
   activity update before Codex displays the exact synthetic assignments. Type
   the replay prompt, show `Protected history replayed`, and show the same exact
   local display again. Say plainly that only `response.output_text.delta` is
   restored; completion/history snapshots stay tokenized.
4. **Hard-block proof (1:35-1:55):** show the packaged synthetic credential
   case as blocked with zero additional upstream requests. Credentials and
   private keys are never tokenized, stored in the ledger, or restored.
5. **Evidence and meaningful model use (1:55-2:28):** show the separate dated
   live GPT-5.6 Luna marker as route evidence, then the final clean 51-Rust-test
   plus 4-Python-regression result. Keep the live route's restoration disabled
   and its wire proof `Not measured`.
6. **Honest close (2:28-2:35):** public repository, exact verified scope,
   synthetic fixtures only, and no universal-DLP claim.

The interactive TUI is the only public shot that may show the two documented
synthetic lower-risk originals. Automated check output, dashboard state, audit,
and reports must remain value-free. If the dashboard reports `Synthetic wire
proof: Failed`, an unknown token is accepted, any raw fixture reaches the
capturing boundary, or cleanup does not verify the private root absent, discard
the run. Never treat displayed status alone as wire evidence.

## Approved claim language

Use:

> For Codex CLI 0.144.4 Responses/SSE requests matching the documented payload
> map, AgentVeil fully inspects the local JSON body before send, blocks supported
> hard-secret classes with zero fake-upstream requests, and masks or tokenizes
> configured lower-risk values. A synthetic live GPT-5.6 Luna tool replay and a
> deterministic capturing-upstream test validate the path.

Do not say:

- “AgentVeil guarantees no secrets ever reach AI.”
- “AgentVeil protects all Codex clients, tools, files, or network traffic.”
- “The dashboard proves no leak.”
- “Restoration is safe in live Codex.”
- “Every encoding or credential format is detected.”

## Demo failure policy

If the client version, payload shape, audit sink, detector, fake-upstream count,
or leak scan differs from the expected result, stop. Record the evidence gap;
do not disable a check, change a fixture into a less adversarial shape, or route
around AgentVeil to make the presentation pass.
