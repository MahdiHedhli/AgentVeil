# AgentVeil killer-demo video script

Target runtime: **2:35 (155 seconds)**. Hard limit: **under 3:00**. Use the
OmniVoice Studio **The Neighbor** profile: American, female, young adult,
moderate pitch. Publish the approved final export publicly on YouTube.

Use synthetic fixtures only. Record in a clean Codex window with notifications
disabled. Never show auth, environment dumps, request/response bodies, audit
bodies, issued tokens, mappings, temporary paths, unrelated repositories, or
unrelated Codex tasks.

## Timed shot list and voiceover

### 0:00-0:12 — The problem

**Screen:** Clean title over a minimal synthetic code fixture: “Useful context
can contain sensitive data.”

**Voiceover:** “Coding agents need context. But that context can also contain
credentials, personal data, and private infrastructure. A warning banner is not
a security boundary.”

### 0:12-0:27 — Launch the real app

**Screen:** In one clean Codex window, place AgentVeil's in-app dashboard above
the real Codex TUI. Launch `agentveil codex-demo`. Make “Codex CLI to synthetic
loopback proof” and “model route loopback-only” legible.

**Voiceover:** “AgentVeil is a local privacy harness for Codex. This is the real
pinned Codex command-line interface, routed through AgentVeil to a capturing
synthetic loopback fixture.”

### 0:27-0:46 — Show the boundary

**Screen:** Hold the combined window. Point through Codex CLI → authenticated
AgentVeil gateway → Loopback fixture. Show loopback, healthy audit, and the
value-free dashboard.

**Voiceover:** “The gateway authenticates its local caller, validates the full
Responses payload, and fails closed before egress. The dashboard shows typed
decisions only—never bodies, authorization, originals, tokens, or mappings.”

### 0:46-1:07 — Intercept and restore in one shot

**Screen:** Type and send the exact documented first synthetic prompt. While
Codex waits, show the dashboard add the email and private-IPv4 protection rows
and wire proof. Then show Codex display the exact synthetic assignments.

**Voiceover:** “Watch one request. AgentVeil detects the synthetic email and
private address, replaces both before the capturing boundary, and restores
only the live output-text delta for Codex's local display. The model route stays
loopback-only.”

### 1:07-1:25 — Replay without friction

**Screen:** Type `Repeat the synthetic configuration exactly.` Show `Protected
history replayed`, the prior values re-protected, and the same exact synthetic
assignments displayed again.

**Voiceover:** “On the next turn, exact owned tokens replay only in the same
unexpired session. Prior raw user history is independently re-protected, while
completion and history snapshots remain tokenized on the wire.”

### 1:25-1:45 — Credentials still hard-block

**Screen:** Show the packaged synthetic S0 case: blocked, upstream not started,
and zero additional upstream requests. Do not show a reusable credential-like
string.

**Voiceover:** “Credentials and private keys are different. They are never
tokenized, stored in the ledger, or restored. The synthetic hard-secret case is
blocked before the fixture receives a request, and the harness proves the count
stays unchanged.”

### 1:45-2:03 — Meaningful GPT-5.6 evidence

**Screen:** Brief, separately labeled shot of the dated live Codex route ending
in `WRAPPER_ROUTE_OK`, with `Synthetic wire proof: Not measured` and `Live
restoration disabled` visible. Do not show fixture output.

**Voiceover:** “Separately, a dated live validation sent protected synthetic
tool output through the guarded Responses route to GPT-5.6 Luna. Live
restoration stayed disabled; the capturing loopback test remains the wire
proof.”

### 2:03-2:21 — Evidence

**Screen:** Clean final-commit test summary: 51 Rust tests, 4 Python
regressions, strict lint, deterministic wire proof, and value-free evidence.

**Voiceover:** “The current candidate has fifty-one Rust tests and four Python
release regressions, covering fail-closed parsing, encoded bypasses, zero-
connect blocking, tool replay, delta-only restoration, and evidence hygiene.”

### 2:21-2:35 — Honest close

**Screen:** Repository URL and hero. Small label: “Codex CLI 0.144.4 · synthetic
fixtures · live restoration disabled.”

**Voiceover:** “AgentVeil is a focused prototype, not perfect D L P. Its scope
is explicit, its fixtures are synthetic, live restoration is disabled, and the
code and evidence are public.”

## Recording acceptance checks

- Final export is less than 3:00 and uses the approved The Neighbor voice at an
  intelligible level.
- The combined dashboard/TUI shot is a continuous authentic capture: first
  prompt, interception, restored local display, replay prompt, and restored
  display again.
- The first prompt uses only the repository's exact synthetic email and private
  IPv4 fixture. No real PII or credential appears.
- The dashboard, automated check output, audit, and reports remain value-free;
  only the explicitly interactive Codex TUI shows the documented synthetic
  lower-risk originals.
- The route is labeled synthetic loopback and “model route loopback-only.” The
  video
  never implies that live OpenAI restoration is enabled.
- The credential shot pairs a whole-request block with zero additional
  upstream requests and says S0 values are never tokenized/stored/restored.
- The live GPT-5.6 shot is separately labeled, shows wire proof `Not measured`,
  and does not expose fixture output.
- If synthetic wire proof becomes `Failed`, an unknown token is accepted, or
  cleanup does not verify the private root absent, discard the run.
- The TUI's private thread is resumable while running. Record a clean `/exit`;
  forced termination can leave synthetic temporary state.
- Repository and test summary match the final clean release commit.
- Upload is public on YouTube, plays while signed out, and has usable audio at
  normal volume.
- Music, imagery, fonts, product marks, and every other media asset are
  original, licensed, or used with authorization. Spoken audio, captions, and
  on-screen instructions are in English.
