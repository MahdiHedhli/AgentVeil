# Value-free reports

Generated AgentVeil reports contain aggregate test metadata only. They must not contain request or response bodies, matched values, token literals, mappings, authorization material, cookies, query strings, original sensitive values, or absolute filesystem paths.

`leak-scan.schema.json` defines the output accepted from `scripts/verify-no-leak`. The scanner reports counts by stable rule identifier. Console failures identify a repository-relative file and line, but never print the matched text or a fingerprint of it.

Generated reports belong under `reports/generated/`, which is ignored by Git. Treat a passing report as supporting evidence only; the release gate also requires the full Rust tests and wire-level zero-connect assertions.

`scripts/release-check` requires a clean worktree and an installed `cargo-audit` by default. Its two `--allow-*` switches exist only for local development rehearsal; a run using either override is not release evidence. Command output is redirected away from the terminal, including on failure, so an assertion cannot echo a canary into the transcript.
