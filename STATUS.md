# AgentVeil status

Core evidence was first anchored on July 14, 2026. Public release `v0.1.4` is
the baseline judge artifact. Candidate `v0.1.5` adds the real-Codex
synthetic-loopback demonstration and its narrower delta-only restoration path.
Generated release reports bind results to an exact Git commit and clean-tree
observations; the working-tree claims below require a fresh report before the
next tag.

## Current state

The core privacy engine, strict Responses payload adapter, authenticated
loopback gateway, secure Codex launcher, deterministic wire proof, packaged
offline demo, benchmark harness, and one live synthetic GPT-5.6 Luna
tool-replay proof are working. Live restoration remains disabled. The read-only
dashboard passed desktop browser QA and responsive mobile DOM/overflow checks.
The current candidate's automated checks are value-free; its interactive
`codex-demo` intentionally shows only the documented synthetic email and
private IPv4 fixtures inside an isolated Codex TUI. Final clean release gates,
the replacement video, and completing the Devpost submission remain.

## Claim matrix

| Claim | Current state | Evidence |
|---|---|---|
| Codex custom-provider route | Verified | Codex CLI `0.144.4`, `wire_api = "responses"`, HTTP SSE, GPT-5.6 Luna live route |
| Secure launcher route pinning | Verified | exact client/model checks, randomized provider ID, ephemeral loopback port, routing-override tests |
| Complete supported-body inspection before egress | Verified in fake-upstream boundary | gateway builds protected bytes before `send`; integration test checks zero additional upstream requests on block/reject |
| Hard-secret blocking | Verified for shipped detectors | unit and wire tests cover supported credential/private-key forms and composed encodings |
| Lower-risk masking/tokenization | Verified for shipped policies | engine tests and gateway capture assertions |
| Tool-result protection before next model turn | Verified | fake-upstream function-output assertion and live synthetic custom-tool-output replay |
| Value-free audit schema and persistence gate | Verified for current audit path | typed audit tests, file-mode tests, fake-upstream leak assertions, live synthetic audit scan |
| Session/TTL/capacity token scope | Verified in unit tests | exact lookup, cross-scope denial, expiry, LRU, clear, hard-secret denial |
| Full synthetic-harness display restoration | Verified in tests | supported typed display snapshots, every transport split, unsupported sink preservation, malformed/truncated rejection |
| Codex synthetic delta-only restoration | Verified in tests and interactive smoke | only `response.output_text.delta` is restored; done/item/completion/response snapshots stay tokenized; exact-token replay remains session/TTL scoped |
| Live OpenAI display restoration | Disabled / not claimed | configuration rejects restoration with the live upstream |
| Dashboard | Verified in candidate `v0.1.5` | loopback/read-only/value-free, exact Host authority, local assets, restrictive browser headers, browser QA; state schema v2 adds client plus expanded restoration/wire-proof labels; synthetic proof state only in a capturing harness |
| Packaged offline demo | Verified in candidate `v0.1.5`; release binding pending | isolated environment test runs twice; hard block keeps fake-upstream count unchanged; rewritten bodies, tool re-entry, dashboard, and audit are value-free |
| Interactive real-Codex synthetic demo | Verified in candidate `v0.1.5`; release binding pending | real pinned TUI, model route loopback-only, no model request to OpenAI; TUI state is resumable while running, deleted and verified absent on clean exit, with forced-kill/crash residue explicitly disclosed |
| Universal Codex protection | Not claimed | only the mapped CLI `0.144.4` Responses path is supported |

## Verification result

`cargo test --locked --all-targets` on the current build passed:

```text
43 library tests passed
3 binary CLI tests passed
2 demo CLI integration tests passed
3 gateway end-to-end tests passed
51 total; 0 failed
```

Four focused Python regression tests cover exact archive construction and
inspection, tag/binary version mismatch rejection, release-report directory
error classification, and descriptor cleanup on benchmark validation failures.

The gateway integration test uses only loopback listeners and synthetic
fixtures. It verifies:

- raw synthetic email and private IPv4 values are absent at the fake upstream;
- the full synthetic-harness response restores supported typed display copies;
- the Codex-compatible path restores only live output-text deltas and keeps
  structured completion/history snapshots tokenized;
- direct, encoded, composed, adjacent, structural-key, metadata, and partial
  private-key bypass candidates do not increase the upstream request count;
- invalid percent-encoded UTF-8 fails closed;
- function-call output is protected before the second upstream turn;
- missing local authorization and unsupported fields reject locally; and
- originals and AgentVeil token strings are absent from the audit file.

## Live proof boundary

A renewed synthetic run on July 14, 2026 used the secure launcher, Codex CLI
`0.144.4`, and `gpt-5.6-luna`. A local tool returned a synthetic email and
private IP in a supported output field. The next model turn returned the
expected route marker, and the gateway audit recorded two lifecycle rows for
the rewritten request without storing body values or AgentVeil tokens. The
child shell observed the AgentVeil session environment variable as empty.

The project label in the fixture is configured only by the loopback-only demo
policy, not the live default policy. The recommended live command therefore
prints the email and private-IP lines plus `TOKEN_ENV_EMPTY`;
`TOKEN_ENV_PRESENT` is a failure signal. Audit metadata demonstrates the dated
route decision, not what crossed the remote wire; the capturing fake-upstream
test remains the egress proof.

This proves one dated live path, not every Codex workflow or future CLI version.

## Remaining submission work

1. Run the final clean release/leak, benchmark, Python, and synthetic proof
   gates and bind them to the release commit.
2. Capture and approve the public sub-three-minute killer-demo video, verify it
   signed out, and complete the Devpost entry without enabling live
   restoration.

Any subsequent release source change invalidates the prior clean-tree evidence
and requires the release/leak gate, default benchmark, and synthetic proofs to
be repeated before publishing another tag.

The working milestone is July 17, 2026. The Devpost submission deadline is
July 21, 2026 at 5:00 PM Pacific / 8:00 PM Eastern.
