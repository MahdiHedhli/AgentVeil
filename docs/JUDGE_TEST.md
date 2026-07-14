# Judge test path

The tagged GitHub release provides prebuilt, SHA-256-checksummed archives. The
offline proof needs no Codex installation, login, API key, source build, or
network access after download.

## Supported prebuilt proof platforms

- Ubuntu 24.04, x86_64
- macOS 14 or later, Apple silicon (arm64)

The separately scoped live Codex route was verified on macOS 26.4.1 arm64 with
Codex CLI exactly `0.144.4`. The offline proof is the recommended judge path.

## Linux x86_64

Candidate `v0.1.6` is the intended submission release. Run these commands only
after that tag and its checksums are publicly available:

```sh
tag=v0.1.6
asset="agentveil-${tag}-linux-x86_64.tar.gz"
base="https://github.com/MahdiHedhli/AgentVeil/releases/download/${tag}"
curl --fail --location --remote-name "${base}/${asset}"
curl --fail --location --remote-name "${base}/${asset}.sha256"
sha256sum --check "${asset}.sha256"
tar -xzf "$asset"
"./agentveil-${tag}-linux-x86_64/agentveil" demo --check
```

## macOS arm64

```sh
tag=v0.1.6
asset="agentveil-${tag}-macos-arm64.tar.gz"
base="https://github.com/MahdiHedhli/AgentVeil/releases/download/${tag}"
curl --fail --location --remote-name "${base}/${asset}"
curl --fail --location --remote-name "${base}/${asset}.sha256"
shasum -a 256 --check "${asset}.sha256"
tar -xzf "$asset"
"./agentveil-${tag}-macos-arm64/agentveil" demo --check
```

Exact success output:

```text
demo-check: status=pass route=synthetic_loopback allow=pass tokenize=pass zero_connect=pass tool_reentry=pass dashboard=pass audit=pass output=value_free
```

An optional second value-free check exercises the Codex-compatible request and
stream contract with a synthetic client. It does not launch Codex:

```sh
"./agentveil-${tag}-linux-x86_64/agentveil" codex-demo --check
# On macOS, use ./agentveil-${tag}-macos-arm64/agentveil instead.
```

```text
codex-demo-check: status=pass route=synthetic_client_codex_compatible block=pass tokenize=pass restore_delta=pass replay=pass wire_proof=pass audit=pass output=value_free
```

Together, the two checks start only ephemeral loopback listeners, use embedded
synthetic fixtures and capturing fake upstreams, and verify hard-secret
zero-connect, tokenization, tool-output re-entry, the Codex-compatible stream
and replay contract, dashboard state, and value-free audit records. They remove
their temporary audit directories before returning. For the interactive local
dashboard, replace `demo --check` with `demo` and open the printed loopback URL.

The release archives are not Apple-notarized or code-signed production
packages. Verify the published SHA-256 file before execution. Do not disable OS
security controls to run AgentVeil; use the Linux artifact if local policy
blocks an unsigned pre-release binary.
