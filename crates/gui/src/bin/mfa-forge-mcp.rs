fn main() {
    mfa_forge_gui::diagnostics::install_panic_hook("mfa-forge-mcp");

    if let Err(error) = mfa_forge_gui::diagnostics::guard_result(
        "mfa-forge-mcp",
        "run_mcp_server",
        mfa_forge_gui::agent::run_mcp_server,
    ) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
