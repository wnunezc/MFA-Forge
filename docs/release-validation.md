# Release Validation Strategy

This document defines the release validation flow for MFA-Forge.

## Validation layers

| Layer | Scope | Command or evidence | Runs in CI |
|---|---|---|---|
| Static gates | Formatting, lints, and compile-time safety for the workspace | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` | Yes |
| Packaging gate | Release binaries, MSI creation, and checksum generation for the current RC line | `cargo build --workspace --release`, `cargo wix --package mfa-forge-gui --no-build ...`, `Get-FileHash` | Yes |
| Installed RC smoke | Real Windows install sanity over GUI, `mfa-forge-agent`, `mfa-forge-mcp`, and the password-rotation prompt | Manual checklist below | No |
| Update path gate | Reproducible launcher-driven update proof from installed `RC17` to `RC18` | Manual checklist below | No |

## Version, tag, and asset policy

Canonical policy for the current pre-`1.0.0` line:

- workspace, package, and MSI version stays numeric as `0.1.N`
- the RC number matches the patch number: `RC17 -> 0.1.17`, `RC18 -> 0.1.18`
- RC Git tags use `v0.1.N-rc.N`
- RC MSI assets use `MFA-Forge-RCN-x64.msi`
- RC draft notes use `docs/release/0.1.N-rc.N-draft.md`
- RC tags publish as GitHub prereleases when publication is explicitly approved

## Local non-publishing gate

Run before cutting a real RC MSI:

```powershell
pwsh -Command "cargo fmt --all -- --check"
pwsh -Command "cargo clippy --workspace --all-targets -- -D warnings"
pwsh -Command "cargo test --workspace"
pwsh -Command "cargo build --workspace --release"
pwsh -Command "cargo wix --package mfa-forge-gui --no-build --target-bin-dir target/release --output target/rc/MFA-Forge-RC18-x64.msi"
pwsh -Command "(Get-FileHash -LiteralPath 'target/rc/MFA-Forge-RC18-x64.msi' -Algorithm SHA256).Hash"
```

## Installed RC smoke checklist

For any locally installed RC candidate:

- MSI version matches the intended numeric line in `Cargo.toml`
- MSI filename matches the intended RC asset name
- GUI unlock path still works on Windows
- `mfa-forge-agent` opens, unlocks, and closes cleanly
- `mfa-forge-mcp` opens, unlocks, and closes cleanly
- password-rotation prompt clearly communicates the action and deny path

## Update path checklist for RC18

This checklist must pass before any RC18 publication decision:

- local baseline is the installed `RC17` MSI
- launcher can discover the intended RC18 GitHub prerelease metadata and asset names
- launcher downloads the RC18 MSI and validates its SHA256 against the published checksum
- launcher hands control to the MSI update flow without stealth or background behavior
- MSI upgrade replaces the installed RC17 binaries with RC18 while preserving the per-user install boundary
- post-update smoke validates GUI, `mfa-forge-agent`, `mfa-forge-mcp`, and the password-rotation prompt
- evidence records exact tag, asset names, checksum, commands run, manual findings, and any skipped checks

## Evidence to record

Record these items in the RC draft:

- RC number and numeric app version
- exact MSI path and checksum
- tag name and prerelease URL when publication is performed
- validation commands run
- manual smoke results
- update-path findings for `RC17 -> RC18`
- anything intentionally skipped and why
