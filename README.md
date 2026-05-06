# MFA-Forge

MFA-Forge is a secure MFA token manager written in Rust. The current release line provides a human CLI, a Windows desktop GUI, a Windows-only local agent session over `stdio`, a Windows-only minimal MCP server over `stdio`, an encrypted local vault, bounded local audit/history reporting, native password rotation within the local automation boundary, and Windows MSI packaging.

## Release status

- current candidate line: `RC18`
- numeric version: `0.1.18`
- local MSI artifact: `target/rc/MFA-Forge-RC18-x64.msi`
- the exact upgrade path from installed `RC17` to `RC18` has been validated locally
- `RC18` is still a local candidate until a tag and GitHub prerelease are created

## Repository guide

- implemented surface: this `README.md`
- roadmap and pending work: `ROADMAP.md`
- feature inventory and product direction: `FEATURE_MAP.md`
- release validation policy: `docs/release-validation.md`
- RC policy and draft release notes: `docs/release/`
- architecture notes and guardrails: `docs/architecture-hardening.md`

## Current surface

Implemented now:

- encrypted local vault with `Argon2id + AES-256-GCM`
- atomic writes with temp-file promotion plus backup and restore support
- CLI for `init`, `agent`, `mcp`, `add`, `import`, `import-csv`, `import-bitwarden-csv`, `list`, `history`, `restore`, `token`, `remove`, `rotate-password`, and `export`
- Windows desktop GUI for unlock, account management, import flows, token display, history restore, export, and theme persistence
- dedicated `mfa-forge-agent` binary for process-scoped local automation
- dedicated `mfa-forge-mcp` binary for MCP clients over JSON-RPC `stdio`
- dedicated `mfa-forge-launcher` binary for release discovery, checksum verification, and MSI handoff
- explicit short-lived grants for token delivery, account provisioning, and audit reporting
- local JSONL audit trail without raw secrets, TOTP values, or `otpauth://` URIs
- recent audit-log review with bounded tail reads and local compaction
- `otpauth://` import in CLI and GUI
- local QR import for `otpauth://` in the GUI
- vault schema migration with automatic `v1/v2 -> v3` persistence on unlock
- persistent project directories, richer search, account history, and bulk delete in the GUI
- Windows MSI packaging with integrated app icon

Still pending:

- loopback API
- optional OS keychain or keyfile support
- broader client-scoped authorization policies
- deeper audit/reporting workflows
- browser integration
- SSH agent integration
- Secret Service equivalent
- remote sync
- HOTP
- WebAuthn and passkeys

## Architecture

```text
MFA-Forge/
├── crates/
│   ├── core/              # domain models, validation, otpauth parsing, TOTP generation
│   ├── application/       # shared vault/session orchestration, unlock flow, ports
│   ├── storage/           # encrypted vault, filesystem persistence, atomic writes, backup/restore
│   ├── platform-windows/  # Windows presence verification and owner-window handling
│   ├── cli/               # human CLI, launcher, and local bridge delegators
│   └── gui/               # egui/eframe shell plus local agent-session and MCP entrypoints
├── FEATURE_MAP.md
├── Cargo.toml
└── ROADMAP.md
```

### Boundary rules

- `core` owns MFA validation and sensitive domain transformations
- `application` owns the shared unlock/session flow and reusable vault orchestration
- `storage` owns vault persistence, re-encryption, and recovery mechanics
- `platform-windows` owns the Windows-specific presence verification boundary
- `cli`, `gui`, the local agent session, and the MCP server orchestrate use-cases without duplicating crypto or validation logic

## Security posture

- secrets are never persisted in plaintext
- metadata and secrets stay separated in the domain model
- default exports remain metadata-only
- password rotation re-encrypts the existing vault instead of rebuilding data manually
- `otpauth://` parsing is normalized through shared domain logic
- the local agent session keeps the vault unlocked only while its process remains alive
- the MCP layer starts locked and only opens the native unlock flow after an explicit `open_session`
- token delivery requires explicit per-account approval and is recorded without exposing raw secrets or token values
- history and audit review require explicit temporary reporting approval and only expose sanitized public data
- neither the local agent session nor the MCP layer expose raw secret export

Out of scope for this release line:

- a fully compromised host with live memory inspection
- hardware-backed secret isolation
- background unlock daemons shared across independent client processes

## CLI usage

```powershell
mfa-forge init
mfa-forge agent
mfa-forge mcp
mfa-forge add --service GitHub --user user@example.com
mfa-forge import --uri "otpauth://totp/GitHub:user@example.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub"
mfa-forge import-csv --path .\accounts.csv
mfa-forge import-bitwarden-csv --path .\bitwarden.csv --preview
mfa-forge list
mfa-forge history
mfa-forge restore --entry-id 00000000-0000-0000-0000-000000000000
mfa-forge token GitHub --user user@example.com
mfa-forge rotate-password
mfa-forge remove GitHub --user user@example.com
mfa-forge export --data-format json
```

Notes:

- if `--secret` is omitted, the CLI prompts securely
- if `--uri` is omitted, the CLI prompts securely for the `otpauth://` value
- `mfa-forge agent` and `mfa-forge mcp` are only supported on Windows in this line
- passing `--uri` on the command line can leak the secret into shell history
- passing CSV files with raw secrets requires the same local handling care as any sensitive seed material

## GUI and automation status

The GUI already provides unlock, project navigation, account management, QR import, account history restore, token display with live countdown, metadata export, password rotation, and persistent theming.

The local agent session already provides a Windows-only process-scoped session over JSON `stdio` with unlock, list, token generation, add, import, update, remove, metadata export, history inspection, password rotation, and explicit session closure.

The minimal MCP server currently provides Windows-only runtime support, locked startup with `open_session`, the current account and audit tools, explicit grant flows, and local audit entries without raw secrets.

## Verification

Minimum verification for this repo:

```powershell
pwsh -Command "cargo fmt --all -- --check"
pwsh -Command "cargo check --workspace"
pwsh -Command "cargo clippy --workspace -- -D warnings"
```

Release-oriented validation is documented in `docs/release-validation.md`.
