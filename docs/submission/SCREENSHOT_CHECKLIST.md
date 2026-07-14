# Submission screenshot checklist

Capture only final-build surfaces with synthetic fixtures. Prefer four strong screenshots over a large sequence of repetitive terminal images.

## Required set

- [ ] **Hero dashboard:** full desktop view with the AgentVeil name, “Privacy boundary” headline, Codex → AgentVeil → GPT-5.6 route, protected-session state, audit health, and Responses/SSE status.
- [ ] **Safe activity detail:** selected event showing only data class, action, safe source field, and upstream outcome. Confirm there is no original value, token preview, auth material, mapping, request body, or personal path anywhere in the frame.
- [ ] **Hard block / zero-connect:** synthetic credential blocked alongside a deterministic fake-upstream request count of `0`. The frame must make clear that this is a synthetic wire-level test.
- [ ] **Protected tool-result flow:** concise view of the Codex → local tool → AgentVeil → GPT-5.6 Luna workflow and its safe completion marker. Do not expose raw fixtures or environment data.

## Optional supporting set

- [ ] **Architecture:** simple three-boundary diagram covering secure launcher, schema-aware gateway, and fixed upstream; include “loopback only” and “live restoration disabled.”
- [ ] **Verification:** clean final test/release summary with the exact commit SHA. Avoid using a stale hard-coded test count.
- [ ] **Built with Codex:** cropped view from the main development task showing a substantive architecture, security-review, or test-feedback exchange without unrelated private context.

## Visual quality

- [ ] Use one consistent desktop size and browser zoom; avoid tiny terminal text.
- [ ] Crop out menu-bar notifications, account avatars, bookmarks, unrelated tabs, and local usernames.
- [ ] Keep the main claim readable at thumbnail size.
- [ ] Use the dashboard's native navy, warm-white, and signal-green palette; do not add decorative security clichés.
- [ ] Show focus and selected states intentionally; avoid hover tooltips covering evidence.
- [ ] Capture PNG at native resolution. Do not upscale a blurry image.
- [ ] Add concise alt text and captions when Devpost permits it.

## Security review before upload

- [ ] Search every visible terminal line and browser panel for bearer values, cookies, API-key prefixes, email addresses, IP addresses, home-directory paths, session tokens, or token mappings.
- [ ] Confirm all sensitive-looking values are the repository's documented synthetic fixtures.
- [ ] Confirm no real Codex config, auth file, environment dump, shell history, donor runtime state, or audit body is visible.
- [ ] Confirm the dashboard is connected only to loopback and exposes safe metadata.
- [ ] Have a second reviewer inspect original-resolution images before upload.
