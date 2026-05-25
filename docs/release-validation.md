# Release Validation Strategy

This document defines the release validation flow for MFA-Forge.

## Validation layers

| Layer | Scope | Command or evidence | Runs in CI |
|---|---|---|---|
| Static gates | Formatting, lints, and compile-time safety for the workspace | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` | Yes |
| Packaging gate | Release binaries, MSI creation, and checksum generation for the current release line | `cargo build --workspace --release`, `cargo wix --package mfa-forge-gui --no-build ...`, `Get-FileHash` | Yes |
| Installed release smoke | Real Windows install sanity over GUI, `mfa-forge-agent`, `mfa-forge-mcp`, grant prompts, language/font rendering, and the password-rotation prompt | Manual checklist below | No |
| Update path gate | Reproducible update proof from the installed previous release line to the current candidate through the approved trigger for that edge | Manual checklist below | No |

## Version, tag, and asset policy

Canonical policy for the current pre-`1.0.0` line:

- workspace, package, and MSI version stays numeric as `0.1.N`
- the RC number matches the patch number: `RCN -> 0.1.N`
- RC Git tags use `v0.1.N-rc.N`
- RC MSI assets use `MFA-Forge-RCN-x64.msi`
- RC draft notes use `docs/release/0.1.N-rc.N-draft.md`
- RC tags publish as GitHub prereleases when publication is explicitly approved

Stable policy after promotion to `1.0.0`:

- workspace, package, and MSI version stay numeric as the stable semantic version
- stable Git tags use `v1.0.0`, `v1.0.1`, etc.
- stable MSI assets use `MFA-Forge-<version>-x64.msi`
- stable notes use `docs/release/<version>-release.md`
- stable tags publish as GitHub releases, not prereleases

## Local non-publishing gate

Run before cutting a real release MSI:

```powershell
pwsh -Command "cargo fmt --all -- --check"
pwsh -Command "cargo clippy --workspace --all-targets -- -D warnings"
pwsh -Command "cargo test --workspace"
pwsh -Command "cargo build --workspace --release"
pwsh -Command "cargo wix --package mfa-forge-gui --no-build --target-bin-dir target/release --output target/rc/MFA-Forge-<version>-x64.msi"
pwsh -Command "(Get-FileHash -LiteralPath 'target/rc/MFA-Forge-<version>-x64.msi' -Algorithm SHA256).Hash"
```

## Installed release smoke checklist

For any locally installed candidate:

- MSI version matches the intended numeric line in `Cargo.toml`
- MSI filename matches the intended asset name
- if release/update support is claimed, the installed MSI must include every binary required for that path, especially `mfa-forge-launcher.exe`
- GUI unlock path still works on Windows
- GUI opens on a clean Windows host with an OpenGL 2.0-compatible driver stack
- token-grant prompt opens, approves once, and closes without freezing the UI
- provisioning-grant prompt approves and denies cleanly
- `mfa-forge-agent` opens, unlocks, and closes cleanly
- `mfa-forge-mcp` opens, unlocks, and closes cleanly
- language switch and help rendering stay readable in `English`, `Español`, `Français`, `हिन्दी`, and `中文`
- password-rotation prompt clearly communicates the action and deny path

## Update path checklist for the current candidate

This checklist must pass before any publication decision:

- local baseline is the installed previous release MSI
- the installed baseline actually contains the launcher if the update path depends on it
- if the installed baseline does not contain the launcher, the candidate release must document and validate a manual MSI upgrade path for that edge instead of marketing it as launcher-driven
- the product exposes a real and user-visible way to trigger the approved future update path; opening the GUI does not count unless startup update logic is explicitly implemented and verified
- launcher can discover the intended current-candidate GitHub prerelease metadata and asset names
- launcher downloads the current-candidate MSI and validates its SHA256 against the published checksum
- launcher hands control to the MSI update flow in a documented way; if a temporary helper copy is required to free in-use binaries, that helper path must be explicit in the evidence
- MSI upgrade replaces the installed previous-release binaries with the current candidate while preserving the per-user install boundary
- post-update smoke validates GUI, `mfa-forge-agent`, `mfa-forge-mcp`, the grant prompts, language/font rendering, and the password-rotation prompt
- evidence records exact tag, asset names, checksum, commands run, manual findings, and any skipped checks

## Critical blocker discovered on 2026-05-07

MFA-Forge reached a release state where launcher-driven updates were being treated as part of the RC story, but the installed RC baseline did not include `mfa-forge-launcher.exe` and the GUI did not implement startup auto-update logic.

This is a critical release-process failure. It should have been detected and resolved when the launcher was introduced, not after RC publication work had already advanced.

From this point forward:

- no release may be described as self-updating unless that behavior exists in the installed product and is validated on the installed baseline
- no launcher-driven update claim is acceptable unless the launcher binary is included in the MSI and the trigger path is exercised on a real installed RC
- missing launcher packaging or missing update trigger path blocks release publication just like a broken MSI or a failing smoke

## Current closed proof

The first literal installed proof for automatic startup update is:

- baseline installed locally: `RC25` / `0.1.25`
- published target: `RC26` / `0.1.26`
- trigger: open `mfa-forge-gui.exe` before unlock
- observed path: GUI startup -> launcher -> helper copy -> published checksum verification -> `msiexec /passive`
- result: installed `gui`, `agent`, `mcp`, and `launcher` all became `0.1.26`

## Evidence to record

Record these items in the release notes:

- app version and release tag
- exact MSI path and checksum
- tag name and release URL when publication is performed
- validation commands run
- dependency evidence for the shipped GUI binary (`dumpbin /DEPENDENTS`) so `VCRUNTIME140.dll` and `OPENGL32.dll` assumptions stay explicit
- manual smoke results
- update-path findings for the exact installed edge that matters
- anything intentionally skipped and why
