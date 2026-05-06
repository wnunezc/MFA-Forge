use egui_commonmark::CommonMarkViewer;

use crate::{
    app::ForgeApp,
    i18n::{Language, tr, trf},
    theme,
};

const HELP_EN: &str = include_str!("../../../docs/help/user-guide.en.md");
const HELP_ES: &str = include_str!("../../../docs/help/user-guide.es.md");
const HELP_FR: &str = include_str!("../../../docs/help/user-guide.fr.md");
const HELP_HI: &str = include_str!("../../../docs/help/user-guide.hi.md");
const HELP_ZH: &str = include_str!("../../../docs/help/user-guide.zh.md");

struct HelpSection<'a> {
    title: &'a str,
    markdown: &'a str,
}

fn help_markdown(language: Language) -> &'static str {
    match language {
        Language::Es => HELP_ES,
        Language::Fr => HELP_FR,
        Language::Hi => HELP_HI,
        Language::Zh => HELP_ZH,
        Language::En => HELP_EN,
    }
}

pub fn render(ctx: &egui::Context, app: &mut ForgeApp) {
    if !app.state().help_dialog.open {
        return;
    }

    let palette = theme::palette(app.theme_preference());
    let mut open = app.state().help_dialog.open;
    let language = app.language();
    let sections = parse_sections(help_markdown(language));
    let search_query = app.state().help_dialog.search_query.to_lowercase();
    let filtered = sections
        .iter()
        .enumerate()
        .filter(|(_, section)| {
            search_query.is_empty()
                || section.title.to_lowercase().contains(&search_query)
                || section.markdown.to_lowercase().contains(&search_query)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    let selected_section = app
        .state()
        .help_dialog
        .selected_section
        .filter(|index| filtered.contains(index))
        .or_else(|| filtered.first().copied());

    app.state_mut().help_dialog.selected_section = selected_section;

    egui::Window::new(format!("{} - MFA-Forge", tr("App help title")))
        .open(&mut open)
        .default_size([980.0, 680.0])
        .min_size([860.0, 560.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Search"));
                ui.add(
                    egui::TextEdit::singleline(&mut app.state_mut().help_dialog.search_query)
                        .desired_width(280.0)
                        .hint_text(tr("Help search hint")),
                );
                if ui.button(tr("Clear")).clicked() {
                    app.state_mut().help_dialog.search_query.clear();
                }
            });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(trf(
                    "App help sections found: {count}",
                    &[("count", &filtered.len().to_string())],
                ))
                .small()
                .color(palette.secondary_text),
            );
            ui.separator();

            ui.columns(2, |columns| {
                columns[0].vertical(|ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("help_sections_list")
                        .show(ui, |ui| {
                            if filtered.is_empty() {
                                ui.label(
                                    egui::RichText::new(tr("No help sections match this search."))
                                        .small()
                                        .color(palette.secondary_text),
                                );
                                return;
                            }

                            for index in &filtered {
                                let selected = selected_section == Some(*index);
                                if ui
                                    .selectable_label(selected, sections[*index].title)
                                    .clicked()
                                {
                                    app.state_mut().help_dialog.selected_section = Some(*index);
                                }
                            }
                        });
                });

                columns[1].vertical(|ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("help_section_content")
                        .show(ui, |ui| {
                            if let Some(index) = app.state().help_dialog.selected_section {
                                CommonMarkViewer::new().show(
                                    ui,
                                    &mut app.help_markdown_cache,
                                    sections[index].markdown,
                                );
                            }
                        });
                });
            });
        });

    if !open {
        app.state_mut().help_dialog.close();
    }
}

fn parse_sections(markdown: &'static str) -> Vec<HelpSection<'static>> {
    let mut headings = Vec::new();
    let mut offset = 0;

    for raw_line in markdown.split_inclusive('\n') {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        if let Some(title) = line.strip_prefix("## ") {
            headings.push((offset, offset + raw_line.len(), title.trim()));
        }
        offset += raw_line.len();
    }

    headings
        .iter()
        .enumerate()
        .map(|(index, (_, body_start, title))| {
            let body_end = headings
                .get(index + 1)
                .map(|(heading_start, _, _)| *heading_start)
                .unwrap_or(markdown.len());
            HelpSection {
                title,
                markdown: markdown[*body_start..body_end].trim(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_guides_have_sections() {
        for guide in [HELP_EN, HELP_ES, HELP_FR, HELP_HI, HELP_ZH] {
            let sections = parse_sections(guide);
            assert!(sections.len() >= 6);
            assert!(sections.iter().all(|section| {
                !section.title.trim().is_empty() && !section.markdown.trim().is_empty()
            }));
        }
    }
}
