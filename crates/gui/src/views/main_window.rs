use std::collections::BTreeMap;

use eframe::egui::{self, Color32, RichText, Stroke, TextEdit};

use mfa_forge_core::{AccountPublic, ProjectDirectory};

use crate::{app::ForgeApp, i18n::tr, state::WorkspaceScope, theme};

const NAV_PANEL_WIDTH: f32 = 258.0;
const TOOLBAR_HEIGHT: f32 = 56.0;
const SEARCH_WIDTH: f32 = 304.0;
const GRID_ROW_HEIGHT: f32 = 30.0;
const WORKSPACE_ROW_HEIGHT: f32 = 28.0;
const SURFACE_OUTER_MARGIN_X: f32 = 10.0;
const SURFACE_OUTER_MARGIN_Y: f32 = 4.0;
const SURFACE_INNER_MARGIN_X: f32 = 12.0;
const SURFACE_INNER_MARGIN_Y: f32 = 10.0;
const GRID_INNER_MARGIN_X: f32 = 12.0;
const GRID_INNER_MARGIN_Y: f32 = 10.0;
const CONTENT_BOTTOM_GAP: f32 = 8.0;
const MAIN_CONTENT_MARGIN_X: f32 = 10.0;
const MAIN_CONTENT_MARGIN_Y: f32 = 6.0;
const MAIN_PANE_GAP: f32 = 6.0;
const MAIN_RIGHT_GAP: f32 = 18.0;
const DELETE_BUTTON_WIDTH: f32 = 28.0;

#[derive(Clone, Copy)]
struct AccountGridWidths {
    select: f32,
    service: f32,
    user: f32,
    factor: f32,
    workspace: f32,
    actions: f32,
}

struct WorkspaceTreeData<'a> {
    directories: &'a [ProjectDirectory],
    counts: &'a BTreeMap<String, usize>,
    selected_directory_path: Option<&'a str>,
}

pub fn render(ctx: &egui::Context, app: &mut ForgeApp) {
    top_menu_bar(ctx, app);
    status_bar(ctx, app);
    main_content_panel(ctx, app);
}

fn status_bar(ctx: &egui::Context, app: &mut ForgeApp) {
    egui::TopBottomPanel::bottom("status_bar")
        .resizable(false)
        .exact_height(48.0)
        .show_separator_line(true)
        .frame(egui::Frame::none().fill(theme::palette(app.theme_preference()).status_fill))
        .show(ctx, |ui| {
            app.status_bar_ui(ui);
        });
}

fn top_menu_bar(ctx: &egui::Context, app: &mut ForgeApp) {
    let (toolbar_fill, title_color) = toolbar_colors(app.theme_preference());
    let selected_directory_path = app.selected_directory_path().map(str::to_owned);
    let vault_busy = app.has_background_vault_work();

    egui::TopBottomPanel::top("main_menu_bar")
        .resizable(false)
        .show_separator_line(true)
        .exact_height(TOOLBAR_HEIGHT)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(toolbar_fill)
                .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("MFA-Forge")
                                .size(19.0)
                                .strong()
                                .color(title_color),
                        );

                        ui.add_space(8.0);

                        ui.add_enabled_ui(!vault_busy, |ui| {
                            if toolbar_button(ui, "🏢 Workspace").clicked() {
                                app.open_create_directory_dialog(None);
                            }
                        });

                        ui.add_enabled_ui(selected_directory_path.is_some() && !vault_busy, |ui| {
                            if toolbar_button(ui, &format!("📁 {}", tr("Subdirectory"))).clicked()
                            {
                                app.open_create_directory_dialog(selected_directory_path.clone());
                            }
                        });

                        ui.add_enabled_ui(!vault_busy, |ui| {
                            ui.menu_button(format!("▶ {}", tr("Account")), |ui| {
                                if ui.button(tr("New account")).clicked() {
                                    app.open_add_dialog();
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button(tr("Import from file")).clicked() {
                                    app.open_import_file_dialog();
                                    ui.close_menu();
                                }
                                if ui.button(tr("Import from URI")).clicked() {
                                    app.open_import_dialog();
                                    ui.close_menu();
                                }
                                if ui.button(tr("Import from QR")).clicked() {
                                    app.open_import_qr_dialog();
                                    ui.close_menu();
                                }
                            });

                            ui.menu_button("▶ Vault", |ui| {
                                if ui.button(tr("Import from backup file")).clicked() {
                                    app.import_vault_backup();
                                    ui.close_menu();
                                }
                                if ui.button(tr("Export backup")).clicked() {
                                    app.open_export_dialog();
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button(tr("History")).clicked() {
                                    app.open_restore_dialog();
                                    ui.close_menu();
                                }
                                if ui.button(tr("Rotate password")).clicked() {
                                    app.open_change_password_dialog();
                                    ui.close_menu();
                                }
                            });

                            if toolbar_button(ui, &format!("❓ {}", tr("Help"))).clicked() {
                                app.open_help_dialog();
                            }
                        });

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let lock_clicked =
                                toolbar_button(ui, &format!("🔒 {}", tr("Lock"))).clicked();
                            let clear_clicked =
                                toolbar_button(ui, &format!("✖ {}", tr("Clear"))).clicked();

                            ui.add_sized(
                                [SEARCH_WIDTH, 32.0],
                                toolbar_search_input(&mut app.state_mut().search_query),
                            );

                            if clear_clicked {
                                app.state_mut().search_query.clear();
                                app.sync_selection();
                            }

                            if lock_clicked {
                                app.lock_vault();
                            }
                        });
                    });
                });
        });
}

fn ui_panel_fill(app: &ForgeApp) -> Color32 {
    match app.theme_preference() {
        theme::ThemePreference::Dark => Color32::from_rgb(20, 22, 26),
        theme::ThemePreference::Light => Color32::from_rgb(238, 241, 245),
    }
}

fn main_content_panel(ctx: &egui::Context, app: &mut ForgeApp) {
    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(ui_panel_fill(app))
                .inner_margin(egui::Margin::symmetric(
                    MAIN_CONTENT_MARGIN_X,
                    MAIN_CONTENT_MARGIN_Y,
                )),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

            let available_width = ui.available_width();
            let available_height = (ui.available_height() - CONTENT_BOTTOM_GAP).max(0.0);

            let accounts_width =
                (available_width - NAV_PANEL_WIDTH - MAIN_PANE_GAP - MAIN_RIGHT_GAP).max(0.0);

            ui.allocate_ui_with_layout(
                egui::vec2(available_width, available_height),
                egui::Layout::left_to_right(egui::Align::Min),
                |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(NAV_PANEL_WIDTH, available_height),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| render_workspace_pane(ui, app, available_height),
                    );

                    ui.add_space(MAIN_PANE_GAP);

                    ui.allocate_ui_with_layout(
                        egui::vec2(accounts_width, available_height),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| render_accounts_pane(ui, app, available_height),
                    );

                    ui.add_space(MAIN_RIGHT_GAP);
                },
            );

            ui.add_space(CONTENT_BOTTOM_GAP);
        });
}

fn render_workspace_pane(ui: &mut egui::Ui, app: &mut ForgeApp, pane_height: f32) {
    let palette = theme::palette(app.theme_preference());
    let directories = app.directories().to_vec();
    let counts = directory_account_counts(app.accounts());
    let selected_directory_path = app.selected_directory_path().map(str::to_owned);
    let unassigned_count = app
        .accounts()
        .iter()
        .filter(|account| account.metadata.project_path.is_none())
        .count();

    let tree = WorkspaceTreeData {
        directories: &directories,
        counts: &counts,
        selected_directory_path: selected_directory_path.as_deref(),
    };

    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), pane_height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            show_surface_panel(ui, palette, |ui| {
                ui.set_min_height(surface_content_min_height(pane_height));

                section_title(
                    ui,
                    palette,
                    "Workspaces",
                    &tr("Select a project. The top bar creates roots, subdirectories, and accounts on top of this view."),
                );

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                let unassigned_selected = app.workspace_scope().is_unassigned();
                if workspace_row(
                    ui,
                    palette,
                    &format!("{} ({unassigned_count})", tr("No workspace")),
                    0,
                    unassigned_selected,
                )
                    .clicked()
                {
                    select_unassigned_scope(app);
                }

                ui.add_space(4.0);

                if directories.is_empty() {
                    ui.label(
                        RichText::new("No workspaces have been created yet.")
                            .small()
                            .color(palette.secondary_text),
                    );
                } else {
                    ui.separator();
                    ui.add_space(6.0);

                    egui::ScrollArea::vertical()
                        .id_salt("workspace_tree_scroll")
                        .auto_shrink([false, false])
                        .max_height(ui.available_height().max(0.0))
                        .show(ui, |ui| {
                            for directory in child_directories(&directories, None) {
                                render_workspace_node(ui, palette, app, &tree, directory, 0);
                            }
                        });
                }
            });
        },
    );
}

fn render_accounts_pane(ui: &mut egui::Ui, app: &mut ForgeApp, pane_height: f32) {
    app.sync_selection();

    let palette = theme::palette(app.theme_preference());
    let visible_accounts = app.visible_accounts();
    let scope = app.workspace_scope().clone();
    let selected_directory_path = app.selected_directory_path().map(str::to_owned);
    let total_accounts = app.total_accounts();
    let active_search_query = app.active_search_query();
    let child_count = selected_directory_path
        .as_deref()
        .map(|path| child_directories(app.directories(), Some(path)).len())
        .unwrap_or_default();

    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), pane_height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            show_surface_panel(ui, palette, |ui| {
                ui.set_min_height(surface_content_min_height(pane_height));

                let (title, summary) = search_or_scope_heading(
                    &scope,
                    active_search_query.as_deref(),
                    visible_accounts.len(),
                    total_accounts,
                    child_count,
                );

                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(title)
                            .size(17.0)
                            .strong()
                            .color(palette.brand_accent),
                    );

                    ui.add_space(10.0);
                    ui.label(RichText::new(summary).small().color(palette.secondary_text));

                    if app.checked_account_count() > 0 {
                        ui.add_space(12.0);
                        if ui
                            .add_enabled(
                                !app.has_background_vault_work(),
                                egui::Button::new(format!(
                                    "🗑 Remove selected ({})",
                                    app.checked_account_count()
                                )),
                            )
                            .clicked()
                        {
                            app.open_remove_checked_accounts_dialog();
                        }
                    }
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                if app.is_search_pending() {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(
                            RichText::new("Searching accounts and workspaces...")
                                .small()
                                .color(palette.secondary_text),
                        );
                    });
                    return;
                }

                if visible_accounts.is_empty() {
                    render_empty_state(
                        ui,
                        palette,
                        &scope,
                        child_count,
                        active_search_query.as_deref(),
                    );
                    return;
                }

                render_accounts_grid(
                    ui,
                    palette,
                    app,
                    &visible_accounts,
                    selected_directory_path.as_deref(),
                );
            });
        },
    );
}

fn surface_content_min_height(pane_height: f32) -> f32 {
    (pane_height - (SURFACE_OUTER_MARGIN_Y * 2.0) - (SURFACE_INNER_MARGIN_Y * 2.0)).max(0.0)
}

fn render_empty_state(
    ui: &mut egui::Ui,
    palette: theme::ThemePalette,
    scope: &WorkspaceScope,
    child_count: usize,
    active_search_query: Option<&str>,
) {
    ui.add_space(6.0);

    if active_search_query.is_some() {
        ui.label(
            RichText::new("No accounts or workspaces matched the current view.")
                .small()
                .color(palette.secondary_text),
        );
        return;
    }

    let message = match scope {
        WorkspaceScope::Unassigned => {
            "There are no accounts without a workspace. Use the top bar to create one here or select an existing workspace."
        }
        WorkspaceScope::Directory(_) if child_count > 0 => {
            "This workspace has no direct accounts. Its subdirectories are still available in the left navigation."
        }
        WorkspaceScope::Directory(_) => {
            "This workspace is empty. Use the top bar to create an account or subdirectory inside the selected project."
        }
    };

    ui.label(RichText::new(message).small().color(palette.secondary_text));
}

fn render_accounts_grid(
    ui: &mut egui::Ui,
    palette: theme::ThemePalette,
    app: &mut ForgeApp,
    visible_accounts: &[AccountPublic],
    selected_directory_path: Option<&str>,
) {
    let grid_width = ui.available_width();
    let grid_height = ui.available_height();

    ui.allocate_ui_with_layout(
        egui::vec2(grid_width, grid_height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            egui::Frame::none()
                .fill(palette.surface_fill)
                .stroke(Stroke::new(1.0, palette.surface_stroke))
                .inner_margin(egui::Margin::symmetric(
                    GRID_INNER_MARGIN_X,
                    GRID_INNER_MARGIN_Y,
                ))
                .show(ui, |ui| {
                    let widths = account_grid_widths(ui.available_width());
                    let scroll_height = (ui.available_height() - 52.0).max(120.0);

                    render_accounts_header(ui, palette, widths);

                    ui.add_space(6.0);

                    egui::ScrollArea::vertical()
                        .id_salt("accounts_grid_scroll")
                        .auto_shrink([false, false])
                        .max_height(scroll_height)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());

                            for account in visible_accounts {
                                render_account_row(
                                    ui,
                                    palette,
                                    app,
                                    account,
                                    selected_directory_path,
                                    widths,
                                );
                                ui.add_space(5.0);
                            }
                        });
                });
        },
    );
}

fn render_accounts_header(
    ui: &mut egui::Ui,
    palette: theme::ThemePalette,
    widths: AccountGridWidths,
) {
    egui::Frame::none()
        .fill(ui.visuals().faint_bg_color)
        .stroke(Stroke::new(1.0, palette.surface_stroke))
        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                grid_header_label(ui, "", widths.select);
                grid_header_label(ui, "Service", widths.service);
                grid_header_label(ui, "User", widths.user);
                grid_header_label(ui, "Factor", widths.factor);
                grid_header_label(ui, "Workspace", widths.workspace);
                grid_header_label(ui, "Actions", widths.actions);
            });
        });
}

fn render_account_row(
    ui: &mut egui::Ui,
    palette: theme::ThemePalette,
    app: &mut ForgeApp,
    account: &AccountPublic,
    selected_directory_path: Option<&str>,
    widths: AccountGridWidths,
) {
    let is_selected = app.state().selected_account_id == Some(account.id);
    let row_fill = if is_selected {
        palette.brand_accent.linear_multiply(0.12)
    } else {
        palette.surface_fill
    };
    let row_stroke = if is_selected {
        palette.brand_accent.linear_multiply(0.60)
    } else {
        palette.surface_stroke
    };
    let mut checked = app.is_account_checked(account.id);

    let row_width = ui.available_width();

    ui.allocate_ui_with_layout(
        egui::vec2(row_width, GRID_ROW_HEIGHT + 18.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(row_width);

            egui::Frame::none()
                .fill(row_fill)
                .stroke(Stroke::new(1.0, row_stroke))
                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_min_width((row_width - 24.0).max(0.0));

                    ui.horizontal(|ui| {
                        let checkbox_response = ui.add_sized(
                            [widths.select, GRID_ROW_HEIGHT],
                            egui::Checkbox::without_text(&mut checked),
                        );
                        if checkbox_response.changed() {
                            app.toggle_account_checked(account.id, checked);
                        }

                        if ui
                            .add_sized(
                                [widths.service, GRID_ROW_HEIGHT],
                                egui::Label::new(
                                    RichText::new(&account.service).strong().size(14.0).color(
                                        if is_selected {
                                            palette.brand_accent
                                        } else {
                                            palette.strong_text
                                        },
                                    ),
                                )
                                .sense(egui::Sense::click()),
                            )
                            .clicked()
                        {
                            app.set_primary_account_selection(account.id);
                        }

                        ui.add_sized(
                            [widths.user, GRID_ROW_HEIGHT],
                            egui::Label::new(
                                RichText::new(&account.user)
                                    .size(13.0)
                                    .color(palette.detail_value),
                            )
                            .wrap(),
                        );

                        ui.add_sized(
                            [widths.factor, GRID_ROW_HEIGHT],
                            egui::Label::new(
                                RichText::new(format!(
                                    "{} / {}s",
                                    account.kind.as_str(),
                                    account.totp.period_seconds
                                ))
                                .size(13.0)
                                .color(palette.secondary_text),
                            ),
                        );

                        ui.add_sized(
                            [widths.workspace, GRID_ROW_HEIGHT],
                            egui::Label::new(
                                RichText::new(workspace_value(account, selected_directory_path))
                                    .size(13.0)
                                    .color(palette.secondary_text),
                            )
                            .wrap(),
                        );

                        ui.allocate_ui_with_layout(
                            egui::vec2(widths.actions, GRID_ROW_HEIGHT),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.centered_and_justified(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                                        let token_label = if app.pending_token_job.is_some() {
                                            "🔑 Token..."
                                        } else {
                                            "🔑 Token"
                                        };

                                        if ui
                                            .add_enabled(
                                                app.pending_token_job.is_none(),
                                                egui::Button::new(token_label)
                                                    .min_size(egui::vec2(78.0, 26.0)),
                                            )
                                            .clicked()
                                        {
                                            app.set_primary_account_selection(account.id);
                                            app.open_token_dialog();
                                        }

                                        ui.menu_button("▶ Actions", |ui| {
                                            if ui.button("Edit").clicked() {
                                                app.set_primary_account_selection(account.id);
                                                app.open_edit_dialog();
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                            if ui.button("Export to file").clicked() {
                                                app.set_primary_account_selection(account.id);
                                                app.export_selected_account_to_file();
                                                ui.close_menu();
                                            }
                                            if ui.button("Export as URI").clicked() {
                                                app.set_primary_account_selection(account.id);
                                                app.export_selected_account_uri();
                                                ui.close_menu();
                                            }
                                            if ui.button("Export as QR").clicked() {
                                                app.set_primary_account_selection(account.id);
                                                app.export_selected_account_qr();
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                            if ui.button("Delete").clicked() {
                                                app.set_primary_account_selection(account.id);
                                                app.open_remove_dialog();
                                                ui.close_menu();
                                            }
                                        });
                                    });
                                });
                            },
                        );
                    });
                });
        },
    );
}

fn render_workspace_node(
    ui: &mut egui::Ui,
    palette: theme::ThemePalette,
    app: &mut ForgeApp,
    tree: &WorkspaceTreeData<'_>,
    directory: ProjectDirectory,
    depth: usize,
) {
    let total_accounts = tree
        .counts
        .get(&directory.path)
        .copied()
        .unwrap_or_default();
    let children = child_directories(tree.directories, Some(directory.path.as_str()));
    let is_selected = tree.selected_directory_path == Some(directory.path.as_str());
    let label = format!("{} ({total_accounts})", directory.display_name());
    let can_remove = total_accounts == 0 && children.is_empty();

    ui.horizontal(|ui| {
        let indent = depth as f32 * 14.0;
        let reserved_width = if can_remove { DELETE_BUTTON_WIDTH } else { 0.0 };
        ui.add_space(indent);
        let row_width = (ui.available_width() - reserved_width).max(24.0);
        let response = ui
            .add_sized(
                [row_width, WORKSPACE_ROW_HEIGHT],
                egui::SelectableLabel::new(
                    is_selected,
                    RichText::new(&label).strong().color(if is_selected {
                        palette.brand_accent
                    } else {
                        palette.strong_text
                    }),
                ),
            )
            .on_hover_text(&directory.path);

        if response.clicked() {
            select_directory_scope(app, directory.path.clone());
        }

        if can_remove
            && ui
                .add_enabled(
                    !app.has_background_vault_work(),
                    egui::Button::new("🗑").min_size(egui::vec2(24.0, 24.0)),
                )
                .on_hover_text("Delete empty workspace")
                .clicked()
        {
            app.open_remove_directory_dialog(directory.path.clone());
        }
    });

    for child in children {
        render_workspace_node(ui, palette, app, tree, child, depth + 1);
    }
}

fn workspace_value(account: &AccountPublic, selected_directory_path: Option<&str>) -> String {
    let Some(project_path) = account.metadata.project_path.as_deref() else {
        return tr("No workspace");
    };

    if let Some(selected_directory_path) = selected_directory_path {
        if project_path == selected_directory_path {
            return "Current root".to_owned();
        }

        if let Some(relative) = project_path.strip_prefix(selected_directory_path) {
            let relative = relative.trim_start_matches('/');
            if !relative.is_empty() {
                return relative.to_owned();
            }
        }
    }

    project_path.to_owned()
}

fn directory_account_counts(accounts: &[AccountPublic]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();

    for account in accounts {
        let Some(project_path) = account.metadata.project_path.as_deref() else {
            continue;
        };

        let mut current = Some(project_path.to_owned());
        while let Some(path) = current {
            *counts.entry(path.clone()).or_insert(0) += 1;
            current = path.rsplit_once('/').map(|(parent, _)| parent.to_owned());
        }
    }

    counts
}

fn child_directories(
    directories: &[ProjectDirectory],
    parent_path: Option<&str>,
) -> Vec<ProjectDirectory> {
    let mut children = directories
        .iter()
        .filter(|directory| directory.parent_path() == parent_path)
        .cloned()
        .collect::<Vec<_>>();
    children.sort_by(|left, right| left.path.cmp(&right.path));
    children
}

fn search_or_scope_heading(
    scope: &WorkspaceScope,
    active_search_query: Option<&str>,
    visible_accounts: usize,
    total_accounts: usize,
    child_count: usize,
) -> (String, String) {
    if let Some(query) = active_search_query {
        return (
            tr("Search results"),
            format!(
                "\"{query}\" | {visible_accounts} match(es) across {total_accounts} vault account(s)."
            ),
        );
    }

    match scope {
        WorkspaceScope::Unassigned => (
            "Accounts without workspace".to_owned(),
            format!(
                "{visible_accounts} visible | {total_accounts} total in the vault. New accounts are created here while this view stays active."
            ),
        ),
        WorkspaceScope::Directory(path) => (
            path.clone(),
            format!(
                "{visible_accounts} visible | {child_count} direct subdirectory(ies). New accounts inherit this workspace while it stays selected."
            ),
        ),
    }
}

fn select_unassigned_scope(app: &mut ForgeApp) {
    app.state_mut().workspace_scope = WorkspaceScope::Unassigned;
    app.sync_selection();
}

fn select_directory_scope(app: &mut ForgeApp, path: String) {
    app.state_mut().workspace_scope = WorkspaceScope::Directory(path);
    app.sync_selection();
}

fn toolbar_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(label).min_size(egui::vec2(0.0, 30.0)))
}

fn toolbar_search_input(query: &mut String) -> TextEdit<'_> {
    TextEdit::singleline(query)
        .font(egui::TextStyle::Body)
        .hint_text(format!("🔍 {} (min. 3 chars)", tr("Search")))
        .hint_text_font(egui::TextStyle::Body)
        .margin(egui::Margin::symmetric(10.0, 5.0))
        .vertical_align(egui::Align::Center)
}

fn show_surface_panel(
    ui: &mut egui::Ui,
    palette: theme::ThemePalette,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(
            SURFACE_OUTER_MARGIN_X,
            SURFACE_OUTER_MARGIN_Y,
        ))
        .show(ui, |ui| {
            egui::Frame::group(ui.style())
                .fill(palette.surface_fill)
                .stroke(Stroke::new(1.0, palette.surface_stroke))
                .inner_margin(egui::Margin::symmetric(
                    SURFACE_INNER_MARGIN_X,
                    SURFACE_INNER_MARGIN_Y,
                ))
                .show(ui, |ui| add_contents(ui));
        });
}

fn section_title(ui: &mut egui::Ui, palette: theme::ThemePalette, title: &str, subtitle: &str) {
    ui.label(
        RichText::new(title)
            .size(15.5)
            .strong()
            .color(palette.brand_accent),
    );
    ui.label(
        RichText::new(subtitle)
            .small()
            .color(palette.secondary_text),
    );
}

fn toolbar_colors(preference: theme::ThemePreference) -> (Color32, Color32) {
    match preference {
        theme::ThemePreference::Dark => (
            Color32::from_rgb(22, 25, 30),
            Color32::from_rgb(244, 246, 249),
        ),
        theme::ThemePreference::Light => (
            Color32::from_rgb(233, 237, 243),
            Color32::from_rgb(46, 52, 60),
        ),
    }
}

fn grid_header_label(ui: &mut egui::Ui, label: &str, width: f32) {
    ui.add_sized(
        [width, 20.0],
        egui::Label::new(RichText::new(label).size(12.8).strong()),
    );
}

fn account_grid_widths(total_width: f32) -> AccountGridWidths {
    let content_width = (total_width - 24.0).max(420.0);
    let select = 32.0;
    let available = (content_width - select).max(388.0);

    let mut service = available * 0.24;
    let mut user = available * 0.20;
    let mut factor = available * 0.12;
    let mut workspace = available * 0.20;
    let actions;

    let minimums = [120.0, 110.0, 84.0, 120.0, 170.0];
    let total_minimum = minimums.iter().sum::<f32>();

    if available >= total_minimum {
        service = service.max(minimums[0]);
        user = user.max(minimums[1]);
        factor = factor.max(minimums[2]);
        workspace = workspace.max(minimums[3]);
        actions = (available - service - user - factor - workspace).max(minimums[4]);
    } else {
        let scale = available / total_minimum;
        service = minimums[0] * scale;
        user = minimums[1] * scale;
        factor = minimums[2] * scale;
        workspace = minimums[3] * scale;
        actions = minimums[4] * scale;
    }

    AccountGridWidths {
        select,
        service,
        user,
        factor,
        workspace,
        actions,
    }
}

fn workspace_row(
    ui: &mut egui::Ui,
    palette: theme::ThemePalette,
    label: &str,
    depth: usize,
    selected: bool,
) -> egui::Response {
    ui.horizontal(|ui| {
        let indent = depth as f32 * 14.0;
        ui.add_space(indent);
        let width = ui.available_width().max(24.0);
        ui.add_sized(
            [width, WORKSPACE_ROW_HEIGHT],
            egui::SelectableLabel::new(
                selected,
                RichText::new(label).strong().color(if selected {
                    palette.brand_accent
                } else {
                    palette.strong_text
                }),
            ),
        )
    })
    .inner
}
