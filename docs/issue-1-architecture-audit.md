# Issue #1 architecture audit

Audit date: 2026-06-19

The issue mixes applicable Rust desktop guidance with requirements from Docker-oriented products. This matrix adapts each rule to MFA-Forge instead of treating unrelated infrastructure as product scope.

| # | Status | MFA-Forge assessment |
|---|---|---|
| 1 | Partial | The workspace separates core, application, storage, platform and adapters. Large GUI/agent modules remain follow-up debt. |
| 2 | Improved in 1.0.2 | Agent and MCP now share one `stdio` lifecycle runtime instead of duplicating blocking loops. |
| 3 | Compliant | The hotfix uses explicit channels, polling and bounded waits without macro-heavy abstractions. |
| 4 | Partial | Vault persistence is isolated; audit/preferences and GUI export file access still need narrower ports. |
| 5 | Compliant | Existing crate boundaries provide the applicable layered desktop architecture. |
| 6 | Partial | UI, storage and platform boundaries exist; several modules exceed 1,000 lines. |
| 7 | Compliant | Shared vault/session workflows live in application services; platform verification is injected through traits. |
| 8 | Improved in 1.0.2 | `stdin` and vault preparation do not block the UI-owner thread; Win32 messages continue to be pumped. |
| 9 | Compliant | Crate/module/bin naming is consistent. |
| 10 | Compliant | Domain and storage boundaries validate account, TOTP and vault inputs. |
| 11 | Compliant | Process calls use `Command` arguments; no shell interpolation is used for sensitive inputs. |
| 12 | Not applicable | MFA-Forge has no SQL. JSON vault/config data uses structured serialization. |
| 13 | Compliant | Rust, MSI, workflows, locale assets and documentation are separated. |
| 14 | Compliant | The code uses Rust composition, traits, enums and `Result` rather than class-oriented patterns. |
| 15 | Guarded in 1.0.2 | CI prevents growth of the existing undocumented-public-API baseline. Full remediation remains incremental. |
| 16 | Compliant | Secrets remain encrypted and are excluded from logs, audit records and public protocol fields. |
| 17 | Guarded in 1.0.2 | CI verifies every `tr`/`trf` key exists in the canonical English catalog. |
| 18 | Compliant | Long vault work is polled from explicit pending state rather than executed during rendering. |
| 19 | Compliant | MSI and release workflow validate the distributed binary surface. Docker resource requirements do not apply. |
| 20 | Improved in 1.0.2 | Native helpers and Windows async operations now have bounded supervision and actionable errors. Docker/mkcert do not apply. |
| 21 | Guarded in 1.0.2 | CI rejects `unwrap`/`expect` in production Rust paths; lifecycle errors preserve context. |
| 22 | Improved in 1.0.2 | Lifecycle diagnostics record only sanitized state, identifiers and exit causes. |
| 23 | Improved in 1.0.2 | Runtime, timeout, identity and broker-health regressions have focused tests. |
| 24 | Compliant | Protocol additions are additive and vault/MSI compatibility is preserved. |
| 25 | Applied in 1.0.2 | Version, MSI, notes, checksum and winget manifests are prepared and validated together. |

## Follow-up issues

- [#3](https://github.com/wnunezc/MFA-Forge/issues/3): isolate audit, preferences and GUI file operations behind application/storage ports.
- [#4](https://github.com/wnunezc/MFA-Forge/issues/4): split oversized GUI, MCP and storage modules by existing responsibilities.
- [#5](https://github.com/wnunezc/MFA-Forge/issues/5): reduce cryptographic test contention and execution time without weakening coverage.

The originally planned guardrail follow-up is completed by `scripts/check-quality-guardrails.ps1` and the CI workflow.
