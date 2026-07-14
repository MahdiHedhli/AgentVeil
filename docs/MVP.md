# AgentVeil Friday MVP

Working deadline: **Friday, July 17, 2026**. Hackathon submission deadline: **Tuesday, July 21, 2026 at 5:00 PM Pacific / 8:00 PM Eastern**.

## Product contract

AgentVeil protects one verified path:

```text
Codex CLI 0.144.4
  -> authenticated loopback AgentVeil provider
  -> inspected OpenAI Responses request
  -> allowlisted OpenAI upstream
  -> Responses SSE
  -> supported local display restoration
  -> Codex CLI
```

The model can use a reference. It must not receive a supported raw secret.

## Required and release-gated

- Codex CLI through HTTP Responses/SSE with GPT-5.6.
- A schema-aware adapter for observed message, instruction, prior-turn, and tool/function-result text.
- Whole-request blocking for supported high-confidence credentials and private keys.
- Session-scoped tokenization for supported lower-risk identifiers.
- A memory-only bounded ledger with OS-random tokens, TTL, exact lookup, and explicit clear.
- Value-free audit metadata.
- Loopback-only gateway authenticated by a per-session local credential.
- Deterministic fake-upstream proof for block, token round-trip, audit privacy, encoding bypass, and tool-result replay.
- One live Codex synthetic-data proof.
- Honest payload map, threat model, privacy statement, limitations, and demo instructions.

## Handling classes

| Class | Meaning | Default | Ledger | Restoration |
|---|---|---|---|---|
| S0 | Credentials, private keys, authentication material | block request | never | never |
| S1 | Context-essential lower-risk identifiers | tokenize | memory-only, scoped and bounded | exact issued token in approved local display text only |
| S2 | Sensitive data that need not remain exact | mask | never | never |

Overlap precedence is `S0 block > S2 mask > S1 tokenize > allow`. Mappings are allocated only after overlap resolution.

Default policy uses S0 for credentials and private keys, S1 for private IP/internal host identifiers, and S2 for email, phone, home path, and custom terms. The isolated synthetic demo policy may promote its known-fake email to S1 to demonstrate round-trip restoration.

## Deliberate pushback

Restoring a value into text consumed by Codex may cause the local Codex transcript to persist and replay the original. Until transcript behavior is verified, real-Codex restoration is not a positive claim. The deterministic harness may restore a known synthetic value to an AgentVeil-local display sink; live Codex may remain tokenized. Outbound rescanning protects future remote egress but does not erase a local transcript.

## Explicit Friday non-goals

- Codex IDE or Codex cloud execution without independent route evidence.
- Responses WebSocket transport.
- Browser ChatGPT or other AI clients/providers.
- Images, OCR, binary blobs, compressed request bodies, arbitrary nested encodings, or every PII class.
- Persistent or remotely synchronized ledgers.
- Optional Presidio, OPF, local-model, or cloud-DLP dependencies.
- Generic transparent TLS interception, a desktop app, accounts, teams, billing, or a cloud control plane.
