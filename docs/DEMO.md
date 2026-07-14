# Demo and verification walkthrough

Evidence anchor: commit `ed09354`. All fixtures below are intentionally
synthetic. Never substitute a real credential, personal record, customer name,
internal hostname, or production address.

The strongest current demo is the deterministic wire proof. The live Codex step
demonstrates the verified route and a synthetic tool-output next turn, but it is
not a replacement for the zero-connect test.

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

## 2. Run the deterministic wire proof

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

## 3. Run all verification tests

```sh
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
```

The current build reports 38 passing tests: 34 library, 3 CLI, and 1 gateway
integration test.

## 4. Live synthetic Codex route

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
AUDIT_PATH="/tmp/agentveil-live-demo.audit.jsonl"
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
if rg -q 'ava\.agentveil@example\.test|10\.24\.8\.15|PROJECT-VEIL-DEMO|\[AV_' "$AUDIT_PATH"; then
  echo 'FAIL: synthetic original or token found in audit'
  exit 1
else
  echo 'PASS: audit contains no synthetic originals or AgentVeil tokens'
fi
```

This audit check is useful evidence, but it does not capture the OpenAI wire.
The fake-upstream test remains the proof of what crossed the controlled egress
boundary.

## 5. What to show in a short video

A defensible sub-three-minute sequence is:

1. **Problem (15 seconds):** Codex tool output can place sensitive-looking data
   into a later model turn.
2. **Boundary (20 seconds):** show the one-line route and state the exact
   Codex/model/transport scope.
3. **Deterministic proof (50 seconds):** run the focused gateway test and explain
   the fake-upstream request-count assertion.
4. **Live proof (45 seconds):** run the fixture through `agentveil codex`, show
   `TOKEN_ENV_EMPTY` and `WRAPPER_ROUTE_OK`, then run the quiet audit scan.
5. **Visibility and evidence (25 seconds):** show the value-free dashboard, then
   the payload map, audit schema, 38-test result, and fail-closed boundary.
6. **Honest limits (15 seconds):** live restoration disabled; no IDE/cloud/
   WebSocket/media/universal-DLP claim.

The dashboard is a post-anchor visibility surface. Update the release anchor and
rerun the proof before presenting the final build; never treat its displayed
count as a substitute for the capturing-upstream assertion.

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
