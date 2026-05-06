# MFA-Forge Release Policy

## Purpose

This folder holds the release policy and RC draft notes for MFA-Forge.

## Current line

- installed baseline: `RC18`
- current candidate line: `RC19`
- numeric candidate version: `0.1.19`
- no tag or GitHub release is implied by the presence of these docs alone

## Versioning and tagging

Canonical policy for the current pre-`1.0.0` arc:

- Cargo, workspace, and MSI version stay numeric as `0.1.N`
- the RC number matches the patch number in this line
- RC tags use `v0.1.N-rc.N`
- stable tags, when they exist, should drop the `-rc.N` suffix

Examples:

- installed baseline: `RC18` -> `0.1.18`
- current candidate line: `RC19` -> `0.1.19`
- candidate tag: `v0.1.19-rc.19`

## Asset naming

- MSI asset: `MFA-Forge-RCN-x64.msi`
- checksum file: `MFA-Forge-RCN-x64.msi.sha256.txt`
- draft notes: `docs/release/0.1.N-rc.N-draft.md`

## Publication gate

Before publishing an RC:

1. pass the validation gates in `docs/release-validation.md`
2. confirm the intended tag, checksum, and release assets
3. update the matching RC draft with the final evidence
4. publish as a GitHub prerelease for the `0.1.x` line
