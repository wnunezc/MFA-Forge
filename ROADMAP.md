# Roadmap

This roadmap tracks product direction at a high level. `README.md` remains the best source for the currently implemented surface.

## Phase 0 - Foundation

- [x] Rust workspace and repository setup
- [x] Git repository initialization
- [x] baseline project structure

## Phase 1 - Core MFA vault

- [x] `core`, `storage`, `cli`, `gui`
- [x] encrypted local vault
- [x] TOTP add, list, token, remove, and metadata export
- [x] Windows MSI packaging

## Phase 2 - Hardening and shared flows

- [x] atomic writes and backup/restore strategy
- [x] password rotation with vault re-encryption
- [x] `otpauth://` import via shared domain logic
- [x] QR parsing for `otpauth://`
- [x] richer validation and migration support
- [x] richer search, internal metadata, and project directories
- [ ] optional OS keychain or keyfile strategy

## Phase 3 - Desktop GUI

- [x] Rust-native desktop shell
- [x] vault initialize and unlock workflow
- [x] account add, edit, remove, token, and export flows
- [x] persistent light and dark theme
- [x] live countdown and explicit copy action
- [x] visual project directories and inline account actions
- [x] account history and restore UX
- [x] deletion of empty workspace and subdirectory nodes
- [x] multi-selection and bulk account deletion
- [ ] wider release validation of Windows secondary verification

## Phase 4 - Local automation

- [x] native unlock window for local automation access
- [x] process-scoped session over JSON `stdio`
- [x] machine-readable account and token operations
- [x] explicit short-lived grants for sensitive operations
- [x] password rotation inside the local automation boundary
- [ ] loopback-only HTTP API
- [ ] broader client-scoped authorization model

## Phase 5 - Interop and integrations

- [x] CSV import
- [x] selective import from external authenticators and managers
- [x] controlled export formats beyond metadata-only JSON
- [ ] browser integration
- [ ] SSH agent integration
- [ ] Secret Service equivalent
- [ ] challenge-response hardware support

## Phase 6 - Advanced MFA

- [ ] HOTP
- [ ] WebAuthn and passkeys
- [ ] hardware-backed options

## Phase 7 - Release hardening

- [x] dedicated `mfa-forge-mcp` binary
- [x] dedicated `mfa-forge-agent` binary
- [x] dedicated `mfa-forge-launcher` binary
- [x] minimal MCP server over JSON-RPC `stdio`
- [x] local audit trail for sensitive automation actions
- [x] validated local `RC17 -> RC18` upgrade path
- [ ] broader client-scoped deny-by-default policy model
- [ ] deeper audit and reporting workflows
- [ ] stable public release publication
