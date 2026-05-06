# Security Policy

## Supported versions

Security fixes are only guaranteed for the latest release candidate published from the `main`
branch and for subsequent versions under active maintenance.

At this moment, the actively maintained line is:

| Version line | Supported |
|---|---|
| `0.1.18-rc.18` and newer | Yes |
| Older release candidates | No |

## Reporting a vulnerability

Please **do not** open a public GitHub Issue for security-sensitive reports.

Preferred reporting channels:

1. GitHub Security Advisories private reporting, if available for this repository
2. Email: [icarosnet@gmail.com](mailto:icarosnet@gmail.com)

When reporting a vulnerability, include:

- a short summary of the issue
- affected version or commit
- reproduction steps or proof of concept
- impact assessment
- any suggested remediation, if available

## Response expectations

The maintainer will try to:

- confirm receipt of the report
- assess severity and reproducibility
- coordinate a fix before public disclosure when appropriate

## Scope notes

MFA-Forge is a Windows-first Rust desktop application for local MFA management. Reports are
especially useful when they involve:

- raw secret or TOTP exposure
- insecure vault handling or export paths
- unsafe local automation boundaries
- insecure update or package distribution flows
- filesystem actions that could escape intended paths
