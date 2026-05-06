# Contributing

Thanks for contributing to MFA-Forge.

## Before you start

- Use [Issues](https://github.com/wnunezc/MFA-Forge/issues) for bugs and concrete feature requests
- For security-sensitive reports, follow [SECURITY.md](SECURITY.md) instead of opening a public issue

## Development expectations

MFA-Forge is a Windows-first Rust desktop application focused on secure local MFA management.

Please keep changes aligned with the current architecture:

- domain logic lives under `crates/core/`
- shared application orchestration lives under `crates/application/`
- encrypted persistence lives under `crates/storage/`
- human CLI and launcher live under `crates/cli/`
- desktop UI and local automation entrypoints live under `crates/gui/`
- Windows-specific integration stays isolated under `crates/platform-windows/`

## Coding guidelines

- Do not use `unwrap()` or `expect()` in production paths
- Keep modules focused on a single responsibility
- Avoid duplicating crypto, vault, or validation logic across surfaces
- Prefer typed errors and explicit handling
- Keep Windows-specific behaviors intentional and clearly isolated

## Validation before opening a pull request

Run the checks that apply to your change:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

If MSI packaging or release workflows are affected, also follow `docs/release-validation.md`.

If a validation step cannot run, explain why in the pull request description.

## Pull request guidance

- Keep PRs focused and easy to review
- Explain the user-visible change and the technical approach
- Mention risks, follow-up work, or known limitations
- Include screenshots when the change affects the UI
- Update docs when behavior or workflows change

## Branching

Contributors should branch from the latest `main` and open pull requests back to `main`.
