# AgentVeil demo video script

Target runtime: **2:50**. Hard limit: **under 3:00**. Record voiceover and publish the final video publicly on YouTube.

Use synthetic fixtures only. Record in a clean local account with notifications disabled. Never show an auth header, environment dump, shell history containing secrets, token mapping, request/response body, or personal filesystem path.

## Timed shot list and voiceover

### 0:00–0:12 — Cold open: the problem

**Screen:** Tight shot of Codex about to work with a small synthetic project fixture. Overlay: “Useful context can contain sensitive data.”

**Voiceover:** “Coding agents need context, but project context can also contain credentials, personal data, and private infrastructure. A warning banner is not a security boundary.”

### 0:12–0:28 — Product reveal

**Screen:** AgentVeil dashboard hero. Show the local route: Codex → AgentVeil → GPT-5.6.

**Voiceover:** “AgentVeil is a local privacy harness for Codex. It inspects supported Responses traffic before remote egress, blocks high-risk material, and replaces approved lower-risk values with short-lived session tokens.”

### 0:28–0:49 — Explain the boundary

**Screen:** Animate or point through three labels: authenticated loopback gateway, schema-aware inspection, fixed OpenAI upstream. Show “No local CA. No Codex fork.”

**Voiceover:** “A secure launcher creates an invocation-scoped Codex provider with random local authorization. The gateway is loopback-only, validates complete JSON shapes, and forwards only through a fixed route and header allowlist. Unknown model-visible shapes fail closed.”

### 0:49–1:15 — Tokenization demo

**Screen:** Run the deterministic synthetic demo. Show a synthetic email and private IP entering locally, then show only opaque placeholders at the fake upstream inspector. Do not reveal a reusable mapping.

**Voiceover:** “Here, synthetic lower-risk values are classified and replaced locally. The fake upstream sees placeholders rather than the originals. Tokens are random, session-scoped, time-bounded, and never written to the audit log.”

### 1:15–1:38 — Hard-block and zero-connect proof

**Screen:** Submit a clearly labeled synthetic credential fixture. Show the blocked result and the fake-upstream request counter remaining at zero.

**Voiceover:** “Credentials and private keys are different: they are never tokenized or restorable. This synthetic credential is blocked before an upstream request begins, and the wire harness proves that with a zero request count.”

### 1:38–2:02 — Tool output and GPT-5.6

**Screen:** Show the sanitized transcript of the verified Codex tool-flow result, ending with the safe marker from GPT-5.6 Luna. Keep the original fixture off-screen or visibly synthetic and redacted.

**Voiceover:** “Protection continues after tools run. In the live validation, Codex read synthetic local values, AgentVeil protected the next custom-tool output, and GPT-5.6 Luna completed the following turn through the guarded Responses route.”

### 2:02–2:23 — Safe observability

**Screen:** Dashboard activity and verification panels. Highlight class, action, source field, audit health, and zero original-value display.

**Voiceover:** “The dashboard and audit expose decisions, not data. They show what class was handled, where it appeared, and whether the request started—without bodies, authorization, protected values, mappings, or token previews.”

### 2:23–2:42 — Built with Codex

**Screen:** Fast montage of the main Codex task: integration decision, adversarial test names, and a clean test summary. Avoid showing unrelated tasks or private context.

**Voiceover:** “We built AgentVeil in Codex itself. Codex helped inspect its real provider seam, implement the Rust gateway, coordinate adversarial reviews, and turn bypass findings—object keys, encoded text, overlaps, and tool results—into regression tests.”

### 2:42–2:50 — Honest close

**Screen:** Repository URL and final hero. Small label: “Local Codex CLI prototype · synthetic fixtures · live restoration disabled.”

**Voiceover:** “AgentVeil is a focused prototype, not perfect DLP. Today it protects the verified local Codex CLI route, uses synthetic fixtures, and keeps live restoration disabled. The code and evidence are public.”

## Recording acceptance checks

- Final export is less than 3:00 and includes intelligible human voiceover.
- Product behavior, meaningful Codex use, and meaningful GPT-5.6 use are all visible—not only asserted in slides.
- The credential block shot visibly pairs the block with the fake-upstream zero count.
- The tokenization shot never implies that live OpenAI restoration is enabled.
- Every sensitive-looking value is a documented synthetic fixture.
- Repository and any shown report match the final release commit.
- Upload is public on YouTube, plays while signed out, and has usable audio at normal volume.
