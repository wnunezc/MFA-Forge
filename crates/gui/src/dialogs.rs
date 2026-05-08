use eframe::egui::{self, RichText, TextEdit};

use mfa_forge_core::TotpAlgorithm;

use crate::{
    app::ForgeApp,
    i18n::{tr, trf},
    state::{AccountFormState, BannerTone, MetadataFormState},
    theme,
};

pub fn render(ctx: &egui::Context, app: &mut ForgeApp) {
    notice_dialog(ctx, app);
    update_dialog(ctx, app);
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
                RichText::new(tr(
                    "This notice does not expose secrets and only confirms the session state.",
                ))
                .small()
                .color(palette.secondary_text),
            );

            ui.separator();
            if ui.button(format!("✖ {}", tr("Close"))).clicked() {
                app.state_mut().notice_dialog.close();
            }
        });

    if !open {
        app.state_mut().notice_dialog.close();
    }
}

fn update_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().update_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().update_dialog.open;
    let current_version = app.current_release_version();
    let target_tag = match app.next_release_tag() {
        Ok(tag) => tag,
        Err(error) => {
            app.state_mut().update_dialog.error = Some(error);
            "unknown".to_owned()
        }
    };
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
                "MFA-Forge does not auto-update when you open the GUI. This action starts the installed launcher for the next RC, verifies the published checksum, and then hands control to Windows Installer.",
            ));
            ui.separator();

            ui.label(RichText::new(tr("Current version")).strong());
            ui.label(current_version);

            ui.add_space(6.0);
            ui.label(RichText::new(tr("Target RC")).strong());
            ui.label(target_tag);

            ui.add_space(6.0);
            ui.label(RichText::new(tr("Stage directory")).strong());
            ui.label(stage_directory);

            ui.add_space(8.0);
            ui.label(
                RichText::new(tr(
                    "The launcher stays explicit: no background update, no silent install, and no vault access.",
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
                    app.start_next_rc_update();
                }
            });
        });

    if !open {
        app.state_mut().update_dialog.close();
    }
}

fn add_account_dialog(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().add_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().add_dialog.open;

    egui::Window::new(tr("Add MFA account"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .show(ctx, |ui| {
            let directories = app.directories().to_vec();
            ui.label(tr(
                "Load a TOTP account without revealing the secret in the interface.",
            ));
            ui.separator();

            account_form_fields(
                ui,
                &directories,
                &mut app.state_mut().add_dialog.form,
                false,
            );

            ui.label(
                RichText::new(tr(
                    "The master password and the secret are never printed by default.",
                ))
                .small()
                .color(palette.warning_text),
            );

            if let Some(error) = &app.state().add_dialog.error {
                ui.colored_label(palette.error_text, error);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Cancel"))).clicked() {
                    app.state_mut().add_dialog.clear();
                }
                if ui.button(format!("💾 {}", tr("Save account"))).clicked() {
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

    egui::Window::new(tr("Edit MFA account"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .show(ctx, |ui| {
            let directories = app.directories().to_vec();
            ui.label(tr(
                "Update the workspace or the TOTP settings. Leave the secret empty to keep the current one.",
            ));
            ui.separator();

            account_form_fields(ui, &directories, &mut app.state_mut().edit_dialog.form, true);

            ui.label(
                RichText::new(tr(
                    "If you do not change the secret, MFA-Forge reuses the encrypted material already stored in the vault.",
                ))
                    .small()
                    .color(palette.secondary_text),
            );

            if let Some(error) = &app.state().edit_dialog.error {
                ui.colored_label(palette.error_text, error);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Cancel"))).clicked() {
                    app.state_mut().edit_dialog.clear();
                }
                if ui.button(format!("💾 {}", tr("Save changes"))).clicked() {
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

    egui::Window::new(tr("Import otpauth://"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(580.0)
        .show(ctx, |ui| {
            let directories = app.directories().to_vec();
            ui.label(tr(
                "Paste a valid `otpauth://` URI. MFA-Forge will extract the service, user, and TOTP settings.",
            ));
            ui.separator();

            ui.label(tr("Import URI"));
            ui.add(
                TextEdit::singleline(&mut app.state_mut().import_dialog.uri)
                    .password(true)
                    .hint_text("otpauth://totp/service:user?..."),
            );

            directory_assignment_fields(
                ui,
                &directories,
                &mut app.state_mut().import_dialog.metadata,
            );

            ui.label(
                RichText::new(tr(
                    "The URI contains the secret: it is hidden on screen and cleared when the dialog closes.",
                ))
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
                        RichText::new(tr("Importing the account without blocking the interface."))
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Cancel"))).clicked() {
                    app.state_mut().import_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().import_dialog.pending,
                        egui::Button::new(if app.state().import_dialog.pending {
                            format!("📤 {}", tr("Importing..."))
                        } else {
                            format!("📤 {}", tr("Import"))
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

    egui::Window::new(format!("📤 {}", tr("Import from QR")))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(580.0)
        .show(ctx, |ui| {
            let directories = app.directories().to_vec();
            ui.label(tr("Provide the path to a local image that contains an `otpauth://` QR."));
            ui.separator();

            ui.label(tr("Image path"));
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut app.state_mut().import_qr_dialog.image_path)
                        .hint_text("D:/path/to/qr.png")
                        .desired_width(360.0),
                );
                if ui.button(format!("🔍 {}", tr("Browse..."))).clicked() {
                    app.browse_import_qr_image();
                }
            });

            directory_assignment_fields(
                ui,
                &directories,
                &mut app.state_mut().import_qr_dialog.metadata,
            );

            ui.label(
                RichText::new(tr(
                    "The image is only used to extract the `otpauth://`; the secret is not left visible in the UI.",
                ))
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
                        RichText::new(tr("Importing the account from the QR image."))
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Cancel"))).clicked() {
                    app.state_mut().import_qr_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().import_qr_dialog.pending,
                        egui::Button::new(if app.state().import_qr_dialog.pending {
                            format!("📥 {}", tr("Importing..."))
                        } else {
                            format!("📤 {}", tr("Import QR"))
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

    egui::Window::new(tr("Import account from file"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(600.0)
        .show(ctx, |ui| {
            let directories = app.directories().to_vec();
            ui.label(tr(
                "Select a compatible file that contains an explicitly exported `otpauth://` URI.",
            ));
            ui.separator();

            ui.label(tr("Account file"));
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut app.state_mut().import_file_dialog.file_path)
                        .hint_text("D:/path/to/account.otpauth")
                        .desired_width(380.0),
                );
                if ui.button(format!("🔍 {}", tr("Browse..."))).clicked() {
                    app.browse_import_file_dialog();
                }
            });

            directory_assignment_fields(
                ui,
                &directories,
                &mut app.state_mut().import_file_dialog.metadata,
            );

            ui.label(
                RichText::new(tr(
                    "The file is validated before import. If it does not contain a valid `otpauth://`, it is rejected.",
                ))
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
                        RichText::new(tr("Importing the account from a file."))
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Cancel"))).clicked() {
                    app.state_mut().import_file_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().import_file_dialog.pending,
                        egui::Button::new(if app.state().import_file_dialog.pending {
                            format!("📥 {}", tr("Importing..."))
                        } else {
                            format!("📥 {}", tr("Import file"))
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
        format!("📁 {}", tr("Create subdirectory"))
    } else {
        format!("🏢 {}", tr("Create root workspace"))
    };
    let action_label = if is_subdirectory {
        format!("📁 {}", tr("Create subdirectory"))
    } else {
        format!("🏢 {}", tr("Create root workspace"))
    };

    egui::Window::new(window_title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .show(ctx, |ui| {
            if let Some(parent_path) = parent_path.as_deref() {
                ui.label(trf("Selected workspace: {path}", &[("path", parent_path)]));
                ui.label(
                    RichText::new(tr("The name you enter will be added under this path."))
                        .small()
                        .color(palette.secondary_text),
                );
            } else {
                ui.label(tr(
                    "Create a root workspace to group accounts from the same project.",
                ));
            }

            ui.separator();
            ui.label(tr("Project / directory name"));
            ui.add(
                TextEdit::singleline(&mut app.state_mut().create_directory_dialog.name)
                    .hint_text("client-a, auth, prod, mobile..."),
            );

            ui.add_space(8.0);
            ui.label(tr("Resulting path"));
            if resulting_path.trim().is_empty() {
                ui.label(
                    RichText::new(tr("It will be completed after you enter the name."))
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
                        RichText::new(tr("Creating the workspace and syncing the vault."))
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Cancel"))).clicked() {
                    app.state_mut().create_directory_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().create_directory_dialog.pending,
                        egui::Button::new(if app.state().create_directory_dialog.pending {
                            tr("Creating...")
                        } else {
                            action_label.clone()
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

    egui::Window::new(tr("Restore from history"))
        .open(&mut open)
        .collapsible(false)
        .default_size([780.0, 460.0])
        .show(ctx, |ui| {
            ui.label(tr(
                "Choose a previous version or a removed account to restore it into the current vault.",
            ));
            ui.separator();

            ui.columns(2, |columns| {
                columns[0].vertical(|ui| {
                    ui.label(RichText::new(tr("Available versions")).strong());
                    ui.add_space(6.0);
                    if restore_pending && entries.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new());
                            ui.label(
                                RichText::new(
                                    pending_message
                                        .as_deref()
                                        .unwrap_or(&tr("Loading restorable history.")),
                                )
                                .small()
                                .color(palette.secondary_text),
                            );
                        });
                    } else if entries.is_empty() {
                        ui.label(
                            RichText::new(tr(
                                "No removed accounts or previous versions are available to restore right now.",
                            ))
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
                    ui.label(RichText::new(tr("Preview")).strong());
                    ui.add_space(6.0);

                    if let Some(selected) = entries
                        .iter()
                        .find(|entry| Some(entry.entry_id) == selected_entry_id)
                    {
                        preview_history_entry(ui, palette, selected);
                    } else {
                        ui.label(tr("Select a version to see the details."));
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
                                .unwrap_or(&tr("Processing restorable history.")),
                        )
                        .small()
                        .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Close"))).clicked() {
                    app.state_mut().restore_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !restore_pending && selected_entry_id.is_some(),
                        egui::Button::new(if restore_pending {
                            format!("📥 {}", tr("Restoring..."))
                        } else {
                            format!("📥 {}", tr("Restore selected version"))
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

    egui::Window::new(tr("Remove empty workspace"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    tr("You can only remove workspaces or subdirectories that have no accounts or children."),
                )
                .color(palette.warning_text),
            );
            ui.separator();
            ui.label(tr("Selected path"));
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
                        RichText::new(tr("Removing the workspace and syncing the vault."))
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Cancel"))).clicked() {
                    app.state_mut().remove_directory_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().remove_directory_dialog.pending,
                        egui::Button::new(if app.state().remove_directory_dialog.pending {
                            format!("🗑 {}", tr("Removing..."))
                        } else {
                            format!("🗑 {}", tr("Remove workspace"))
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

    egui::Window::new(tr("Rotate master password"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(440.0)
        .show(ctx, |ui| {
            ui.label(tr(
                "Re-encrypt the current vault with a new master password.",
            ));
            ui.separator();

            ui.label(tr("New password"));
            ui.add(
                TextEdit::singleline(&mut app.state_mut().change_password_dialog.new_password)
                    .password(true)
                    .hint_text(tr("Enter a strong password")),
            );

            ui.label(tr("Confirm new password"));
            ui.add(
                TextEdit::singleline(&mut app.state_mut().change_password_dialog.confirm_password)
                    .password(true)
                    .hint_text(tr("Repeat the new password")),
            );

            ui.label(
                RichText::new(tr(
                    "The unlocked session stays active if the rotation completes successfully.",
                ))
                .small()
                .color(palette.secondary_text),
            );

            if let Some(error) = &app.state().change_password_dialog.error {
                ui.colored_label(palette.error_text, error);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Cancel"))).clicked() {
                    app.state_mut().change_password_dialog.clear();
                }
                if ui.button(tr("Apply rotation")).clicked() {
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

    egui::Window::new(tr("TOTP token"))
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
                            .unwrap_or(&tr("No workspace")),
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
                            format!("⏳ {}", tr("Updating..."))
                        } else {
                            format!("🔄 {}", tr("Refresh now"))
                        }),
                    )
                    .clicked()
                {
                    app.refresh_token_for_selected();
                    ui.ctx().request_repaint();
                }
                if ui
                    .add_enabled(
                        app.selected_token().is_some(),
                        egui::Button::new(format!("📋 {}", tr("Copy code"))),
                    )
                    .clicked()
                {
                    app.copy_selected_token(ctx);
                }
                if ui.button(format!("🚪 {}", tr("Close"))).clicked() {
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
                                    RichText::new(trf(
                                        "Current period code · expires in {seconds}s",
                                        &[("seconds", &token.seconds_remaining.to_string())],
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
                            tr("If the TOTP period is still active, a refresh can return the same code without being a failure."),
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
                            RichText::new(tr("Secure vault reading is still in progress."))
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

    egui::Window::new(tr("Export data"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.label(tr("Export the current vault as an MFA-Forge-compatible encrypted backup."));
            ui.separator();

            ui.label(trf("Current vault: {path}", &[("path", app.vault_path())]));
            ui.label(trf(
                "Accounts: {count}",
                &[("count", &app.total_accounts().to_string())],
            ));
            ui.label(tr("Format: MFA-Forge encrypted backup"));

            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    tr("The exported file does not render large structures on screen and keeps the compatible backup format for reimport."),
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
                        RichText::new(tr("Exporting the vault backup."))
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Close"))).clicked() {
                    app.state_mut().export_dialog.close();
                }
                if ui
                    .add_enabled(
                        !app.state().export_dialog.pending,
                        egui::Button::new(if app.state().export_dialog.pending {
                            format!("📤 {}", tr("Exporting..."))
                        } else {
                            format!("📤 {}", tr("Save backup"))
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

    egui::Window::new(tr("Export account as URI"))
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
                    tr("This URI includes the secret. It is only shown because the export was explicitly requested."),
                )
                .small()
                .color(palette.warning_text),
            );

            if app.state().account_uri_dialog.pending {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new(tr("Preparing the export URI."))
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            } else {
                let mut reveal_uri = app.state().account_uri_dialog.reveal;
                ui.separator();
                ui.checkbox(&mut reveal_uri, tr("Show full URI"));
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
                if ui.button(format!("✖ {}", tr("Close"))).clicked() {
                    app.state_mut().account_uri_dialog.close();
                }
                if ui
                    .add_enabled(
                        !app.state().account_uri_dialog.pending
                            && !app.state().account_uri_dialog.uri.is_empty(),
                        egui::Button::new(format!("📋 {}", tr("Copy URI"))),
                    )
                    .clicked()
                {
                    ctx.copy_text(app.state().account_uri_dialog.uri.clone());
                    app.set_banner(
                        BannerTone::Success,
                        tr("Account URI copied to the clipboard."),
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

    egui::Window::new(tr("Remove account"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            let account_labels = app.state().remove_dialog.account_labels.clone();
            if !account_labels.is_empty() {
                ui.label(
                    RichText::new(tr("This action removes the account from the local vault."))
                        .color(palette.warning_text),
                );
                ui.separator();
                if account_labels.len() == 1 {
                    ui.label(trf("Account: {name}", &[("name", &account_labels[0])]));
                } else {
                    ui.label(trf(
                        "Selected accounts: {count}",
                        &[("count", &account_labels.len().to_string())],
                    ));
                    for label in account_labels.iter().take(5) {
                        ui.label(format!("• {label}"));
                    }
                    if account_labels.len() > 5 {
                        ui.label(trf(
                            "... and {count} more.",
                            &[("count", &(account_labels.len() - 5).to_string())],
                        ));
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
                        RichText::new(tr("Removing the account and syncing the vault."))
                            .small()
                            .color(palette.secondary_text),
                    );
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("✖ {}", tr("Cancel"))).clicked() {
                    app.state_mut().remove_dialog.clear();
                }
                if ui
                    .add_enabled(
                        !app.state().remove_dialog.pending,
                        egui::Button::new(if app.state().remove_dialog.pending {
                            format!("🗑 {}", tr("Removing..."))
                        } else {
                            format!("🗑 {}", tr("Remove permanently"))
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
    ui.label(tr("Service"));
    ui.text_edit_singleline(&mut form.service);

    ui.label(tr("User"));
    ui.text_edit_singleline(&mut form.user);

    ui.label(if editing {
        tr("New Base32 secret (optional)")
    } else {
        tr("Base32 secret")
    });
    ui.add(
        TextEdit::singleline(&mut form.secret)
            .password(true)
            .hint_text(if editing {
                tr("Leave empty to keep the current secret")
            } else {
                "BASE32_SECRET_HERE".to_owned()
            }),
    );

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(tr("Algorithm"));
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
            ui.label(tr("Digits"));
            ui.add(
                TextEdit::singleline(&mut form.digits)
                    .desired_width(64.0)
                    .hint_text("6"),
            );
        });

        ui.vertical(|ui| {
            ui.label(tr("Period (s)"));
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
    ui.label(RichText::new(tr("Workspace / directory")).strong());
    ui.label(
        tr("The new account inherits the workspace selected in navigation. You can confirm it or change it here."),
    );

    let current_directory = if metadata.project_path.trim().is_empty() {
        tr("No workspace")
    } else {
        metadata.project_path.clone()
    };

    ui.label(
        RichText::new(trf(
            "Current destination: {path}",
            &[("path", &current_directory)],
        ))
        .small()
        .color(ui.visuals().weak_text_color()),
    );

    egui::ComboBox::from_id_salt(ui.next_auto_id())
        .selected_text(current_directory)
        .width(320.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(metadata.project_path.trim().is_empty(), tr("No workspace"))
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
    ui.label(trf("Service: {value}", &[("value", &account.service)]));
    ui.label(trf("User: {value}", &[("value", &account.user)]));
    ui.label(trf(
        "Factor: {value}",
        &[("value", &account.kind.to_string())],
    ));
    ui.label(trf(
        "TOTP: {algorithm} / {digits} digits / {seconds}s",
        &[
            ("algorithm", &account.totp.algorithm.to_string()),
            ("digits", &account.totp.digits.to_string()),
            ("seconds", &account.totp.period_seconds.to_string()),
        ],
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
        0..=59 => trf("{count}s ago", &[("count", &delta.to_string())]),
        60..=3_599 => trf("{count}m ago", &[("count", &(delta / 60).to_string())]),
        3_600..=86_399 => trf("{count}h ago", &[("count", &(delta / 3_600).to_string())]),
        _ => trf("{count}d ago", &[("count", &(delta / 86_400).to_string())]),
    }
}
