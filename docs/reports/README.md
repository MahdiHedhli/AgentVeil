# Value-free reports

Generated AgentVeil reports contain aggregate test metadata only. They must not contain request or response bodies, matched values, token literals, mappings, authorization material, cookies, query strings, original sensitive values, or absolute filesystem paths.

`leak-scan.schema.json` defines report version 2 from `scripts/verify-no-leak`. A report records the exact Git HEAD, clean-worktree state, scanner identifier, UTC generation time, enabled coverage, current tracked-file count, unique reachable-history blob count, evidence-file count, and aggregate errors/findings. It never records a matched value or match fingerprint. Console failures use a repository-relative label only when that label itself is safe; sensitive or unusual labels are replaced with `redacted-path`, and history findings never print object IDs or paths.

Generated reports belong under the private `reports/generated/` directory, which is ignored by Git. A normal `--report` run is fail-closed release mode: `release_eligible` can be `true` only when the scan passes on a clean worktree with nonzero current-tracked, unique-history-blob, and generated-evidence coverage. `--rehearsal` permits development runs but always records `release_eligible: false`. The checked-in template is deliberately `status: template`, never `pass`.

The scanner rejects tracked ignored files, generated/runtime paths, audit files,
sensitive filenames, and partial private-key headers. It also scans each unique
reachable Git blob once. A known synthetic historical literal may be suppressed
only when its rule, match digest, whole-file/blob digest, source kind, and narrow
path all match `tests/fixtures/leak-manifest.json`. This prevents a constant PEM
header exception from hiding different trailing content. The allowlist must
never contain a fingerprint derived from real data.

`scripts/release-check` requires a clean worktree and an installed `cargo-audit` by default. It runs the deterministic offline demo, stores its exact value-free result as private generated evidence, then creates and validates the v2 leak report. Either `--allow-*` switch changes the final status to `rehearsal` and `release_eligible=false`. Command output is suppressed on failure so an assertion cannot echo a canary into the transcript.

The release inventory also requires the benchmark harness, benchmark schema, non-pass template, and benchmark interpretation document. It validates the harness command inventory but does not run the default multi-iteration benchmark; final benchmark evidence is produced deliberately with `scripts/benchmark` under the procedure in `benchmark.md`.

Treat a release-eligible leak report as supporting evidence, not as a global no-leak guarantee. The release gate separately requires the Rust tests, strict linting, dependency advisory check, deterministic capturing-upstream proof, and final live synthetic route proof.
