use std::{fs, path::PathBuf};

use directories::ProjectDirs;
use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle, Visuals};
use serde::{Deserialize, Serialize};

use crate::i18n::Language;

const THEME_PREFERENCES_FILE: &str = "gui-preferences.json";

/// Tema visual persistido para la GUI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    Light,
    #[default]
    Dark,
}

impl ThemePreference {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

/// Paleta semántica compartida por las vistas de la aplicación.
#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub brand_accent: Color32,
    pub strong_text: Color32,
    pub secondary_text: Color32,
    pub muted_text: Color32,
    pub info_text: Color32,
    pub success_text: Color32,
    pub warning_text: Color32,
    pub error_text: Color32,
    pub detail_label: Color32,
    pub detail_value: Color32,
    pub surface_fill: Color32,
    pub surface_stroke: Color32,
    pub status_fill: Color32,
    pub status_stroke: Color32,
    pub status_text: Color32,
    pub status_idle_text: Color32,
    pub token_text: Color32,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredGuiPreferences {
    #[serde(default)]
    theme: ThemePreference,
    #[serde(default)]
    language: Language,
}

#[derive(Debug, Clone, Copy)]
pub struct GuiPreferences {
    pub theme: ThemePreference,
    pub language: Language,
}

/// Aplica spacing, tipografías y visuales del tema activo.
pub fn apply(ctx: &egui::Context, preference: ThemePreference) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(9.0, 5.0);
    style.spacing.window_margin = egui::Margin::same(12.0);
    style.visuals = visuals(preference);
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(24.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(14.0, FontFamily::Monospace),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
    ]
    .into();

    ctx.set_style(style);
}

/// Instala fallbacks tipograficos de Windows para Latin, Devanagari y CJK.
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    add_windows_font_if_exists(&mut fonts, "windows_ui", r"C:\Windows\Fonts\segoeui.ttf", 0);
    add_windows_font_if_exists(&mut fonts, "windows_cjk", r"C:\Windows\Fonts\msyh.ttc", 0);
    add_windows_font_if_exists(
        &mut fonts,
        "windows_indic",
        r"C:\Windows\Fonts\Nirmala.ttc",
        0,
    );

    insert_if_present(&mut fonts, FontFamily::Proportional, "windows_ui");
    push_if_present(&mut fonts, FontFamily::Proportional, "windows_cjk");
    push_if_present(&mut fonts, FontFamily::Proportional, "windows_indic");
    push_if_present(&mut fonts, FontFamily::Monospace, "windows_cjk");
    push_if_present(&mut fonts, FontFamily::Monospace, "windows_indic");

    ctx.set_fonts(fonts);
}

/// Devuelve la preferencia guardada; si no existe o está dañada, usa oscuro.
pub fn load_preference() -> ThemePreference {
    load_preferences().theme
}

pub fn load_language_preference() -> Language {
    load_preferences().language
}

pub fn load_preferences() -> GuiPreferences {
    let Ok(path) = theme_preference_path() else {
        return GuiPreferences {
            theme: ThemePreference::default(),
            language: Language::default(),
        };
    };

    let Ok(bytes) = fs::read(path) else {
        return GuiPreferences {
            theme: ThemePreference::default(),
            language: Language::default(),
        };
    };

    serde_json::from_slice::<StoredGuiPreferences>(&bytes)
        .map(|stored| GuiPreferences {
            theme: stored.theme,
            language: stored.language,
        })
        .unwrap_or(GuiPreferences {
            theme: ThemePreference::default(),
            language: Language::default(),
        })
}

/// Guarda inmediatamente la preferencia de tema para restaurarla al reiniciar.
pub fn save_preference(preference: ThemePreference) -> Result<(), String> {
    let mut stored = load_preferences();
    stored.theme = preference;
    save_preferences(stored)
}

pub fn save_language_preference(language: Language) -> Result<(), String> {
    let mut stored = load_preferences();
    stored.language = language;
    save_preferences(stored)
}

fn save_preferences(preferences: GuiPreferences) -> Result<(), String> {
    let path = theme_preference_path()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!("The GUI preferences directory could not be created: {error}")
        })?;
    }

    let payload = serde_json::to_vec_pretty(&StoredGuiPreferences {
        theme: preferences.theme,
        language: preferences.language,
    })
    .map_err(|error| format!("The GUI preferences could not be serialized: {error}"))?;

    fs::write(path, payload)
        .map_err(|error| format!("The GUI preferences could not be saved: {error}"))
}

/// Expone la paleta semántica usada por la UI.
pub fn palette(preference: ThemePreference) -> ThemePalette {
    match preference {
        ThemePreference::Dark => ThemePalette {
            brand_accent: Color32::from_rgb(124, 170, 225),
            strong_text: Color32::from_rgb(245, 240, 231),
            secondary_text: Color32::from_rgb(204, 209, 217),
            muted_text: Color32::from_rgb(178, 185, 196),
            info_text: Color32::from_rgb(181, 208, 255),
            success_text: Color32::from_rgb(169, 233, 186),
            warning_text: Color32::from_rgb(244, 190, 102),
            error_text: Color32::from_rgb(229, 90, 90),
            detail_label: Color32::from_rgb(196, 201, 210),
            detail_value: Color32::from_rgb(239, 241, 244),
            surface_fill: Color32::from_rgb(24, 27, 34),
            surface_stroke: Color32::from_rgb(66, 74, 89),
            status_fill: Color32::from_rgb(20, 23, 29),
            status_stroke: Color32::from_rgb(56, 63, 77),
            status_text: Color32::from_rgb(205, 210, 218),
            status_idle_text: Color32::from_rgb(189, 196, 206),
            token_text: Color32::from_rgb(250, 244, 236),
        },
        ThemePreference::Light => ThemePalette {
            brand_accent: Color32::from_rgb(58, 95, 149),
            strong_text: Color32::from_rgb(44, 50, 58),
            secondary_text: Color32::from_rgb(78, 86, 98),
            muted_text: Color32::from_rgb(101, 109, 122),
            info_text: Color32::from_rgb(55, 93, 146),
            success_text: Color32::from_rgb(57, 112, 79),
            warning_text: Color32::from_rgb(147, 103, 40),
            error_text: Color32::from_rgb(171, 68, 68),
            detail_label: Color32::from_rgb(88, 95, 106),
            detail_value: Color32::from_rgb(44, 50, 58),
            surface_fill: Color32::from_rgb(250, 251, 253),
            surface_stroke: Color32::from_rgb(170, 176, 186),
            status_fill: Color32::from_rgb(229, 233, 239),
            status_stroke: Color32::from_rgb(181, 188, 198),
            status_text: Color32::from_rgb(76, 82, 92),
            status_idle_text: Color32::from_rgb(92, 98, 108),
            token_text: Color32::from_rgb(58, 95, 149),
        },
    }
}

fn add_windows_font_if_exists(
    fonts: &mut egui::FontDefinitions,
    name: &str,
    path: &str,
    index: u32,
) {
    if let Ok(bytes) = fs::read(path) {
        fonts.font_data.insert(
            name.to_owned(),
            egui::FontData {
                font: bytes.into(),
                index,
                tweak: Default::default(),
            },
        );
    }
}

fn push_if_present(fonts: &mut egui::FontDefinitions, family: FontFamily, name: &str) {
    if fonts.font_data.contains_key(name) {
        fonts
            .families
            .entry(family)
            .or_default()
            .push(name.to_owned());
    }
}

fn insert_if_present(fonts: &mut egui::FontDefinitions, family: FontFamily, name: &str) {
    if fonts.font_data.contains_key(name) {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.to_owned());
    }
}

fn theme_preference_path() -> Result<PathBuf, String> {
    let project_dirs = ProjectDirs::from("dev", "OpsZone", "MFA-Forge")
        .ok_or_else(|| "The MFA-Forge local data directory could not be resolved.".to_owned())?;
    Ok(project_dirs.data_local_dir().join(THEME_PREFERENCES_FILE))
}

fn visuals(preference: ThemePreference) -> Visuals {
    match preference {
        ThemePreference::Dark => dark_visuals(),
        ThemePreference::Light => light_visuals(),
    }
}

fn dark_visuals() -> Visuals {
    let mut visuals = Visuals::dark();

    visuals.panel_fill = Color32::from_rgb(20, 22, 26);
    visuals.window_fill = Color32::from_rgb(24, 26, 30);
    visuals.extreme_bg_color = Color32::from_rgb(14, 16, 20);
    visuals.faint_bg_color = Color32::from_rgb(28, 30, 35);
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(62, 66, 76));
    visuals.selection.bg_fill = Color32::from_rgb(79, 129, 189).linear_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(79, 129, 189));
    visuals.hyperlink_color = Color32::from_rgb(124, 170, 225);
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(20, 22, 26);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(56, 60, 70));
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(220, 224, 232));
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(33, 36, 42);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(33, 36, 42);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(70, 74, 84));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(236, 239, 244));
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(42, 46, 54);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(42, 46, 54);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(104, 133, 173));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.active.bg_fill = Color32::from_rgb(48, 58, 72);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(48, 58, 72);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(110, 145, 192));
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.open.bg_fill = Color32::from_rgb(38, 41, 48);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, Color32::from_rgb(98, 126, 165));
    visuals.warn_fg_color = Color32::from_rgb(248, 193, 69);
    visuals.error_fg_color = Color32::from_rgb(229, 90, 90);
    visuals.override_text_color = Some(Color32::from_rgb(240, 242, 245));

    visuals
}

fn light_visuals() -> Visuals {
    let mut visuals = Visuals::light();

    visuals.panel_fill = Color32::from_rgb(238, 241, 245);
    visuals.window_fill = Color32::from_rgb(246, 247, 249);
    visuals.extreme_bg_color = Color32::from_rgb(223, 227, 233);
    visuals.faint_bg_color = Color32::from_rgb(232, 235, 240);
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(170, 176, 186));
    visuals.selection.bg_fill = Color32::from_rgb(76, 118, 172).linear_multiply(0.22);
    visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(76, 118, 172));
    visuals.hyperlink_color = Color32::from_rgb(58, 95, 149);
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(238, 241, 245);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(182, 188, 197));
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(76, 82, 92));
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(245, 247, 250);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(245, 247, 250);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(170, 176, 186));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(58, 64, 72));
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(233, 238, 244);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(233, 238, 244);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(112, 133, 163));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::from_rgb(44, 50, 58));
    visuals.widgets.active.bg_fill = Color32::from_rgb(220, 228, 238);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(220, 228, 238);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(92, 116, 149));
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::from_rgb(34, 40, 47));
    visuals.widgets.open.bg_fill = Color32::from_rgb(228, 233, 240);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, Color32::from_rgb(104, 124, 154));
    visuals.warn_fg_color = Color32::from_rgb(147, 103, 40);
    visuals.error_fg_color = Color32::from_rgb(171, 68, 68);
    visuals.override_text_color = Some(Color32::from_rgb(50, 54, 62));

    visuals
}
