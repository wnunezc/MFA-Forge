use eframe::egui::{self, RichText};

use crate::{
    app::ForgeApp,
    i18n::{tr, trf},
    theme,
};

pub(super) fn update_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().update_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().update_dialog.open;
    let current_version = app.current_release_version();
    let stage_directory = match app.update_stage_directory() {
        Ok(path) => path.display().to_string(),
        Err(error) => {
            let unavailable = trf("Unavailable: {error}", &[("error", &error)]);
            app.state_mut().update_dialog.error = Some(error);
            unavailable
        }
    };

    egui::Window::new(tr("RC update"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .show(ctx, |ui| {
            ui.label(tr(
                "MFA-Forge checks GitHub for newer prerelease RCs when you open the GUI. This action forces the installed launcher to check again, verify the published checksum, and then hand control to Windows Installer if an update exists.",
            ));
            ui.separator();

            ui.label(RichText::new(tr("Current version")).strong());
            ui.label(current_version);

            ui.add_space(6.0);
            ui.label(RichText::new(tr("Stage directory")).strong());
            ui.label(stage_directory);

            ui.add_space(8.0);
            ui.label(
                RichText::new(tr(
                    "The launcher stays responsible for version detection, checksum verification, and the MSI handoff. It still has no vault access.",
                ))
                .small()
                .color(palette.secondary_text),
            );

            if let Some(error) = &app.state().update_dialog.error {
                ui.add_space(8.0);
                ui.colored_label(palette.error_text, error);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Cancel"))).clicked() {
                    app.state_mut().update_dialog.close();
                }
                if ui
                    .button(format!("⬇ {}", tr("Start launcher")))
                    .clicked()
                {
                    app.start_latest_rc_update(false);
                }
            });
        });

    if !open {
        app.state_mut().update_dialog.close();
    }
}
