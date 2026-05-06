use std::time::Duration;

use eframe::egui::{self, RichText};

use mfa_forge_core::TotpToken;

use crate::{
    app::ForgeApp,
    state::{AppState, Banner, BannerTone, LoaderMode, Screen},
    theme,
};

const STATUS_BAR_HEIGHT: f32 = 42.0;
const STATUS_BAR_ROW_HEIGHT: f32 = 28.0;
const STATUS_BAR_RIGHT_WIDTH: f32 = 132.0;
const STATUS_BAR_LEFT_WIDTH: f32 = 520.0;
const STATUS_BAR_THEME_WIDTH: f32 = 132.0;
const STATUS_BAR_SECTION_GAP: f32 = 14.0;

impl ForgeApp {
    pub fn loader_mode_label(&self) -> &'static str {
        match self.state.loader.mode {
            LoaderMode::Initialize => "Inicializar vault",
            LoaderMode::Unlock => "Desbloquear vault",
        }
    }

    pub fn admin_requirement_label(&self) -> &'static str {
        "Privilegios elevados: no requeridos para este alcance"
    }

    pub fn theme_preference(&self) -> theme::ThemePreference {
        self.state.theme_preference
    }

    pub fn selected_token(&self) -> Option<&TotpToken> {
        self.state.token_dialog.token.as_ref()
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    pub fn set_banner(&mut self, tone: BannerTone, message: impl Into<String>) {
        self.state.banner = Some(Banner {
            tone,
            message: message.into(),
        });
    }

    pub fn set_theme_preference(
        &mut self,
        ctx: &egui::Context,
        preference: theme::ThemePreference,
    ) {
        if self.state.theme_preference == preference {
            return;
        }

        self.state.theme_preference = preference;
        theme::apply(ctx, preference);

        if let Err(error) = theme::save_preference(preference) {
            self.set_banner(
                BannerTone::Warning,
                format!("El tema se aplicó, pero no se pudo guardar la preferencia: {error}"),
            );
        }
    }

    pub(crate) fn status_bar_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let palette = theme::palette(self.state.theme_preference);
        let current_theme = self.state.theme_preference;
        let mut selected_theme = current_theme;
        let visible_accounts = self.visible_accounts().len();
        let total_accounts = self.total_accounts();
        let workspace_label = self.state.workspace_scope.label();
        let banner = status_banner(&self.state.banner, self.state.screen, palette);

        ui.set_height(STATUS_BAR_HEIGHT);

        egui::Frame::none()
            .fill(palette.status_fill)
            .stroke(egui::Stroke::new(1.0, palette.status_stroke))
            .inner_margin(egui::Margin::symmetric(16.0, 8.0))
            .show(ui, |ui| {
                let content_width = ui.available_width();
                let left_width = STATUS_BAR_LEFT_WIDTH.min(
                    (content_width - STATUS_BAR_RIGHT_WIDTH - (STATUS_BAR_SECTION_GAP * 2.0))
                        .max(0.0),
                );
                let center_width = (content_width
                    - left_width
                    - STATUS_BAR_RIGHT_WIDTH
                    - (STATUS_BAR_SECTION_GAP * 2.0))
                    .max(0.0);

                ui.set_min_size(egui::vec2(content_width, STATUS_BAR_HEIGHT));
                ui.allocate_ui_with_layout(
                    egui::vec2(content_width, STATUS_BAR_ROW_HEIGHT),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                        ui.allocate_ui_with_layout(
                            egui::vec2(left_width, STATUS_BAR_ROW_HEIGHT),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
                                    status_item(
                                        ui,
                                        "Vault",
                                        self.vault_status_label(),
                                        palette.status_text,
                                    );
                                    status_separator(ui, palette.status_stroke);
                                    status_item(
                                        ui,
                                        "Cuentas",
                                        &total_accounts.to_string(),
                                        palette.status_text,
                                    );
                                    status_separator(ui, palette.status_stroke);
                                    status_item(
                                        ui,
                                        "Visibles",
                                        &visible_accounts.to_string(),
                                        palette.status_text,
                                    );
                                    status_separator(ui, palette.status_stroke);
                                    status_item(ui, "Ámbito", workspace_label, palette.status_text);
                                });
                            },
                        );

                        ui.add_space(STATUS_BAR_SECTION_GAP);
                        ui.allocate_ui_with_layout(
                            egui::vec2(center_width, STATUS_BAR_ROW_HEIGHT),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                if let Some((message, color)) = banner.as_ref() {
                                    ui.add(status_message_label(message, *color, true));
                                }
                            },
                        );

                        ui.add_space(STATUS_BAR_SECTION_GAP);

                        let remaining = ui.available_width();
                        ui.allocate_ui_with_layout(
                            egui::vec2(remaining, STATUS_BAR_ROW_HEIGHT),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.spacing_mut().interact_size.y = STATUS_BAR_ROW_HEIGHT;
                                theme_selector_ui(ui, &mut selected_theme);
                            },
                        );
                    },
                );
            });

        if selected_theme != current_theme {
            self.set_theme_preference(&ctx, selected_theme);
        }
    }

    pub(crate) fn sync_token_dialog(&mut self) {
        if !self.state.token_dialog.open {
            return;
        }

        let Some(account) = self.selected_account() else {
            self.state.token_dialog.close();
            return;
        };

        let now = unix_timestamp_now();

        if self.state.token_dialog.last_visible_second == Some(now) {
            return;
        }

        self.state.token_dialog.last_visible_second = Some(now);

        let should_refresh = match self.state.token_dialog.token.as_mut() {
            Some(token) if token.account_id != account.id => true,
            Some(token) => {
                token.seconds_remaining = token.expires_at.saturating_sub(now);
                now >= token.expires_at
            }
            None => true,
        };

        if should_refresh && self.pending_token_job.is_none() {
            self.state.token_dialog.pending = true;
            self.state.token_dialog.action_message =
                Some("Actualizando la ventana TOTP...".to_owned());
            self.state.token_dialog.action_tone = Some(BannerTone::Info);
            self.request_token_for(account, self.state.token_dialog.token.clone());
        }
    }
}

fn unix_timestamp_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn status_message_label(message: &str, text_color: egui::Color32, truncate: bool) -> egui::Label {
    let label = egui::Label::new(RichText::new(message).size(13.5).color(text_color));

    if truncate {
        label.truncate()
    } else {
        label.wrap()
    }
}

fn status_item(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32) {
    ui.label(
        RichText::new(format!("{label}:"))
            .size(13.5)
            .strong()
            .color(color),
    );
    ui.label(RichText::new(value).size(13.5).color(color));
}

fn status_separator(ui: &mut egui::Ui, color: egui::Color32) {
    ui.label(RichText::new("|").size(13.5).strong().color(color));
}

fn theme_selector_ui(ui: &mut egui::Ui, selected_theme: &mut theme::ThemePreference) {
    egui::ComboBox::from_id_salt("status_bar_theme_selector")
        .selected_text(selected_theme.label())
        .width(STATUS_BAR_THEME_WIDTH)
        .show_ui(ui, |ui| {
            ui.selectable_value(
                selected_theme,
                theme::ThemePreference::Dark,
                theme::ThemePreference::Dark.label(),
            );
            ui.selectable_value(
                selected_theme,
                theme::ThemePreference::Light,
                theme::ThemePreference::Light.label(),
            );
        });
}

fn status_banner(
    banner: &Option<Banner>,
    screen: Screen,
    palette: theme::ThemePalette,
) -> Option<(String, egui::Color32)> {
    match banner {
        Some(banner) => {
            let color = match banner.tone {
                BannerTone::Info => palette.info_text,
                BannerTone::Success => palette.success_text,
                BannerTone::Warning => palette.warning_text,
                BannerTone::Error => palette.error_text,
            };
            Some((banner.message.clone(), color))
        }
        None => match screen {
            Screen::Loader => Some((
                "Esperando inicialización o desbloqueo del vault.".to_owned(),
                palette.status_idle_text,
            )),
            Screen::Main => None,
        },
    }
}

pub(crate) fn request_open_windows_repaint(ctx: &egui::Context) {
    ctx.request_repaint_after(Duration::from_secs(1));
}
