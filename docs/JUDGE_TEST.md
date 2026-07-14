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

Replace `v0.1.2` only if the submission points to a newer release:

```sh
tag=v0.1.2
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
tag=v0.1.2
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

The command starts only ephemeral loopback listeners, uses embedded synthetic
fixtures and a capturing fake upstream, verifies hard-secret zero-connect,
tokenization, tool-output re-entry, dashboard state, and value-free audit
records, then removes its temporary audit directory. For the interactive local
dashboard, replace `demo --check` with `demo` and open the printed loopback URL.

The release archives are not Apple-notarized or code-signed production
packages. Verify the published SHA-256 file before execution. Do not disable OS
security controls to run AgentVeil; use the Linux artifact if local policy
blocks an unsigned pre-release binary.
