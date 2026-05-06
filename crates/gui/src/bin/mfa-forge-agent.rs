fn main() {
    mfa_forge_gui::diagnostics::install_panic_hook("mfa-forge-agent");

    if let Err(error) = mfa_forge_gui::diagnostics::guard_result(
        "mfa-forge-agent",
        "run_stdio_session",
        mfa_forge_gui::agent::run_stdio_session,
    ) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
