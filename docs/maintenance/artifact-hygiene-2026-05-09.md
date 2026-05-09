---
title: Artifact hygiene ledger
date: 2026-05-09
scope: MFA-Forge
---

# MFA-Forge — Artifact Hygiene Ledger (2026-05-09)

## Objetivo

Compactar artefactos locales sin perder la release estable retenida ni la evidencia minima canonica del edge funcional promovido a `1.0.0`.

## Checkpoint pre-limpieza

### Politica de retencion

Se conservan:

- `target/rc/MFA-Forge-1.0.0-x64.msi`
- `target/rc/published-1.0.0/MFA-Forge-1.0.0-x64.msi.sha256.txt`
- ejecutables raiz vigentes en `target/release/`
- `target/smoke-rc26/`
- set final reducido de `target/smoke-language/`
- set final reducido de `target/smoke-grants/`

Se eliminan despues del checkpoint:

- `target/rc/update-proof/`
- `target/rc/published-rc19/`
- `target/rc/published-rc20/`
- `target/rc/published-rc26/`
- `target/rc/extract*`
- `target/rc/repro-local*.msi`
- RCs antiguas no vigentes
- `target/debug`
- `target/codex-session-*`
- `target/wix`
- `target/winget*`
- subarboles reconstruibles de `target/release/`
- duplicados supersedidos en `smoke-language` y `smoke-grants`

### Medicion pre-limpieza

- `target/` antes de la purga: `15186.58 MB` (`14.83 GB`)

### Nota operativa

- Git no versiona `target/`; los commits sirven como checkpoints documentales de la politica de retencion, no como restauracion literal de artefactos ignorados.

## Checkpoint post-limpieza

### Resultado

- `target/` despues de la purga: `40.40 MB`
- artefactos retenidos fisicamente:
  - `target/rc/MFA-Forge-1.0.0-x64.msi`
  - `target/rc/published-1.0.0/MFA-Forge-1.0.0-x64.msi.sha256.txt`
  - `target/release/mfa-forge.exe`
  - `target/release/mfa-forge-gui.exe`
  - `target/release/mfa-forge-agent.exe`
  - `target/release/mfa-forge-mcp.exe`
  - `target/release/mfa-forge-launcher.exe`
  - `target/smoke-rc26/*`
  - set reducido de `target/smoke-language/`
  - set reducido de `target/smoke-grants/`

### Delta

- reduccion aproximada: `15146.18 MB`
