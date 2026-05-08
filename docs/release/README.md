# MFA-Forge Release Policy

## Purpose

This folder holds the release policy and RC draft notes for MFA-Forge.

## Current line

- installed baseline: previous published release line
- current candidate line follows `Cargo.toml`
- numeric candidate version follows the workspace version in `Cargo.toml`
- no tag or GitHub release is implied by the presence of these docs alone

## Versioning and tagging

Canonical policy for the current pre-`1.0.0` arc:

- Cargo, workspace, and MSI version stay numeric as `0.1.N`
- the RC number matches the patch number in this line
- RC tags use `v0.1.N-rc.N`
- stable tags, when they exist, should drop the `-rc.N` suffix

Examples:

- installed baseline: `RC(N-1)` -> `0.1.(N-1)`
- current candidate line: `RCN` -> `0.1.N`
- candidate tag: `v0.1.N-rc.N`

Stable policy once `1.0.0` is cut:

- Cargo, workspace, and MSI version stay numeric as the stable semantic version
- stable tags drop the RC suffix and use `v1.0.0`, `v1.0.1`, etc.
- stable MSI assets use `MFA-Forge-<version>-x64.msi`
- stable release notes live at `docs/release/<version>-release.md`

## Asset naming

- MSI asset: `MFA-Forge-RCN-x64.msi`
- checksum file: `MFA-Forge-RCN-x64.msi.sha256.txt`
- draft notes: `docs/release/0.1.N-rc.N-draft.md`

Stable examples:

- MSI asset: `MFA-Forge-1.0.0-x64.msi`
- checksum file: `MFA-Forge-1.0.0-x64.msi.sha256.txt`
- release notes: `docs/release/1.0.0-release.md`

## Publication gate

Before publishing an RC:

1. pass the validation gates in `docs/release-validation.md`
2. confirm the intended tag, checksum, and release assets
3. update the matching RC draft with the final evidence
4. if the installed previous RC lacked the launcher or another required trigger, record the exact manual upgrade path for that edge instead of calling it launcher-driven
5. if startup update on GUI open is part of the RC story, validate that exact installed edge on a real installed baseline instead of inferring it from launcher presence alone
6. publish as a GitHub prerelease for the `0.1.x` line

Before publishing a stable release:

1. pass the same validation gates in `docs/release-validation.md`
2. confirm the intended stable tag, checksum, and release assets
3. update the matching stable release notes with final evidence
4. publish as a GitHub release, not a prerelease
