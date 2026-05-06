# MFA-Forge Feature Map

Reference feature inventory inspired by the public documentation of `KeePassXC`, adapted to the actual scope of `MFA-Forge`.

Source references:

- repository README: <https://github.com/keepassxreboot/keepassxc>
- official wiki: <https://github.com/keepassxreboot/keepassxc/wiki>

## Adaptation principle

`MFA-Forge` is not a clone of `KeePassXC`.

The reference is used for:

- security posture
- offline and local-first product ergonomics
- inventory of useful capabilities
- advanced integrations to evaluate incrementally

Everything implemented in `MFA-Forge` must preserve:

- strict separation between public metadata and sensitive data
- deny-by-default automation
- simple local trust boundaries

## Reference capability inventory

### Local secure base

- encrypted local vault
- offline storage
- sensitive data encrypted at rest
- cross-platform domain model

### Organization and entry management

- groups and folders
- search
- entry icons
- custom attributes
- attachments
- entry history and restore
- cross-field references

### MFA and authentication

- TOTP storage and generation
- challenge-response with hardware tokens
- passkeys through browser integration

### Productivity tools

- CLI
- password or passphrase generation
- auto-type
- auto-open of databases

### Integrations

- browser integration
- SSH agent integration
- Secret Service
- shared sync workflows

### Import, export, and auditing

- import from CSV and external managers
- export to structured formats
- health and reporting workflows

### Cryptography and formats

- modern encrypted container formats
- additional cipher choices where justified

## MFA-Forge implementation map

## Phase A - Secure foundation

Goal:

- consolidate the current MVP
- reinforce storage and security boundaries

Features:

- [x] encrypted local vault
- [x] master password
- [x] baseline CLI
- [x] TOTP add, list, token, remove, and metadata export
- [x] password rotation
- [x] atomic writes and corruption recovery
- [x] richer search and internal metadata
- [ ] optional keyfile support

## Phase B - Desktop experience

Goal:

- deliver a native desktop UI over the shared core

Features:

- [x] baseline desktop GUI
- [x] vault unlock
- [x] visual add, edit, and remove flows
- [x] token dialog with live countdown and explicit copy
- [x] persistent theme selection
- [x] visual directories and subdirectories
- [x] per-account iconography
- [x] account history and restore
- [x] internal account metadata for search, import, and export
- [x] deletion of empty workspace and subdirectory nodes
- [x] multi-selection and bulk account deletion
- [ ] broader release validation of Windows secondary verification

## Phase C - Interoperability

Goal:

- make adoption and migration easier without exposing secrets outside the vault

Features:

- [x] import from `otpauth://`
- [x] QR import for `otpauth://`
- [x] CSV import
- [x] selective import from external managers
- [x] controlled metadata export
- [x] CSV metadata export without secrets by default

## Phase D - Advanced MFA

Goal:

- expand beyond TOTP

Features:

- [ ] HOTP
- [ ] hardware challenge-response
- [ ] WebAuthn and passkeys
- [ ] per-account policy extensions

## Phase E - Local integrations

Goal:

- enable developer-focused local integrations without weakening the security model

Features:

- [x] process-scoped local session over `stdio`
- [x] temporary native unlock for local automation
- [x] metadata operations and TOTP generation over a machine-readable channel
- [x] bounded local audit trail for sensitive automation operations
- [x] public history review and recent audit-log review behind explicit approval
- [x] password rotation through agent and MCP with a dedicated native prompt
- [ ] local loopback API
- [ ] browser integration
- [ ] SSH agent integration
- [ ] Secret Service equivalent

## Phase F - MCP and local automation

Goal:

- enable secure automation for local tools and connected clients

Expected artifact:

- `mfa-forge-mcp` binary

Rules and features:

- [x] minimal MCP binary over `stdio`
- [x] locked startup with explicit `open_session`
- [x] initial deny-by-default path for token generation
- [x] no raw secret reads
- [x] token generation only through explicit temporary approval
- [x] unlock scope limited to the local process lifetime
- [x] local traceability for grants and token delivery without secrets or TOTP in logs
- [x] explicit approval for history and recent audit review
- [x] broader traceability for audit review and password rotation
- [x] reuse of `core` and `storage`
- [ ] broader client or tool policy model
- [ ] no bypass of local authentication if additional surfaces are added later

Candidate MCP operations:

- [x] `health`
- [x] `open_session`
- [x] `session_info`
- [x] `list_accounts_metadata`
- [x] `get_account_metadata`
- [x] `generate_totp_token`
- [x] `grant_audit_reporting`
- [x] `list_history`
- [x] `read_audit_events`
- [x] `summarize_audit_events`
- [x] `create_account`
- [x] `import_otpauth`
- [x] `update_account`
- [x] `remove_account`
- [x] `export_metadata`
- [x] `rotate_master_password`

## Features that should not be copied blindly

- a general-purpose password-manager scope that dilutes the MFA focus
- broad compatibility work without a defined risk model
- alternate ciphers added only for checkbox parity
- new automation surfaces without a clear trust boundary
