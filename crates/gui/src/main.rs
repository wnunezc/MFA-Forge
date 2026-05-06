#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    mfa_forge_gui::diagnostics::install_panic_hook("mfa-forge-gui");

    if let Err(error) =
        mfa_forge_gui::diagnostics::guard_result("mfa-forge-gui", "run_main_app", || {
            mfa_forge_gui::run_main_app().map_err(|error| error.to_string())
        })
    {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
