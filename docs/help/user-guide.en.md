# MFA-Forge User Guide

## Overview
MFA-Forge is a local-first MFA manager for Windows. Its main purpose is to keep TOTP accounts in an encrypted vault while exposing a consistent workflow across the GUI, the human CLI, the local agent session, and the MCP server. The application is designed so that secrets stay local, sensitive actions remain explicit, and automation never silently bypasses the unlock and grant boundaries.

## Getting Started
On first launch, create a master password. That password is the main key for the vault: without it, you cannot add accounts, import seeds, export backups, rotate the password, or generate codes.

After you enter the master password, MFA-Forge still performs the additional Windows verification flow used by this release line. In practice, the application only becomes usable after both steps succeed.

Once unlocked, the main window shows three working areas:

- the workspace tree on the left
- the account list in the center
- the contextual actions and dialogs layered on top of that layout

The goal of this structure is to let you organize accounts first, then operate on the selected scope without switching to a different screen.

## Workspaces
Workspaces are the grouping system for accounts. Use them to separate tokens by project, client, environment, or team.

How they work:

- a root workspace is the top-level project bucket
- a subdirectory is a nested path under an existing workspace
- accounts can either live inside a workspace path or remain unassigned

Why they matter:

- the active workspace filters the account view
- new accounts inherit the currently selected workspace by default
- export, restore, and review flows stay easier to reason about when accounts are grouped consistently

If you keep personal or shared break-glass accounts, leaving them unassigned can be useful so they remain outside project-specific folders.

## Adding Accounts
MFA-Forge supports four main ways to load a TOTP account:

1. Manual entry
2. `otpauth://` URI import
3. QR image import
4. Compatible file import

Manual entry is the best option when you want to control the label, user, workspace, algorithm, digits, and period directly.

URI, QR, and compatible-file import are best when another system already gave you a seed in standard TOTP form. In those cases, MFA-Forge parses the source, extracts the account fields, and stores the secret encrypted in the vault.

Important behavior:

- secrets stay masked in the UI
- import dialogs clear sensitive text when they close
- changing account metadata does not require changing the secret
- editing a secret is optional; leaving the field empty preserves the current encrypted secret

## Tokens and History
The token window is the operational view for reading a code. When you open it from an account row, MFA-Forge reads the current TOTP value from the unlocked vault and shows the countdown for the active period.

What to expect when refreshing:

- if the same TOTP period is still active, a refresh can legitimately return the same code
- if the period rolled over, the visible code updates immediately
- copying a code only copies the current token value, not the secret

History serves a different purpose. It is there for recovery, not for token reading.

The restore dialog lets you:

- inspect restorable snapshots
- recover accounts that were removed
- restore a previous visible version back into the active vault

Use history when an account was deleted by mistake, when metadata was changed incorrectly, or when you need to recover a prior version without rebuilding the account manually.

## Backup and Restore
Export creates an encrypted MFA-Forge backup file. Its purpose is to preserve the full vault in a form that can later be re-imported by MFA-Forge.

Import is intentionally strong in effect: after validation, it replaces the active vault contents with the imported encrypted backup. This is useful for disaster recovery or machine migration, but it should be treated as a controlled restore operation, not as a merge.

Recommended practice:

- create a backup before large edits or bulk imports
- store backups in a protected location
- verify that you are importing the intended backup before applying it

## Local Agent and MCP
The local agent session and the MCP server exist to support local automation, but they do not run as permanently trusted channels.

Core behavior:

- both start from a deny-by-default posture
- opening a session requires the native unlock flow
- the unlocked session only lives while the process stays alive
- sensitive operations require explicit grants or dedicated prompts

Examples of protected actions:

- generating a token for an account
- provisioning or importing accounts
- reading sensitive history or audit data
- rotating the master password

This means automation is possible, but it remains bounded by deliberate user approval and local session lifetime.

## Troubleshooting
If unlock fails:

- confirm the master password first
- then complete the Windows verification prompt if it appears
- if the app still returns to the loader, try the flow again and watch for a native prompt outside the main window

If an import fails:

- confirm the source still contains a valid `otpauth://` payload
- verify the Base32 secret is still complete
- verify that the selected QR image actually contains the intended token seed

If a token looks unchanged:

- check the remaining seconds in the current TOTP period
- refresh again after the period rolls over

If automation is denied:

- check whether the session is still open
- check whether the required grant expired or was consumed
- reopen the local session and re-approve the exact action when needed
