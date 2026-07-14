# AgentVeil status

Core evidence anchor: commit `ed09354` on July 14, 2026. The current build adds
the value-free loopback dashboard and two tests; the final release commit must
refresh the anchor after rerunning all gates.

## Current state

The core privacy engine, strict Responses payload adapter, authenticated
loopback gateway, secure Codex launcher, deterministic wire proof, and one live
synthetic GPT-5.6 Luna tool-replay proof are working. Live restoration remains
disabled. A read-only value-free dashboard is now implemented on loopback. The
packaged demo command, benchmark, final release proof, video, and Devpost
submission remain unfinished.

## Claim matrix

| Claim | State at `ed09354` | Evidence |
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
| Dashboard | Implemented after core anchor | loopback/read-only/value-free, local assets, restrictive browser headers, 2 tests; not wire proof |
| Universal Codex protection | Not claimed | only the mapped CLI `0.144.4` Responses path is supported |

## Verification result

`cargo test --locked` on the current build passed:

```text
34 library tests passed
3 CLI tests passed
1 gateway integration test passed
38 total; 0 failed
```

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

A manual synthetic run against this anchor used the secure launcher, Codex CLI
`0.144.4`, and `gpt-5.6-luna`. A local tool read a fixture containing a synthetic
email, private IP, and project label. The next model turn returned the expected
route marker. The gateway audit contained typed findings for the tool output but
contained none of the originals or AgentVeil tokens. The child shell observed
the AgentVeil session environment variable as empty.

This proves one dated live path, not every Codex workflow or future CLI version.

## Remaining release work at this anchor

1. Browser-verify the local dashboard and packaged synthetic demo flow.
2. Run the automated release/leak check on a clean final commit and retain its
   value-free machine-readable report; install `cargo-audit` first.
3. Run latency/throughput measurements without weakening fail-closed behavior.
4. Re-run the complete fake and live proof on the release commit.
5. Capture the sub-three-minute video and complete the Devpost entry.

The working milestone is July 17, 2026. The Devpost submission deadline is
July 21, 2026 at 5:00 PM Pacific / 8:00 PM Eastern.
