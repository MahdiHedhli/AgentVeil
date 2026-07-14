# AgentVeil final submission checklist

## Deadlines

- **Internal working deadline:** Friday, July 17, 2026. Freeze scope to verified release gates; use the remaining time for evidence, video, and submission QA.
- **OpenAI API-credit request deadline:** Friday, July 17, 2026 at 12:00 PM Pacific, if credits are needed. Reconfirm this in the organizer materials before relying on it.
- **Final Devpost deadline:** Tuesday, July 21, 2026 at 5:00 PM Pacific / 8:00 PM Eastern.
- [ ] Put calendar holds ahead of both organizer deadlines; do not plan the final upload for the last hour.

## Eligibility and submission record

- [ ] Re-read the live Devpost rules immediately before submission and confirm participant, region, build-period, content, and track eligibility.
- [ ] Select exactly one track: **Developer Tools**, unless the team explicitly changes this decision before final submission.
- [ ] Complete all participant/team fields from verified information; do not infer or invent team facts.
- [ ] Keep a timestamped copy or screenshot of the submitted entry and confirmation page.

## Public artifacts

- [ ] Repository is public: [github.com/MahdiHedhli/AgentVeil](https://github.com/MahdiHedhli/AgentVeil).
- [ ] Final commit is pushed; working tree is clean; release SHA is recorded in the README and evidence report.
- [ ] README accurately states what is verified, how to run the safe deterministic demo, architecture, threat model, limitations, security-reporting path, license, and donor attribution.
- [ ] Fresh clone on a clean machine/account can build, test, and run the documented demo without private local state.
- [ ] Public YouTube video is under 3 minutes, includes voiceover, and visibly demonstrates the product, meaningful Codex use, and meaningful GPT-5.6 use.
- [ ] Repository, YouTube video, and any demo link open while signed out/incognito and do not request private access.

## Evidence gates

- [ ] Run the complete Rust test suite, strict Clippy, formatting check, leak scanner, and release check on the final commit; record exact results rather than reusing the `ed09354` count.
- [ ] Re-run the deterministic fake-upstream proof with synthetic fixtures: outbound tokenization, hard-secret block with zero upstream requests, tool-output inspection, fragmented SSE, and value-free audit assertions.
- [ ] Re-run the real `agentveil codex` launcher proof on the pinned supported Codex CLI and `gpt-5.6-luna` using synthetic values only.
- [ ] Confirm the protected tool-result reaches the next model turn and the audit contains neither original fixtures nor issued token text.
- [ ] Confirm live OpenAI restoration remains disabled and the demo does not imply otherwise.
- [ ] Confirm gateway, dashboard, fake upstream, and verification harness bind only to loopback.
- [ ] Review open risks and limitations; fix release-blocking issues or state unsupported behavior plainly.

## Devpost copy and media

- [ ] Paste and proof the final product description; replace every `<ADD ...>` placeholder.
- [ ] Keep claims bounded to the final evidence report: no “perfect,” “production-ready,” or global “zero leaks” language.
- [ ] Upload final screenshots only after the security review in `SCREENSHOT_CHECKLIST.md`.
- [ ] Verify project name, tagline, selected track, built-with list, repository URL, public video URL, and demo/run instructions.
- [ ] Check spelling, image order, captions, thumbnail, and video audio on desktop and mobile.

## Required Codex feedback

- [ ] Run `/feedback` in the **main Codex development task** before the submission deadline.
- [ ] Review the feedback, incorporate applicable changes, and record a concise disposition for anything intentionally not adopted.
- [ ] Ensure the final entry explains how Codex materially contributed to architecture, implementation, adversarial review, and verification—not merely that it generated code.

## Final security pass

- [ ] Use synthetic sensitive-data fixtures only across the repository, demo, video, screenshots, issues, and Devpost entry.
- [ ] Scan tracked files and generated submission assets for credentials, personal data, auth headers, cookies, token mappings, request/response bodies, sensitive paths, and copied donor runtime state.
- [ ] Verify audit/report/dashboard output contains classifications and decisions only.
- [ ] Verify S0 credentials and private keys remain blocked, never tokenized, and never restorable.
- [ ] Confirm Apache-2.0 license and donor attribution are preserved.

## Submit and verify

- [ ] Submit before Tuesday, July 21, 2026 at 5:00 PM Pacific / 8:00 PM Eastern.
- [ ] Open the published Devpost page signed out and test every link and embedded video.
- [ ] Save the public submission URL and confirmation evidence.
- [ ] Announce only after the public entry, repository, and video have each passed signed-out verification.
