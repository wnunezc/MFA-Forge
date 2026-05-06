use eframe::egui::{self, RichText, TextEdit};

use mfa_forge_core::TotpAlgorithm;

use crate::{
    app::ForgeApp,
    state::{AccountFormState, BannerTone, MetadataFormState},
    theme,
};

pub fn render(ctx: &egui::Context, app: &mut ForgeApp) {
    notice_dialog(ctx, app);
    add_account_dialog(ctx, app);
    edit_account_dialog(ctx, app);
    import_dialog(ctx, app);
    import_file_dialog(ctx, app);
    import_qr_dialog(ctx, app);
    create_directory_dialog(ctx, app);
    remove_directory_dialog(ctx, app);
    restore_dialog(ctx, app);
    change_password_dialog(ctx, app);
    token_dialog(ctx, app);
    export_dialog(ctx, app);
    account_uri_dialog(ctx, app);
    remove_dialog(ctx, app);
}

fn notice_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().notice_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().notice_dialog.open;
    let title = app.state().notice_dialog.title.clone();
    let message = app.state().notice_dialog.message.clone();

    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.label(RichText::new(message).color(palette.strong_text).size(16.0));

            ui.add_space(10.0);
            ui.label(
                RichText::new(
                    "Este aviso no expone secretos y solo confirma el estado de la sesión.",
                )
                .small()
                .color(palette.secondary_text),
            );

            ui.separator();
            if ui.button("✖ Cerrar").clicked() {
                app.state_mut().notice_dialog.close();
            }
        });

    if !open {
        app.state_mut().notice_dialog.close();
    }
}

fn add_account_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().add_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().add_dialog.open;

    egui::Window::new("Agregar cuenta MFA")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .show(ctx, |ui| {
            let directories = app.directories().to_vec();
            ui.label("Carga una cuenta TOTP sin revelar el secreto en la interfaz.");
            ui.separator();

            account_form_fields(
                ui,
                &directories,
                &mut app.state_mut().add_dialog.form,
                false,
            );

            ui.label(
                RichText::new("La contraseña maestra y el secreto nunca se imprimen por defecto.")
                    .small()
                    .color(palette.warning_text),
            );

            if let Some(error) = &app.state().add_dialog.error {
                ui.colored_label(palette.error_text, error);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cancelar").clicked() {
                    app.state_mut().add_dialog.clear();
                }
                if ui.button("💾 Guardar cuenta").clicked() {
                    app.submit_add_dialog();
                }
            });
        });

    if !open {
        app.state_mut().add_dialog.clear();
    }
}

fn edit_account_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().edit_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().edit_dialog.open;

    egui::Window::new("Editar cuenta MFA")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .show(ctx, |ui| {
            let directories = app.directories().to_vec();
            ui.label("Actualiza el workspace o los parámetros TOTP. Deja el secreto vacío para conservar el actual.");
            ui.separator();

            account_form_fields(ui, &directories, &mut app.state_mut().edit_dialog.form, true);

            ui.label(
                RichText::new("Si no cambias el secreto, MFA-Forge reutiliza el material ya cifrado del vault.")
                    .small()
                    .color(palette.secondary_text),
            );

            if let Some(error) = &app.state().edit_dialog.error {
                ui.colored_label(palette.error_text, error);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cancelar").clicked() {
                    app.state_mut().edit_dialog.clear();
                }
                if ui.button("💾 Guardar cambios").clicked() {
                    app.submit_edit_dialog();
                }
            });
        });

    if !open {
        app.state_mut().edit_dialog.clear();
    }
}

fn import_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().import_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().import_dialog.open;

    egui::Window::new("Importar otpauth://")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(580.0)
        .show(ctx, |ui| {
            let directories = app.directories().to_vec();
            ui.label("Pega un URI `otpauth://` válido. MFA-Forge extraerá servicio, usuario y configuración TOTP.");
            ui.separator();

            ui.label("URI de importación");
            ui.add(
                TextEdit::singleline(&mut app.state_mut().import_dialog.uri)
                    .password(true)
                    .hint_text("otpauth://totp/Servicio:usuario?..."),
            );

            directory_assignment_fields(
                ui,
                &directories,
                &mut app.state_mut().import_dialog.metadata,
            );

            ui.label(
                RichText::new("El URI contiene el secreto: se oculta en pantalla y se limpia al cerrar el diálogo.")
                    .small()
                    .color(palette.warning_text),
            );

            if let Some(error) = &app.state().import_dialog.error {
                ui.colored_label(palette.error_text, error);
            }

            if app.state().import_dialog.pending {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new("Importando la cuenta sin bloquear la interfaz.")
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cancelar").clicked() {
                    app.state_mut().import_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().import_dialog.pending,
                        egui::Button::new(if app.state().import_dialog.pending {
                            "📤 Importando..."
                        } else {
                            "📤 Importar"
                        }),
                    )
                    .clicked()
                {
                    app.submit_import_dialog();
                }
            });
        });

    if !open {
        app.state_mut().import_dialog.clear();
    }
}

fn import_qr_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().import_qr_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().import_qr_dialog.open;

    egui::Window::new("📤 Importar desde QR")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(580.0)
        .show(ctx, |ui| {
            let directories = app.directories().to_vec();
            ui.label("Indica la ruta de una imagen local con un QR `otpauth://`.");
            ui.separator();

            ui.label("Ruta de imagen");
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut app.state_mut().import_qr_dialog.image_path)
                        .hint_text("D:/ruta/qr.png")
                        .desired_width(360.0),
                );
                if ui.button("🔍 Seleccionar...").clicked() {
                    app.browse_import_qr_image();
                }
            });

            directory_assignment_fields(
                ui,
                &directories,
                &mut app.state_mut().import_qr_dialog.metadata,
            );

            ui.label(
                RichText::new("La imagen solo se usa para extraer el `otpauth://`; el secreto no se deja visible en la UI.")
                    .small()
                    .color(palette.warning_text),
            );

            if let Some(error) = &app.state().import_qr_dialog.error {
                ui.colored_label(palette.error_text, error);
            }

            if app.state().import_qr_dialog.pending {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new("📤 Importando la cuenta desde la imagen QR.")
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cancelar").clicked() {
                    app.state_mut().import_qr_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().import_qr_dialog.pending,
                        egui::Button::new(if app.state().import_qr_dialog.pending {
                            "📥 Importando..."
                        } else {
                            "📤 Importar QR"
                        }),
                    )
                    .clicked()
                {
                    app.submit_import_qr_dialog();
                }
            });
        });

    if !open {
        app.state_mut().import_qr_dialog.clear();
    }
}

fn import_file_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().import_file_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().import_file_dialog.open;

    egui::Window::new("Importar cuenta desde archivo")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(600.0)
        .show(ctx, |ui| {
            let directories = app.directories().to_vec();
            ui.label("Selecciona un archivo compatible que contenga un URI `otpauth://` exportado explícitamente.");
            ui.separator();

            ui.label("Archivo de cuenta");
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut app.state_mut().import_file_dialog.file_path)
                        .hint_text("D:/ruta/cuenta.otpauth")
                        .desired_width(380.0),
                );
                if ui.button("🔍 Seleccionar...").clicked() {
                    app.browse_import_file_dialog();
                }
            });

            directory_assignment_fields(
                ui,
                &directories,
                &mut app.state_mut().import_file_dialog.metadata,
            );

            ui.label(
                RichText::new("El archivo se valida antes de importarse. Si no contiene un `otpauth://` válido, se rechaza.")
                    .small()
                    .color(palette.secondary_text),
            );

            if let Some(error) = &app.state().import_file_dialog.error {
                ui.colored_label(palette.error_text, error);
            }

            if app.state().import_file_dialog.pending {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new("Importando la cuenta desde archivo.")
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cancelar").clicked() {
                    app.state_mut().import_file_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().import_file_dialog.pending,
                        egui::Button::new(if app.state().import_file_dialog.pending {
                            "📥 Importando..."
                        } else {
                            "📥 Importar archivo"
                        }),
                    )
                    .clicked()
                {
                    app.submit_import_file_dialog();
                }
            });
        });

    if !open {
        app.state_mut().import_file_dialog.clear();
    }
}

fn create_directory_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().create_directory_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().create_directory_dialog.open;
    let parent_path = app.state().create_directory_dialog.parent_path.clone();
    let entered_name = app
        .state()
        .create_directory_dialog
        .name
        .trim()
        .replace('\\', "/");
    let resulting_path = match parent_path.as_deref() {
        Some(parent) if !entered_name.is_empty() => format!("{parent}/{entered_name}"),
        Some(parent) => parent.to_owned(),
        None => entered_name.clone(),
    };
    let is_subdirectory = parent_path.is_some();
    let window_title = if is_subdirectory {
        "📁 Crear subdirectorio"
    } else {
        "🏢 Crear workspace raíz"
    };
    let action_label = if is_subdirectory {
        "📁 Crear subdirectorio"
    } else {
        "🏢 Crear workspace raíz"
    };

    egui::Window::new(window_title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .show(ctx, |ui| {
            if let Some(parent_path) = parent_path.as_deref() {
                ui.label(format!("Workspace seleccionado: {parent_path}"));
                ui.label(
                    RichText::new("El nombre que ingreses se agregará debajo de esta ruta.")
                        .small()
                        .color(palette.secondary_text),
                );
            } else {
                ui.label("Crea un workspace raíz para agrupar cuentas del mismo proyecto.");
            }

            ui.separator();
            ui.label("Nombre del proyecto / directorio");
            ui.add(
                TextEdit::singleline(&mut app.state_mut().create_directory_dialog.name)
                    .hint_text("cliente-a, auth, prod, mobile..."),
            );

            ui.add_space(8.0);
            ui.label("Ruta resultante");
            if resulting_path.trim().is_empty() {
                ui.label(
                    RichText::new("Se completará cuando ingreses el nombre.")
                        .small()
                        .color(palette.secondary_text),
                );
            } else {
                ui.label(RichText::new(resulting_path).monospace());
            }

            if let Some(error) = &app.state().create_directory_dialog.error {
                ui.add_space(6.0);
                ui.colored_label(palette.error_text, error);
            }

            if app.state().create_directory_dialog.pending {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new("Creando el workspace y sincronizando el vault.")
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cancelar").clicked() {
                    app.state_mut().create_directory_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().create_directory_dialog.pending,
                        egui::Button::new(if app.state().create_directory_dialog.pending {
                            "Creando..."
                        } else {
                            action_label
                        }),
                    )
                    .clicked()
                {
                    app.submit_create_directory_dialog();
                }
            });
        });

    if !open {
        app.state_mut().create_directory_dialog.clear();
    }
}

fn restore_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().restore_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().restore_dialog.open;
    let entries = app.state().restore_dialog.entries.clone();
    let selected_entry_id = app.state().restore_dialog.selected_entry_id;
    let restore_error = app.state().restore_dialog.error.clone();
    let restore_pending = app.state().restore_dialog.pending;
    let pending_message = app.state().restore_dialog.pending_message.clone();

    egui::Window::new("Restaurar desde historial")
        .open(&mut open)
        .collapsible(false)
        .default_size([780.0, 460.0])
        .show(ctx, |ui| {
            ui.label("Elige una versión previa o una cuenta eliminada para restaurarla en el vault actual.");
            ui.separator();

            ui.columns(2, |columns| {
                columns[0].vertical(|ui| {
                    ui.label(RichText::new("Versiones disponibles").strong());
                    ui.add_space(6.0);
                    if restore_pending && entries.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new());
                            ui.label(
                                RichText::new(
                                    pending_message
                                        .as_deref()
                                        .unwrap_or("Cargando historial restaurable."),
                                )
                                .small()
                                .color(palette.secondary_text),
                            );
                        });
                    } else if entries.is_empty() {
                        ui.label(
                            RichText::new(
                                "No hay cuentas eliminadas ni versiones previas restaurables por ahora.",
                            )
                            .small()
                            .color(palette.secondary_text),
                        );
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for entry in &entries {
                                let selected = Some(entry.entry_id) == selected_entry_id;
                                let response = ui.selectable_label(
                                    selected,
                                    format!(
                                        "{} | {} | {}",
                                        entry.event.label(),
                                        entry.account.display_name(),
                                        relative_age_label(entry.captured_at),
                                    ),
                                );

                                if response.clicked() {
                                    app.state_mut().restore_dialog.selected_entry_id =
                                        Some(entry.entry_id);
                                }
                            }
                        });
                    }
                });

                columns[1].vertical(|ui| {
                    ui.label(RichText::new("Preview").strong());
                    ui.add_space(6.0);

                    if let Some(selected) = entries
                        .iter()
                        .find(|entry| Some(entry.entry_id) == selected_entry_id)
                    {
                        preview_history_entry(ui, palette, selected);
                    } else {
                        ui.label("Selecciona una versión para ver el detalle.");
                    }
                });
            });

            if let Some(error) = &restore_error {
                ui.add_space(8.0);
                ui.colored_label(palette.error_text, error);
            }

            if restore_pending && !entries.is_empty() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new(
                            pending_message
                                .as_deref()
                                .unwrap_or("Procesando historial restaurable."),
                        )
                        .small()
                        .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cerrar").clicked() {
                    app.state_mut().restore_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !restore_pending && selected_entry_id.is_some(),
                        egui::Button::new(if restore_pending {
                            "📥 Restaurando..."
                        } else {
                            "📥 Restaurar versión seleccionada"
                        }),
                    )
                    .clicked()
                {
                    app.restore_selected_history_entry();
                }
            });
        });

    if !open {
        app.state_mut().restore_dialog.clear();
    }
}

fn remove_directory_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().remove_directory_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().remove_directory_dialog.open;
    let path = app.state().remove_directory_dialog.path.clone();

    egui::Window::new("Eliminar workspace vacío")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "Solo puedes eliminar workspaces o subdirectorios que no tengan cuentas ni hijos.",
                )
                .color(palette.warning_text),
            );
            ui.separator();
            ui.label("Ruta seleccionada");
            ui.label(RichText::new(path).monospace());

            if let Some(error) = &app.state().remove_directory_dialog.error {
                ui.add_space(6.0);
                ui.colored_label(palette.error_text, error);
            }

            if app.state().remove_directory_dialog.pending {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new("Eliminando el workspace y sincronizando el vault.")
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cancelar").clicked() {
                    app.state_mut().remove_directory_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().remove_directory_dialog.pending,
                        egui::Button::new(if app.state().remove_directory_dialog.pending {
                            "🗑 Eliminando..."
                        } else {
                            "🗑 Eliminar workspace"
                        }),
                    )
                    .clicked()
                {
                    app.confirm_remove_selected_directory();
                }
            });
        });

    if !open {
        app.state_mut().remove_directory_dialog.clear();
    }
}

fn change_password_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().change_password_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().change_password_dialog.open;

    egui::Window::new("Rotar contraseña maestra")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(440.0)
        .show(ctx, |ui| {
            ui.label("Re-cifra el vault actual con una nueva contraseña maestra.");
            ui.separator();

            ui.label("Nueva contraseña");
            ui.add(
                TextEdit::singleline(&mut app.state_mut().change_password_dialog.new_password)
                    .password(true)
                    .hint_text("Ingresa una contraseña fuerte"),
            );

            ui.label("Confirmar nueva contraseña");
            ui.add(
                TextEdit::singleline(&mut app.state_mut().change_password_dialog.confirm_password)
                    .password(true)
                    .hint_text("Repite la nueva contraseña"),
            );

            ui.label(
                RichText::new(
                    "La sesión desbloqueada seguirá activa si la rotación termina correctamente.",
                )
                .small()
                .color(palette.secondary_text),
            );

            if let Some(error) = &app.state().change_password_dialog.error {
                ui.colored_label(palette.error_text, error);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cancelar").clicked() {
                    app.state_mut().change_password_dialog.clear();
                }
                if ui.button("Aplicar rotación").clicked() {
                    app.submit_change_password_dialog();
                }
            });
        });

    if !open {
        app.state_mut().change_password_dialog.clear();
    }
}

fn token_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().token_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().token_dialog.open;
    let mut close_requested = false;

    egui::Window::new("Token TOTP")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            if let Some(account) = app.selected_account() {
                ui.label(
                    RichText::new(account.display_name())
                        .strong()
                        .size(20.0)
                        .color(palette.brand_accent),
                );
                ui.label(
                    RichText::new(
                        account
                            .metadata
                            .project_path
                            .as_deref()
                            .unwrap_or("Sin workspace"),
                    )
                    .small()
                    .color(palette.secondary_text),
                );
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !app.state().token_dialog.pending,
                        egui::Button::new(if app.state().token_dialog.pending {
                            "⏳ Actualizando..."
                        } else {
                            "🔄 Refrescar ahora"
                        }),
                    )
                    .clicked()
                {
                    app.refresh_token_for_selected();
                    ui.ctx().request_repaint();
                }
                if ui
                    .add_enabled(app.selected_token().is_some(), egui::Button::new("📋 Copiar código"))
                    .clicked()
                {
                    app.copy_selected_token(ctx);
                }
                if ui.button("🚪 Cerrar").clicked() {
                    close_requested = true;
                }
            });

            if !close_requested {
                ui.add_space(10.0);

                if let Some(token) = app.selected_token() {
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().faint_bg_color)
                        .stroke(egui::Stroke::new(1.0, palette.surface_stroke))
                        .inner_margin(egui::Margin::symmetric(14.0, 14.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(&token.code)
                                        .monospace()
                                        .size(42.0)
                                        .strong()
                                        .color(palette.token_text),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "Código vigente del período actual · expira en {}s",
                                        token.seconds_remaining
                                    ))
                                    .small()
                                    .color(palette.detail_label),
                                );
                            });
                        });
                }

                if let Some(message) = &app.state().token_dialog.action_message {
                    let (text_color, fill_color) = token_feedback_style(
                        app.state().token_dialog.action_tone,
                        palette,
                    );

                    ui.add_space(8.0);
                    egui::Frame::none()
                        .fill(fill_color)
                        .stroke(egui::Stroke::new(1.0, text_color.linear_multiply(0.35)))
                        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(message).small().color(text_color));
                        });
                } else {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Si el período TOTP sigue vigente, un refresh puede devolver el mismo código sin ser un fallo.",
                        )
                        .small()
                        .color(palette.secondary_text),
                    );
                }

                if app.state().token_dialog.pending {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(
                            RichText::new("La lectura segura del vault sigue en curso.")
                                .small()
                                .color(palette.secondary_text),
                        );
                    });
                }

                if let Some(error) = &app.state().token_dialog.error {
                    ui.add_space(8.0);
                    ui.colored_label(palette.error_text, error);
                }
            }
        });

    if close_requested || !open {
        app.state_mut().token_dialog.close();
    }
}

fn token_feedback_style(
    tone: Option<BannerTone>,
    palette: theme::ThemePalette,
) -> (egui::Color32, egui::Color32) {
    match tone.unwrap_or(BannerTone::Info) {
        BannerTone::Info => (palette.info_text, palette.info_text.linear_multiply(0.08)),
        BannerTone::Success => (
            palette.success_text,
            palette.success_text.linear_multiply(0.08),
        ),
        BannerTone::Warning => (
            palette.warning_text,
            palette.warning_text.linear_multiply(0.08),
        ),
        BannerTone::Error => (palette.error_text, palette.error_text.linear_multiply(0.08)),
    }
}

fn export_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().export_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().export_dialog.open;

    egui::Window::new("Exportar datos")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.label("Exporta el vault actual como un backup cifrado compatible con MFA-Forge.");
            ui.separator();

            ui.label(format!("Vault actual: {}", app.vault_path()));
            ui.label(format!("Cuentas: {}", app.total_accounts()));
            ui.label("Formato: backup cifrado MFA-Forge");

            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "El archivo exportado no renderiza estructuras grandes en pantalla y mantiene el formato de backup compatible para reimportación.",
                )
                .small()
                .color(palette.secondary_text),
            );

            if let Some(error) = &app.state().export_dialog.error {
                ui.add_space(8.0);
                ui.colored_label(palette.error_text, error);
            }

            if app.state().export_dialog.pending {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new("Exportando el backup del vault.")
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cerrar").clicked() {
                    app.state_mut().export_dialog.close();
                }
                if ui
                    .add_enabled(
                        !app.state().export_dialog.pending,
                        egui::Button::new(if app.state().export_dialog.pending {
                            "📤 Exportando..."
                        } else {
                            "📤 Guardar backup"
                        }),
                    )
                    .clicked()
                {
                    app.export_vault_backup();
                }
            });
        });

    if !open {
        app.state_mut().export_dialog.close();
    }
}

fn account_uri_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().account_uri_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().account_uri_dialog.open;

    egui::Window::new("Exportar cuenta como URI")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(620.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(&app.state().account_uri_dialog.account_label)
                    .strong()
                    .color(palette.brand_accent),
            );
            ui.label(
                RichText::new(
                    "Este URI incluye el secreto. Solo se muestra porque la exportación fue solicitada explícitamente.",
                )
                .small()
                .color(palette.warning_text),
            );

            if app.state().account_uri_dialog.pending {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new("Preparando el URI de exportación.")
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            } else {
                let mut reveal_uri = app.state().account_uri_dialog.reveal;
                ui.separator();
                ui.checkbox(&mut reveal_uri, "Mostrar URI completo");
                app.state_mut().account_uri_dialog.reveal = reveal_uri;
                ui.add(
                    TextEdit::singleline(&mut app.state_mut().account_uri_dialog.uri)
                        .desired_width(560.0)
                        .password(!reveal_uri),
                );
            }

            if let Some(error) = &app.state().account_uri_dialog.error {
                ui.add_space(8.0);
                ui.colored_label(palette.error_text, error);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cerrar").clicked() {
                    app.state_mut().account_uri_dialog.close();
                }
                if ui
                    .add_enabled(
                        !app.state().account_uri_dialog.pending
                            && !app.state().account_uri_dialog.uri.is_empty(),
                        egui::Button::new("📋 Copiar URI"),
                    )
                    .clicked()
                {
                    ctx.copy_text(app.state().account_uri_dialog.uri.clone());
                    app.set_banner(
                        BannerTone::Success,
                        "URI de la cuenta copiado al portapapeles.",
                    );
                }
            });
        });

    if !open {
        app.state_mut().account_uri_dialog.close();
    }
}

fn remove_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().remove_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().remove_dialog.open;

    egui::Window::new("Eliminar cuenta")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            let account_labels = app.state().remove_dialog.account_labels.clone();
            if !account_labels.is_empty() {
                ui.label(
                    RichText::new("Esta acción elimina la cuenta del vault local.")
                        .color(palette.warning_text),
                );
                ui.separator();
                if account_labels.len() == 1 {
                    ui.label(format!("Cuenta: {}", account_labels[0]));
                } else {
                    ui.label(format!("Cuentas seleccionadas: {}", account_labels.len()));
                    for label in account_labels.iter().take(5) {
                        ui.label(format!("• {label}"));
                    }
                    if account_labels.len() > 5 {
                        ui.label(format!("... y {} más.", account_labels.len() - 5));
                    }
                }
            }

            if let Some(error) = &app.state().remove_dialog.error {
                ui.colored_label(palette.error_text, error);
            }

            if app.state().remove_dialog.pending {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new("Eliminando la cuenta y sincronizando el vault.")
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("✖ Cancelar").clicked() {
                    app.state_mut().remove_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().remove_dialog.pending,
                        egui::Button::new(if app.state().remove_dialog.pending {
                            "🗑 Eliminando..."
                        } else {
                            "🗑 Eliminar definitivamente"
                        }),
                    )
                    .clicked()
                {
                    app.confirm_remove_selected();
                }
            });
        });

    if !open {
        app.state_mut().remove_dialog.clear();
    }
}

fn account_form_fields(
    ui: &mut egui::Ui,
    directories: &[mfa_forge_core::ProjectDirectory],
    form: &mut AccountFormState,
    editing: bool,
) {
    ui.label("Servicio");
    ui.text_edit_singleline(&mut form.service);

    ui.label("Usuario");
    ui.text_edit_singleline(&mut form.user);

    ui.label(if editing {
        "Nuevo secreto Base32 (opcional)"
    } else {
        "Secreto Base32"
    });
    ui.add(
        TextEdit::singleline(&mut form.secret)
            .password(true)
            .hint_text(if editing {
                "Deja vacío para conservar el secreto actual"
            } else {
                "JBSWY3DPEHPK3PXP"
            }),
    );

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label("Algoritmo");
            egui::ComboBox::from_id_salt(if editing {
                "edit_algorithm"
            } else {
                "add_algorithm"
            })
            .selected_text(form.algorithm.as_str())
            .width(110.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut form.algorithm, TotpAlgorithm::Sha1, "sha1");
                ui.selectable_value(&mut form.algorithm, TotpAlgorithm::Sha256, "sha256");
                ui.selectable_value(&mut form.algorithm, TotpAlgorithm::Sha512, "sha512");
            });
        });

        ui.vertical(|ui| {
            ui.label("Dígitos");
            ui.add(
                TextEdit::singleline(&mut form.digits)
                    .desired_width(64.0)
                    .hint_text("6"),
            );
        });

        ui.vertical(|ui| {
            ui.label("Período (s)");
            ui.add(
                TextEdit::singleline(&mut form.period_seconds)
                    .desired_width(80.0)
                    .hint_text("30"),
            );
        });
    });

    directory_assignment_fields(ui, directories, &mut form.metadata);
}

fn directory_assignment_fields(
    ui: &mut egui::Ui,
    directories: &[mfa_forge_core::ProjectDirectory],
    metadata: &mut MetadataFormState,
) {
    ui.add_space(10.0);
    ui.separator();
    ui.label(RichText::new("Workspace / directorio").strong());
    ui.label(
        "La cuenta nueva hereda el workspace seleccionado en la navegación. Puedes confirmarlo o cambiarlo aquí.",
    );

    let current_directory = if metadata.project_path.trim().is_empty() {
        "Sin workspace".to_owned()
    } else {
        metadata.project_path.clone()
    };

    ui.label(
        RichText::new(format!("Destino actual: {current_directory}"))
            .small()
            .color(ui.visuals().weak_text_color()),
    );

    egui::ComboBox::from_id_salt(ui.next_auto_id())
        .selected_text(current_directory)
        .width(320.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(metadata.project_path.trim().is_empty(), "Sin workspace")
                .clicked()
            {
                metadata.project_path.clear();
            }

            for directory in directories {
                if ui
                    .selectable_label(metadata.project_path == directory.path, &directory.path)
                    .clicked()
                {
                    metadata.project_path = directory.path.clone();
                }
            }
        });
}

fn preview_history_entry(
    ui: &mut egui::Ui,
    palette: theme::ThemePalette,
    entry: &mfa_forge_core::AccountHistoryEntryPublic,
) {
    let account = &entry.account;
    ui.label(
        RichText::new(account.display_name())
            .strong()
            .color(palette.brand_accent),
    );
    ui.label(
        RichText::new(format!(
            "{} • {}",
            entry.event.label(),
            relative_age_label(entry.captured_at)
        ))
        .small()
        .color(palette.secondary_text),
    );
    ui.separator();
    ui.label(format!("Servicio: {}", account.service));
    ui.label(format!("Usuario: {}", account.user));
    ui.label(format!("Factor: {}", account.kind));
    ui.label(format!(
        "TOTP: {} / {} dígitos / {}s",
        account.totp.algorithm, account.totp.digits, account.totp.period_seconds
    ));

    if let Some(project_path) = account.metadata.project_path.as_deref() {
        ui.label(format!("Workspace: {project_path}"));
    }
}

fn relative_age_label(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(timestamp);
    let delta = now.saturating_sub(timestamp);

    match delta {
        0..=59 => format!("hace {}s", delta),
        60..=3_599 => format!("hace {}m", delta / 60),
        3_600..=86_399 => format!("hace {}h", delta / 3_600),
        _ => format!("hace {}d", delta / 86_400),
    }
}
