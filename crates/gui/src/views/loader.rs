use eframe::egui::{self, RichText, TextEdit};

use crate::{app::ForgeApp, state::LoaderMode, theme};

pub fn render(ctx: &egui::Context, app: &mut ForgeApp) {
    let palette = theme::palette(app.theme_preference());

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.add_space(18.0);

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("MFA-Forge")
                    .size(34.0)
                    .strong()
                    .color(palette.brand_accent),
            );
            ui.label(
                RichText::new("Developer-first authenticator con vault local cifrado")
                    .size(15.0)
                    .color(palette.secondary_text),
            );
        });

        ui.add_space(10.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(
                    "Storage local | Argon2id + AES-256-GCM | Sin admin | GUI + CLI + API/MCP futuro",
                )
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
                        "Primera ejecución. Crea la contraseña maestra que protegerá el vault."
                    }
                    LoaderMode::Unlock => {
                        "Desbloquea el vault para acceder a las cuentas y generar tokens."
                    }
                })
                .color(palette.secondary_text),
            );

            ui.add_space(12.0);
            ui.label(RichText::new("Ruta del vault").small());
            ui.add(
                TextEdit::singleline(&mut app.vault_path().to_owned())
                    .interactive(false)
                    .font(egui::TextStyle::Monospace),
            );

            ui.label(RichText::new("Contraseña maestra").small());
            ui.add(
                TextEdit::singleline(&mut app.state_mut().loader.password_input)
                    .password(true)
                    .hint_text("Ingresa una contraseña fuerte"),
            );

            if app.state().loader.current_mode() == LoaderMode::Initialize {
                ui.label(RichText::new("Confirmar contraseña").small());
                ui.add(
                    TextEdit::singleline(&mut app.state_mut().loader.confirm_password_input)
                        .password(true)
                        .hint_text("Repite la contraseña"),
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
                        "Validando contraseña maestra y preparando el unlock..."
                    } else {
                        "Esperando validación de Windows (PIN / Hello)..."
                    };
                    ui.label(RichText::new(status).small().color(palette.warning_text));
                });
            }

            ui.add_space(10.0);
            match app.state().loader.current_mode() {
                LoaderMode::Initialize => {
                    if ui
                        .add_sized([150.0, 28.0], egui::Button::new("Crear vault"))
                        .clicked()
                    {
                        app.initialize_vault();
                    }
                }
                LoaderMode::Unlock => {
                    if ui
                        .add_enabled_ui(!app.is_unlock_pending(), |ui| {
                            ui.add_sized([150.0, 28.0], egui::Button::new("Desbloquear"))
                        })
                        .inner
                        .clicked()
                    {
                        app.unlock_vault();
                    }

                    if app.is_unlock_preparing() {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("La validación de la contraseña corre en background para no congelar la ventana.")
                                .small()
                                .color(palette.secondary_text),
                        );
                    } else if app.is_unlock_verifying() {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("El PIN / Windows Hello debería mostrarse fuera de la app.")
                                .small()
                                .color(palette.secondary_text),
                        );
                    }
                }
            }

            ui.label(
                RichText::new("El MVP actual no requiere privilegios elevados. Solo usa archivos locales bajo tu perfil de usuario.")
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
                RichText::new("Lo que estás viendo")
                    .size(18.0)
                    .strong()
                    .color(palette.brand_accent),
            );

            bullet(
                ui,
                palette,
                "Loader / unlock",
                "Pantalla inicial para crear o desbloquear el vault sin exponer secretos.",
            );
            bullet(
                ui,
                palette,
                "Ventana principal",
                "Dashboard con lista de cuentas, acciones rápidas, búsqueda y panel de detalles.",
            );
            bullet(
                ui,
                palette,
                "Diálogos",
                "Alta, import otpauth://, edición, rotación de contraseña, token, export y borrado.",
            );
            bullet(
                ui,
                palette,
                "Roadmap visible",
                "La GUI ya deja sitio claro para API local y MCP sin requerirlos hoy.",
            );

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(
                RichText::new("Seguridad y privilegios")
                    .strong()
                    .color(palette.warning_text),
            );
            ui.label("• El vault vive cifrado en disco.");
            ui.label("• La UI nunca muestra secretos raw.");
            ui.label(format!("• {}", app.admin_requirement_label()));
            ui.label(
                "• Futuras integraciones del sistema podrían requerir permisos adicionales, pero no el MVP actual.",
            );
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
