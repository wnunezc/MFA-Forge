pub mod agent;
pub mod app;
mod app_actions;
mod app_status;
mod app_tasks;
mod app_unlock;
pub mod diagnostics;
mod dialogs;
mod help;
pub mod i18n;
pub mod platform_auth;
mod qr_import;
mod runtime;
mod state;
pub mod theme;
pub mod vault;
mod views;

pub fn run_main_app() -> eframe::Result<()> {
    runtime::ensure_supported_runtime("La GUI desktop de MFA-Forge")
        .map_err(|error| eframe::Error::AppCreation(Box::new(std::io::Error::other(error))))?;

    let app_icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/app-icon.png"))
            .map_err(|error| eframe::Error::AppCreation(Box::new(error)))?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MFA-Forge")
            .with_inner_size([1380.0, 860.0])
            .with_min_inner_size([1120.0, 720.0])
            .with_icon(app_icon),
        ..Default::default()
    };

    eframe::run_native(
        "MFA-Forge",
        options,
        Box::new(|cc| {
            app::ForgeApp::new(cc)
                .map(|app| Box::new(app) as Box<dyn eframe::App>)
                .map_err(|error| error.into())
        }),
    )
}
