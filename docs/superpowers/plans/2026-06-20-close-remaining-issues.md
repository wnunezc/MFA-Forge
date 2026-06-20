# Close Remaining Issues Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close MFA-Forge issues #1, #2, #3, #4 and #5 while preserving the stable 1.0.2 lifecycle/window behavior already committed in `eca1e18`.

**Architecture:** Keep the hotfix stable. Move concrete persistence responsibilities out of GUI rendering/action modules into focused storage/application modules, split oversized files only by existing cohesive responsibilities, and reduce test runtime by measuring hotspots before changing test profiles. Do not change vault format, public MCP/agent protocol, MSI behavior, or secret handling.

**Tech Stack:** Rust stable, Cargo workspace, egui/eframe, Win32 platform wrappers, serde JSON storage, GitHub issues via `gh`.

---

## Files and responsibilities

- `crates/storage/src/app_data.rs`: central app-data path provider for non-vault local files such as GUI preferences and main window state.
- `crates/storage/src/preferences.rs`: typed GUI preference persistence with JSON serialization.
- `crates/storage/src/audit_log.rs`: sanitized JSONL audit-log append/read/summarize/compact operations currently embedded in GUI agent code.
- `crates/storage/src/repository/history.rs`: account history capture and restore helpers extracted from `repository.rs`.
- `crates/storage/src/repository/io.rs`: atomic vault file IO helpers extracted from `repository.rs`.
- `crates/storage/src/repository/directory_registry.rs`: directory registry normalization helpers extracted from `repository.rs`.
- `crates/gui/src/theme.rs`: theme application only; preference load/save delegates to storage.
- `crates/gui/src/platform_auth.rs`: main-window state path delegates to storage app-data provider.
- `crates/gui/src/agent/audit.rs`: compatibility facade over storage audit-log functions.
- `crates/gui/src/agent/mcp/router.rs`: route MCP tool names to session-host operations.
- `crates/gui/src/agent/mcp/protocol.rs`: MCP request/response protocol structures extracted from `mcp.rs`.
- `crates/gui/src/dialogs/*.rs`: split dialog renderers by existing dialog type without changing UI behavior.
- `crates/gui/src/app_actions/*.rs`: split action methods by existing responsibility: import/export, account lifecycle, theme/language, updates.
- `docs/release/1.0.2-release.md`: evidence matrix for issue closure, vendored `winit`, and test timing before/after.

---

### Task 1: Baseline and issue evidence

- [ ] **Step 1: Capture current issue list and branch status**

Run:

```powershell
gh issue list --repo wnunezc/MFA-Forge --state open --limit 20
git status --short
git log -1 --oneline
```

Expected: issues #1-#5 open, clean tree after `eca1e18`, branch `hotfix/1.0.2-lifecycle`.

- [ ] **Step 2: Measure test baseline for #5**

Run focused timing commands:

```powershell
Measure-Command { cargo test -p mfa-forge-storage } | Select-Object TotalSeconds
Measure-Command { cargo test -p mfa-forge-application } | Select-Object TotalSeconds
Measure-Command { cargo test -p mfa-forge-gui agent::mcp } | Select-Object TotalSeconds
```

Expected: numeric timings captured for release notes.

- [ ] **Step 3: Record baseline in release notes**

Modify `docs/release/1.0.2-release.md` with a section `Issue closure matrix` listing #1-#5, current status, and baseline timings.

---

### Task 2: Move GUI preferences and app-data paths behind storage ports (#3)

- [ ] **Step 1: Create storage app-data path module**

Create `crates/storage/src/app_data.rs` exposing:

```rust
use std::path::PathBuf;

use directories::ProjectDirs;

pub fn data_local_file(file_name: &str) -> Result<PathBuf, String> {
    ProjectDirs::from("dev", "OpsZone", "MFA-Forge")
        .map(|dirs| dirs.data_local_dir().join(file_name))
        .ok_or_else(|| "MFA-Forge local data directory is not available.".to_owned())
}
```

Export it from `crates/storage/src/lib.rs`.

- [ ] **Step 2: Create storage preferences module**

Create `crates/storage/src/preferences.rs` with a generic JSON read/write API for GUI preferences. It must create parent directories, ignore missing/corrupt preference files by returning defaults, and never log secrets.

- [ ] **Step 3: Refactor GUI theme preferences**

Modify `crates/gui/src/theme.rs` so file reads/writes call `mfa_forge_storage::preferences` and path construction uses `mfa_forge_storage::app_data::data_local_file("gui-preferences.json")`.

- [ ] **Step 4: Refactor main-window state path**

Modify `crates/gui/src/platform_auth.rs` so `main_window_state_path()` uses `mfa_forge_storage::app_data::data_local_file("main-window.json")` with a local fallback only if storage cannot provide the path.

- [ ] **Step 5: Verify**

Run:

```powershell
cargo test -p mfa-forge-storage
cargo clippy -p mfa-forge-storage -p mfa-forge-gui -- -D warnings
```

Expected: pass.

---

### Task 3: Move audit-log file access behind storage ports (#3)

- [ ] **Step 1: Extract audit log storage**

Create `crates/storage/src/audit_log.rs` and move pure file operations from `crates/gui/src/agent/audit.rs`: append JSONL, read recent events, summarize, compact, and default audit path calculation.

- [ ] **Step 2: Keep GUI audit facade narrow**

Modify `crates/gui/src/agent/audit.rs` to retain `AuditEntry` construction and redaction policy in GUI/agent, but delegate all filesystem operations to `mfa_forge_storage::audit_log`.

- [ ] **Step 3: Add storage audit tests**

Move or add tests in `crates/storage/src/audit_log.rs` for append/read/summarize/compact using `tempfile::TempDir`.

- [ ] **Step 4: Verify**

Run:

```powershell
cargo test -p mfa-forge-storage audit_log
cargo test -p mfa-forge-gui audit
cargo clippy -p mfa-forge-storage -p mfa-forge-gui -- -D warnings
```

Expected: pass.

---

### Task 4: Split oversized modules by existing responsibilities (#4)

- [ ] **Step 1: Split storage repository internals**

Extract only private helpers from `crates/storage/src/repository.rs` into child modules under `crates/storage/src/repository/`: `io.rs`, `history.rs`, and `directory_registry.rs`. Preserve public `VaultRepository` API unchanged.

- [ ] **Step 2: Split MCP protocol/router from host**

Extract MCP request/response parsing and tool dispatch tables from `crates/gui/src/agent/mcp.rs` into `crates/gui/src/agent/mcp/protocol.rs` and `router.rs`. Preserve JSON protocol and tool names unchanged.

- [ ] **Step 3: Split GUI app actions**

Convert `crates/gui/src/app_actions.rs` into `crates/gui/src/app_actions/mod.rs` plus focused modules for import/export, account lifecycle, settings, and updates. Preserve method names on `ForgeApp` so views do not change.

- [ ] **Step 4: Split dialog renderers**

Move dialog renderer functions from `crates/gui/src/dialogs.rs` into `crates/gui/src/dialogs/mod.rs` plus focused modules. Preserve `dialogs::render(ctx, app)` as the public entry point.

- [ ] **Step 5: Verify after each split**

After every split run:

```powershell
cargo check -p mfa-forge-gui -p mfa-forge-storage
cargo test -p mfa-forge-storage
```

Expected: pass before continuing to the next split.

---

### Task 5: Reduce test duration and contention (#5)

- [ ] **Step 1: Identify measured hotspots**

Run storage/application/gui test packages with `-- --nocapture` only if needed, and use `Measure-Command` around package-level and module-level tests. Record slowest modules in release notes.

- [ ] **Step 2: Separate expensive crypto vectors safely**

If repeated repository tests create full Argon2 vaults unnecessarily, add test helper constructors that reuse fast deterministic test parameters only for tests that do not assert production KDF strength. Keep at least one test asserting production-strength KDF parameters and encryption behavior.

- [ ] **Step 3: Reduce serial contention**

Remove avoidable shared global paths from tests by using `TempDir` and per-test repositories. Do not weaken tests that verify backup/restore/migration atomicity.

- [ ] **Step 4: Verify before/after timing**

Run:

```powershell
Measure-Command { cargo test -p mfa-forge-storage } | Select-Object TotalSeconds
Measure-Command { cargo test -p mfa-forge-application } | Select-Object TotalSeconds
Measure-Command { cargo test -p mfa-forge-gui agent::mcp } | Select-Object TotalSeconds
```

Expected: timings are documented; coverage still includes production-strength crypto path.

---

### Task 6: Final closure evidence for #1 and #2

- [ ] **Step 1: Update release notes matrix**

Mark #2 fixed by lifecycle runtime, timeout/cancel, soak evidence, window stability, and WER inspection. Mark #1 resolved by implemented items plus explicit derived issues now closed by this plan.

- [ ] **Step 2: Run final local gates**

Run:

```powershell
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo build -p mfa-forge-gui --release
```

Expected: all pass.

- [ ] **Step 3: Commit final issue cleanup**

Run:

```powershell
git add .
git commit -m "refactor: close architecture and test debt"
```

Expected: local commit only. No push.

- [ ] **Step 4: Prepare GitHub comments**

Prepare concise evidence comments for #1-#5 and PR #6. Do not close issues until final gates and user approval to update GitHub state.
