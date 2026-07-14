# AgentVeil final submission checklist

## Deadlines

- **Internal working deadline:** Friday, July 17, 2026. Freeze scope to verified release gates; use the remaining time for evidence, video, and submission QA.
- **$100 Codex credit request deadline:** Friday, July 17, 2026 at 12:00 PM Pacific, for registered entrants while supplies last and subject to approval; approved credits expire July 31.
- **Final Devpost deadline:** Tuesday, July 21, 2026 at 5:00 PM Pacific / 8:00 PM Eastern.
- **Judging availability:** Official Rules state July 22 at 10:00 AM Pacific through August 5 at 5:00 PM Pacific; the schedule page lists August 9. Keep every artifact available through August 9.
- **Winner announcement:** on or around August 12, 2026 at 2:00 PM Pacific.
- [ ] Put calendar holds ahead of both organizer deadlines; do not plan the final upload for the last hour.

## Eligibility and submission record

- [ ] Re-read the live Devpost rules immediately before submission and confirm participant, region, build-period, content, and track eligibility.
- [ ] Select exactly one track: **Developer Tools**, unless the team explicitly changes this decision before final submission.
- [ ] Complete all participant/team fields from verified information; do not infer or invent team facts.
- [ ] Keep a timestamped copy or screenshot of the submitted entry and confirmation page.

## Public artifacts

- [ ] Repository is public: [github.com/MahdiHedhli/AgentVeil](https://github.com/MahdiHedhli/AgentVeil).
- [ ] Final commit is pushed; working tree is clean; release SHA and clean-tree state are recorded in the private generated evidence report.
- [ ] README accurately states what is verified, how to run the safe deterministic demo, architecture, threat model, limitations, security-reporting path, license, and donor attribution.
- [ ] Fresh clone on a clean machine/account can build, test, and run the documented demo without private local state.
- [ ] Prebuilt SHA-256-verified release artifacts work without rebuilding on the stated Ubuntu 24.04 x86_64 and macOS 14+ arm64 judge platforms.
- [ ] The release page links `docs/JUDGE_TEST.md`, and its offline command works without Codex login, an API key, or an OpenAI request.
- [ ] Public YouTube video is under 3 minutes, uses the approved The Neighbor
  voice, and visibly demonstrates the actual product, meaningful Codex use, and
  meaningful GPT-5.6 use.
- [ ] Killer-demo core is one clean continuous shot with AgentVeil's dashboard
  above the real Codex TUI: first prompt, interception, local display,
  same-session replay, and local display again. No unrelated project or task is
  visible.
- [ ] Repository, YouTube video, and any demo link open while signed out/incognito and do not request private access.
- [ ] Keep the repository, video, release artifacts, and instructions free and unrestricted through at least August 9, 2026, covering the longer organizer schedule.

## Evidence gates

- [ ] Run the complete Rust test suite, 4 Python regressions, strict Clippy,
  formatting check, both `demo --check` and `codex-demo --check`, benchmark,
  leak scanner, dependency audit, and release check on the final commit. The
  working candidate has 51 Rust tests; record the final result rather than
  blindly reusing that count.
- [ ] Re-run the deterministic fake-upstream proof with synthetic fixtures: outbound tokenization, hard-secret block with zero upstream requests, tool-output inspection, fragmented SSE, and value-free audit assertions.
- [ ] Re-run the real `agentveil codex` launcher proof on the pinned supported Codex CLI and `gpt-5.6-luna` using synthetic values only.
- [ ] Confirm the protected tool-result reaches the next model turn and the audit contains neither original fixtures nor issued token text.
- [ ] Confirm live OpenAI restoration remains disabled and the demo does not imply otherwise.
- [ ] Confirm full synthetic-harness restoration and Codex delta-only
  restoration are presented as distinct modes: Codex restores only
  `response.output_text.delta`; done/item/completion/response snapshots remain
  tokenized.
- [ ] Confirm gateway, dashboard, fake upstream, and verification harness bind only to loopback.
- [ ] Confirm dashboard assets/state reject a mismatched Host authority and live mode does not claim synthetic wire proof.
- [ ] Confirm dashboard state remains value-free, schema v2 carries the client
  discriminator and expanded enum labels, consumers gate on the version, and a
  synthetic boundary failure is sticky and visibly reported as failed.
- [ ] Run the interactive synthetic Codex demo through two turns, exit cleanly,
  and verify its entire private runtime is absent. State plainly that the TUI is
  resumable while running and forced kill/crash can leave synthetic residue.
- [ ] Review open risks and limitations; fix release-blocking issues or state unsupported behavior plainly.

## Devpost copy and media

- [ ] Paste and proof the final product description; replace every `<ADD ...>` placeholder.
- [ ] Keep claims bounded to the final evidence report: no “perfect,” “production-ready,” or global “zero leaks” language.
- [ ] Upload final screenshots only after the security review in `SCREENSHOT_CHECKLIST.md`.
- [ ] Confirm only the explicitly interactive synthetic Codex TUI shows the
  two documented lower-risk originals. Automated output, dashboard, audit, and
  reports must remain value-free.
- [ ] Verify project name, tagline, selected track, built-with list, repository URL, public video URL, and demo/run instructions.
- [ ] Check spelling, image order, captions, thumbnail, and video audio on desktop and mobile.

## Required Codex feedback

- [ ] Run `/feedback` in the **main Codex development task** before the submission deadline.
- [ ] Confirm Devpost and `DEVPOST.md` contain the main-task `/feedback` Session
  ID `019f611e-d944-7640-b73c-ca652925371c`; do not substitute another task's
  ID.
- [ ] Review the feedback, incorporate applicable changes, and record a concise disposition for anything intentionally not adopted.
- [ ] Ensure the final entry explains how Codex materially contributed to architecture, implementation, adversarial review, and verification—not merely that it generated code.

## Final security pass

- [ ] Use synthetic sensitive-data fixtures only across the repository, demo, video, screenshots, issues, and Devpost entry.
- [ ] Scan tracked files and generated submission assets for credentials, personal data, auth headers, cookies, token mappings, request/response bodies, sensitive paths, and copied donor runtime state.
- [ ] Verify audit/report/dashboard output contains classifications and decisions only.
- [ ] Verify S0 credentials and private keys remain blocked, never tokenized, and never restorable.
- [ ] Confirm Apache-2.0 license and donor attribution are preserved.
- [ ] Confirm all submission copy, captions, audio, and judge instructions are in English.
- [ ] Confirm video music, imagery, fonts, marks, and other media are original, licensed, or used with authorization.

## Submit and verify

- [ ] Submit before Tuesday, July 21, 2026 at 5:00 PM Pacific / 8:00 PM Eastern.
- [ ] Open the published Devpost page signed out and test every link and embedded video.
- [ ] Save the public submission URL and confirmation evidence.
- [ ] Announce only after the public entry, repository, and video have each passed signed-out verification.
