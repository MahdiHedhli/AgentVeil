# Demo and verification walkthrough

Evidence anchor: commit `ed09354`. All fixtures below are intentionally
synthetic. Never substitute a real credential, personal record, customer name,
internal hostname, or production address.

The strongest current demo is the packaged deterministic wire proof. The live
Codex step demonstrates the verified route and a synthetic tool-output next
turn, but it is not a replacement for the capturing-upstream zero-connect test.

## 1. Preflight

From the repository root:

```sh
cargo build --release --locked
./target/release/agentveil doctor
./target/release/agentveil policy-validate policies/default.yaml
./target/release/agentveil policy-validate policies/demo.yaml
```

`doctor` must report Codex `0.144.4`, available authentication, and loopback
binding. The policy commands print only name/hash/status metadata.

## 2. Run the packaged offline demo

For the release-safe, value-free check:

```sh
./target/release/agentveil demo --check
```

Exact expected output:

```text
demo-check: status=pass route=synthetic_loopback allow=pass tokenize=pass zero_connect=pass tool_reentry=pass dashboard=pass audit=pass output=value_free
```

To keep its dashboard open:

```sh
./target/release/agentveil demo
```

Open the printed loopback URL, then stop the process with Ctrl-C. The command
creates one ephemeral fake Responses server and one ephemeral AgentVeil
listener, runs the synthetic proof, and removes its private audit directory on
exit. It clears no user state, reads no Codex login, uses no DNS or environment
proxy, and makes no OpenAI request.

The dashboard's “Synthetic wire proof: Passed” state is available only in this
capturing harness. A normal live session truthfully displays “Not measured.”

## 3. Inspect the focused deterministic wire test

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

The current build reports 42 passing tests: 37 library, 3 CLI, 1 offline-demo
CLI, and 1 gateway integration test.

## 5. Live synthetic Codex route

This step contacts OpenAI using existing Codex login state. It demonstrates one
dated live path only. Keep `--ephemeral` and use the repository fixture exactly
as shipped.

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
./target/release/agentveil codex \
  --reasoning-effort none \
  --audit "$AUDIT_PATH" \
  -- \
  exec --ephemeral --skip-git-repo-check \
  'Use one local shell command to read fixtures/demo/synthetic-context.txt and report TOKEN_ENV_EMPTY if AGENTVEIL_SESSION_TOKEN is empty in that shell. Then reply with exactly WRAPPER_ROUTE_OK. Do not repeat any fixture value.'
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

The shell output may visibly contain the known synthetic fixture because it is
local. The privacy assertion concerns the next model request: the gateway scans
the tool output before forwarding it. Live response restoration is disabled, so
the model receives the configured mask/token replacements, not restored
originals.

Check the audit without printing its contents:

```sh
test -f "$AUDIT_PATH" && test -r "$AUDIT_PATH" || {
  echo 'FAIL: audit evidence is missing or unreadable'
  exit 1
}
set +e
rg -q 'ava\.agentveil@example\.test|10\.24\.8\.15|PROJECT-VEIL-DEMO|\[AV_' "$AUDIT_PATH"
scan_status=$?
set -e
case "$scan_status" in
  0) echo 'FAIL: synthetic original or token found in audit'; exit 1 ;;
  1) echo 'PASS: audit contains no synthetic originals or AgentVeil tokens' ;;
  *) echo 'FAIL: audit scan could not complete'; exit 1 ;;
esac
```

This audit check is useful evidence, but it does not capture the OpenAI wire.
The fake-upstream test remains the proof of what crossed the controlled egress
boundary.

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
5. **Evidence (25 seconds):** show the payload map, audit schema, 42-test result,
   and fail-closed boundary.
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
