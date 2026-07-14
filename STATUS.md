# AgentVeil status

Core evidence was first anchored on July 14, 2026. The current public release
includes the offline demo, value-free loopback dashboard, hardened audit sink,
benchmark, self-contained judge documentation, and stronger release evidence.
Generated release reports bind their results to the exact Git commit and
clean-tree observations; they remain private, value-free local evidence rather
than committed runtime state.

## Current state

The core privacy engine, strict Responses payload adapter, authenticated
loopback gateway, secure Codex launcher, deterministic wire proof, packaged
offline demo, benchmark harness, and one live synthetic GPT-5.6 Luna
tool-replay proof are working. Live restoration remains disabled. The read-only
dashboard passed desktop browser QA and responsive mobile DOM/overflow checks.
The clean release/leak gate, default benchmark, renewed fake-upstream proof, and
renewed live synthetic proof passed on the current release. Prebuilt Ubuntu
x86_64 and macOS arm64 artifacts are public and checksum-verified. Recording
the video, running `/feedback`, and completing the Devpost submission remain.

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
| Synthetic SSE display restoration | Verified in tests | every transport split, unsupported sink preservation, malformed/truncated rejection |
| Live OpenAI display restoration | Disabled / not claimed | configuration rejects restoration with the live upstream |
| Dashboard | Verified in current release | loopback/read-only/value-free, exact Host authority, local assets, restrictive browser headers, browser QA; synthetic proof state only in the capturing harness |
| Packaged offline demo | Verified in current release | isolated environment test runs twice; hard block keeps fake-upstream count unchanged; rewritten bodies, tool re-entry, dashboard, and audit are value-free |
| Universal Codex protection | Not claimed | only the mapped CLI `0.144.4` Responses path is supported |

## Verification result

`cargo test --locked` on the current build passed:

```text
37 library tests passed
3 CLI tests passed
1 offline-demo CLI test passed
1 gateway integration test passed
42 total; 0 failed
```

Two focused Python regression tests also cover release-report directory error
classification and descriptor cleanup on benchmark validation failures.

The gateway integration test uses only loopback listeners and synthetic
fixtures. It verifies:

- raw synthetic email and private IPv4 values are absent at the fake upstream;
- the local synthetic restoration response returns the exact originals;
- direct, encoded, composed, adjacent, structural-key, metadata, and partial
  private-key bypass candidates do not increase the upstream request count;
- invalid percent-encoded UTF-8 fails closed;
- function-call output is protected before the second upstream turn;
- missing local authorization and unsupported fields reject locally; and
- originals and AgentVeil token strings are absent from the audit file.

## Live proof boundary

A renewed synthetic run on July 14, 2026 used the secure launcher, Codex CLI
`0.144.4`, and `gpt-5.6-luna`. A local tool read a fixture containing a synthetic
email, private IP, and project label. The next model turn returned the expected
route marker. The gateway audit contained two rewritten events and contained
none of the originals or AgentVeil tokens. The child shell observed the
AgentVeil session environment variable as empty.

This proves one dated live path, not every Codex workflow or future CLI version.

## Remaining submission work

1. Run `/feedback` in the main Codex development task and disposition the
   resulting feedback.
2. Capture the public sub-three-minute YouTube video and complete the Devpost
   entry.

Any subsequent release source change invalidates the prior clean-tree evidence
and requires the release/leak gate, default benchmark, and synthetic proofs to
be repeated before publishing another tag.

The working milestone is July 17, 2026. The Devpost submission deadline is
July 21, 2026 at 5:00 PM Pacific / 8:00 PM Eastern.
