use eframe::egui::{self, RichText, TextEdit};

use crate::{app::ForgeApp, i18n::tr, state::LoaderMode, theme};

pub fn render(ctx: &egui::Context, app: &mut ForgeApp) {
    let palette = theme::palette(app.theme_preference());

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.add_space(18.0);

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(tr("MFA-Forge"))
                    .size(34.0)
                    .strong()
                    .color(palette.brand_accent),
            );
            ui.label(
                RichText::new(tr("Developer-first authenticator with an encrypted local vault"))
                    .size(15.0)
                    .color(palette.secondary_text),
            );
        });

        ui.add_space(10.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(tr(
                    "Local storage | Argon2id + AES-256-GCM | No admin | GUI + CLI + future MCP/API",
                ))
                .small()
                .monospace()
                .color(palette.muted_text),
            );
        });

        ui.add_space(18.0);

        ui.columns(2, |columns| {
            left_column(&mut columns[0], app);
            right_column(&mut columns[1], app);
        });
    });
}

fn left_column(ui: &mut egui::Ui, app: &mut ForgeApp) {
    let palette = theme::palette(app.theme_preference());

    section_frame()
        .fill(palette.surface_fill)
        .stroke(egui::Stroke::new(1.0, palette.surface_stroke))
        .show(ui, |ui| {
            ui.label(
                RichText::new(app.loader_mode_label())
                    .size(20.0)
                    .strong()
                    .color(palette.strong_text),
            );
            ui.label(
                RichText::new(match app.state().loader.current_mode() {
                    LoaderMode::Initialize => {
                        tr("First run. Create the master password that will protect the vault.")
                    }
                    LoaderMode::Unlock => tr("Unlock the vault to access accounts and generate tokens."),
                })
                .color(palette.secondary_text),
            );

            ui.add_space(12.0);
            ui.label(RichText::new(tr("Vault path")).small());
            ui.add(
                TextEdit::singleline(&mut app.vault_path().to_owned())
                    .interactive(false)
                    .font(egui::TextStyle::Monospace),
            );

            ui.label(RichText::new(tr("Master password")).small());
            ui.add(
                TextEdit::singleline(&mut app.state_mut().loader.password_input)
                    .password(true)
                    .hint_text(tr("Enter a strong password")),
            );

            if app.state().loader.current_mode() == LoaderMode::Initialize {
                ui.label(RichText::new(tr("Confirm password")).small());
                ui.add(
                    TextEdit::singleline(&mut app.state_mut().loader.confirm_password_input)
                        .password(true)
                        .hint_text(tr("Repeat the password")),
                );
            }

            if let Some(error) = &app.state().loader.error {
                ui.colored_label(palette.error_text, error);
            }

            if app.is_unlock_pending() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    let status = if app.is_unlock_preparing() {
                        tr("Validating the master password and preparing unlock...")
                    } else {
                        tr("Waiting for Windows verification (PIN / Hello)...")
                    };
                    ui.label(RichText::new(status).small().color(palette.warning_text));
                });
            }

            ui.add_space(10.0);
            match app.state().loader.current_mode() {
                LoaderMode::Initialize => {
                    if ui
                        .add_sized([150.0, 28.0], egui::Button::new(tr("Create vault")))
                        .clicked()
                    {
                        app.initialize_vault();
                    }
                }
                LoaderMode::Unlock => {
                    if ui
                        .add_enabled_ui(!app.is_unlock_pending(), |ui| {
                            ui.add_sized([150.0, 28.0], egui::Button::new(tr("Unlock")))
                        })
                        .inner
                        .clicked()
                    {
                        app.unlock_vault();
                    }

                    if app.is_unlock_preparing() {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(tr("Password validation runs in the background so the window stays responsive."))
                                .small()
                                .color(palette.secondary_text),
                        );
                    } else if app.is_unlock_verifying() {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(tr(
                                "The PIN / Windows Hello prompt should appear outside the app.",
                            ))
                                .small()
                                .color(palette.secondary_text),
                        );
                    }
                }
            }

            ui.label(
                RichText::new(tr("The current MVP does not require elevated privileges. It only uses local files under your user profile."))
                    .small()
                    .color(palette.warning_text),
            );
        });
}

fn right_column(ui: &mut egui::Ui, app: &mut ForgeApp) {
    let palette = theme::palette(app.theme_preference());

    section_frame()
        .fill(palette.surface_fill)
        .stroke(egui::Stroke::new(1.0, palette.surface_stroke))
        .show(ui, |ui| {
            ui.label(
                RichText::new(tr("What you are looking at"))
                    .size(18.0)
                    .strong()
                    .color(palette.brand_accent),
            );

            bullet(
                ui,
                palette,
                &tr("Loader / unlock"),
                &tr("Initial screen to create or unlock the vault without exposing secrets."),
            );
            bullet(
                ui,
                palette,
                &tr("Main window"),
                &tr("Dashboard with accounts, quick actions, search, and detail context."),
            );
            bullet(
                ui,
                palette,
                &tr("Dialogs"),
                &tr("Add, import, edit, password rotation, token, export, and delete flows."),
            );
            bullet(
                ui,
                palette,
                &tr("Visible roadmap"),
                &tr(
                    "The GUI already leaves clear room for future local API and MCP integration without requiring them today.",
                ),
            );

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(
                RichText::new(tr("Security and privileges"))
                    .strong()
                    .color(palette.warning_text),
            );
            ui.label(format!("• {}", tr("The vault remains encrypted on disk.")));
            ui.label(format!("• {}", tr("The UI never shows raw secrets.")));
            ui.label(format!("• {}", app.admin_requirement_label()));
            ui.label(format!(
                "• {}",
                tr("Future OS integrations may require additional permissions, but not the current MVP."),
            ));
            ui.add_space(10.0);
            if ui.button(tr("Help")).clicked() {
                app.open_help_dialog();
            }
        });
}

fn bullet(ui: &mut egui::Ui, palette: theme::ThemePalette, title: &str, body: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(title).strong().color(palette.strong_text));
    ui.label(RichText::new(body).color(palette.secondary_text));
}

fn section_frame() -> egui::Frame {
    egui::Frame::default()
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::same(16.0))
}
