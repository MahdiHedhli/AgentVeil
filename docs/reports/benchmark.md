# Benchmark evidence

`scripts/benchmark` measures two existing synthetic release-test boundaries:

- `library_protect_restore` exercises classification, rewriting, session-scoped tokenization, and exact restoration in the privacy engine.
- `gateway_wire_proof` exercises the authenticated loopback gateway against its in-process fake upstream, including protected forwarding, hard blocking before connection, tool-result protection, restoration, and value-free audit assertions.

The harness forces Cargo offline, removes credential-like application variables by constructing a narrow child environment, suppresses all child output, and writes only aggregate timings. Its report contains no samples, command lines, request or response bodies, matched values, tokens, mappings, authorization material, hostnames, or filesystem paths. Generated evidence is private mode `0600` under ignored `reports/generated/`. The writer rejects symlinked or non-directory report components and an existing `reports/generated/` that is not already mode `0700`; it never repairs a pre-existing directory's permissions.

## Interpreting results

The metric is wall-clock time for a complete, already-built release-test process iteration. It includes Cargo and test-runner startup, fixture setup, loopback socket setup where applicable, assertions, and teardown. It is deliberately labeled `whole_test_process_not_request_latency`; do not cite it as proxy per-request latency or compare it to the product aspiration of less than 25 ms overhead.

Percentiles use the nearest-rank method. Throughput is measured test iterations per second, not model requests per second. Default runs use 25 measured iterations after three unmeasured warmups. A small run is suitable for harness smoke verification, while submission evidence should use the defaults on an otherwise idle machine and retain the generated JSON alongside the full release evidence.

Every generated report records the current Git `HEAD` SHA and observes worktree state before and after measurement. `release_eligible` is true only when both benchmark scenarios pass at the default 25 measured iterations and three warmups, `HEAD` remains unchanged, and both worktree observations are clean. A smaller, single-scenario, or dirty-tree pass remains useful development feedback but is not release evidence. `benchmark.template.json` is marked `status: template`, has `release_eligible: false`, and cannot be mistaken for a measured report.

```sh
scripts/benchmark
scripts/benchmark --iterations 3 --warmup 1 --no-report
```

The script fails closed if the exact expected test is absent, a command times out or fails, captured inventory output is malformed, aggregate relationships are inconsistent, the toolchain or repository identity is malformed, `HEAD` changes during the run, a report directory or destination is unsafe, or the output path is outside the direct `reports/generated/benchmark*.json` namespace. Child test output remains suppressed on failure so an assertion cannot echo a synthetic fixture into the transcript.
