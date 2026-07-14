# Demo and verification walkthrough

Generated release reports bind evidence to the exact Git commit and clean-tree
observations. All fixtures below are intentionally synthetic. Never substitute
a real credential, personal record, customer name, internal hostname, or
production address.

The strongest current demo is the packaged deterministic wire proof. The live
Codex step demonstrates the verified route and a synthetic tool-output next
turn, but it is not a replacement for the capturing-upstream zero-connect test.

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

## 3. Inspect the focused deterministic wire test

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
3. Synthetic display restoration returns the originals locally.
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

## 4. Run all verification tests

```sh
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked --all-targets
```

The current build reports 42 passing Rust tests: 37 library, 3 CLI, 1
offline-demo CLI, and 1 gateway integration test. Three Python regressions
cover exact archive construction and inspection, release-report directory
diagnostics, and descriptor cleanup.

## 5. Live synthetic Codex route

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

## 6. What to show in a short video

A defensible sub-three-minute sequence is:

1. **Problem (15 seconds):** Codex tool output can place sensitive-looking data
   into a later model turn.
2. **Boundary (20 seconds):** show the one-line route and state the exact
   Codex/model/transport scope.
3. **Deterministic proof (55 seconds):** run `agentveil demo --check`, then open
   the offline dashboard and explain its capturing-upstream request-count
   assertion.
4. **Live proof (40 seconds):** run the fixture through `agentveil codex`, show
   `TOKEN_ENV_EMPTY` and `WRAPPER_ROUTE_OK`, then run the quiet audit scan.
5. **Evidence (25 seconds):** show the payload map, audit schema, 42-Rust-test
   plus 3-release-script-test result, and fail-closed boundary.
6. **Honest limits (15 seconds):** live restoration disabled; no IDE/cloud/
   WebSocket/media/universal-DLP claim.

The dashboard changes labels by mode: the offline proof shows its synthetic
route, while live mode shows the configured Codex/OpenAI route and “Not
measured.” Never treat displayed status as a substitute for the capturing-
upstream assertion.

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
