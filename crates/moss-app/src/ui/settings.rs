use super::{menu, phosphor, theme::Theme};
use crate::{
    emoji_catalog,
    settings::{SettingsDropdown, SettingsSection, agent_setup_text, version_string},
    state::State,
};
use rune::*;

const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

pub fn overlay(s: &State, t: Theme) -> El {
    let (win_w, win_h) = rune::memo::layout_with("root", |r| (r[2], r[3])).unwrap_or((1200., 800.));
    let modal_w = (win_w - 72.).clamp(700., 1060.);
    let modal_h = (win_h - 72.).clamp(480., 700.);
    let compact = modal_w < 840.;
    let sidebar_w = if compact { 212. } else { 236. };
    let content_px = if compact { 28. } else { 48. };

    let scrim_alpha = if t.dark { 0.48 } else { 0.30 };
    let shadow_alpha = if t.dark { 0.42 } else { 0.20 };

    let body = match s.settings.section {
        SettingsSection::Preferences => preferences_page(s, t),
        SettingsSection::Connections => connections_page(s, t),
        SettingsSection::General => general_page(s, t),
        SettingsSection::ImportExport => import_export_page(s, t),
    };

    let dialog = el()
        .key("modal")
        .w(modal_w)
        .h(modal_h)
        .fill(t.bg)
        .radius(12.)
        .border(1., t.line)
        .shadow(14., 44., Color::BLACK.alpha(shadow_alpha))
        .role("dialog")
        .label("Settings")
        .modal(true, |s: &mut State| s.close_settings())
        .transition(Transition::opacity())
        .child(
            row()
                .w_full()
                .h_full()
                .instant_changes()
                .child(sidebar(s, sidebar_w, t))
                .child(el().w(1.).h_full().fill(t.line))
                .child(
                    el().key("settings-content")
                        .grow(1.)
                        .min_w(0.)
                        .h_full()
                        .child(
                            col()
                                .key("settings-scroll")
                                .w_full()
                                .h_full()
                                .my(12.)
                                .px(content_px)
                                .pt(20.)
                                .pb(32.)
                                .scroll(ScrollSpec {
                                    stick_bottom: false,
                                })
                                .children(s.error.as_ref().map(|err| error_banner(err, t)))
                                .child(body),
                        )
                        .child(close_button(t)),
                ),
        )
        .children(active_dropdown(s, t).map(El::raise));

    el().key("modal-scrim")
        .pin(Some(0.), Some(0.), Some(0.), Some(0.))
        .fill(Color::BLACK.alpha(scrim_alpha))
        .center()
        .child(dialog)
}

fn close_button(t: Theme) -> El {
    row()
        .key("close-modal")
        .role("button")
        .label("Close")
        .focusable(true)
        .size(28., 28.)
        .center()
        .radius(6.)
        .hover(t.hover)
        .press(t.selected)
        .pin(None, Some(12.), Some(12.), None)
        .child(rune::icon(&phosphor::X, 15.).color(t.muted))
        .on_press(|s: &mut State| s.close_settings())
}

fn error_banner(message: &str, t: Theme) -> El {
    row()
        .key("settings-error-banner")
        .w_full()
        .mb(16.)
        .px(12.)
        .py(9.)
        .gap(8.)
        .center_y()
        .radius(6.)
        .fill(t.error.alpha(if t.dark { 0.16 } else { 0.08 }))
        .border(1., t.error.alpha(0.35))
        .child(
            text(message)
                .font(TextStyle::new(13., 400.).family(Family::Sans).lh(18.))
                .color(t.error)
                .grow(1.)
                .min_w(0.)
                .wrap(),
        )
        .child(
            row()
                .key("settings-dismiss-error")
                .role("button")
                .label("Dismiss error")
                .focusable(true)
                .size(22., 22.)
                .center()
                .radius(4.)
                .hover(t.error.alpha(0.14))
                .child(rune::icon(&phosphor::X, 12.).color(t.error))
                .on_press_tx(|s: &mut State, tx| {
                    tx.instant();
                    s.error = None;
                }),
        )
}

fn sidebar(s: &State, width: f32, t: Theme) -> El {
    let workspace_label = if s.workspace.name.trim().is_empty() {
        "Workspace".to_owned()
    } else {
        s.workspace.name.clone()
    };

    col()
        .key("settings-sidebar")
        .w(width)
        .min_w(width)
        .h_full()
        .fill(t.sidebar)
        .corners([12., 0., 0., 12.])
        .px(8.)
        .pt(14.)
        .pb(12.)
        .gap(2.)
        .child(sidebar_group_heading("Moss", t))
        .child(sidebar_nav_item(
            "settings-nav-preferences",
            "Preferences",
            &phosphor::SLIDERS_HORIZONTAL,
            SettingsSection::Preferences,
            s.settings.section,
            t,
        ))
        .child(sidebar_nav_item(
            "settings-nav-connections",
            "Connections",
            &phosphor::LINK,
            SettingsSection::Connections,
            s.settings.section,
            t,
        ))
        .child(el().h(16.))
        .child(
            row()
                .key("settings-sidebar-workspace")
                .w_full()
                .px(10.)
                .pt(4.)
                .pb(6.)
                .gap(6.)
                .center_y()
                .child(workspace_badge(&s.workspace.icon, &workspace_label, 16., t))
                .child(
                    text(&workspace_label)
                        .font(TextStyle::new(11.5, 600.).family(Family::Sans).lh(16.))
                        .color(t.muted)
                        .grow(1.)
                        .min_w(0.)
                        .truncate(),
                ),
        )
        .child(sidebar_nav_item(
            "settings-nav-general",
            "General",
            &phosphor::GEAR_SIX,
            SettingsSection::General,
            s.settings.section,
            t,
        ))
        .child(sidebar_nav_item(
            "settings-nav-import-export",
            "Import & export",
            &phosphor::ARROWS_LEFT_RIGHT,
            SettingsSection::ImportExport,
            s.settings.section,
            t,
        ))
        .child(el().grow(1.))
        .child(
            col()
                .w_full()
                .px(10.)
                .pt(8.)
                .gap(4.)
                .child(el().h(1.).w_full().fill(t.line).mb(6.))
                .child(
                    text(version_string())
                        .key("settings-version-footer")
                        .font(TextStyle::new(11.5, 400.).family(Family::Sans).lh(16.))
                        .color(t.faint)
                        .truncate(),
                ),
        )
}

fn sidebar_group_heading(title: &str, t: Theme) -> El {
    row().w_full().px(10.).pt(4.).pb(5.).child(
        text(title)
            .font(TextStyle::new(11.5, 600.).family(Family::Sans).lh(16.))
            .color(t.muted),
    )
}

fn sidebar_nav_item(
    key: &'static str,
    title: &'static str,
    glyph: &'static Icon,
    target: SettingsSection,
    active: SettingsSection,
    t: Theme,
) -> El {
    let selected = target == active;
    row()
        .key(key)
        .w_full()
        .h(30.)
        .px(10.)
        .gap(9.)
        .center_y()
        .radius(6.)
        .fill(if selected {
            t.selected
        } else {
            Color::TRANSPARENT
        })
        .hover(if selected { t.selected } else { t.hover })
        .press(t.selected)
        .role("tab")
        .label(title)
        .checked(selected)
        .focusable(true)
        .child(
            el().size(18., 18.)
                .center()
                .child(rune::icon(glyph, 16.).color(if selected { t.text } else { t.muted })),
        )
        .child(
            text(title)
                .font(
                    TextStyle::new(13.5, if selected { 500. } else { 400. })
                        .family(Family::Sans)
                        .lh(18.),
                )
                .color(t.text)
                .grow(1.)
                .min_w(0.)
                .truncate(),
        )
        .on_press_tx(move |s: &mut State, tx| {
            tx.instant();
            s.select_settings_section(target);
        })
}

fn workspace_badge(icon: &str, name: &str, size: f32, t: Theme) -> El {
    let glyph = if icon.trim().is_empty() {
        name.chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "M".into())
    } else {
        icon.to_owned()
    };
    let font_size = (size * 0.62).clamp(11., 24.);
    el().size(size, size)
        .center()
        .radius((size * 0.22).clamp(4., 8.))
        .fill(if t.dark {
            color::hex("#2C2C2C").unwrap()
        } else {
            color::hex("#EAEAE6").unwrap()
        })
        .border(1., t.line)
        .child(
            text(glyph)
                .font(TextStyle::new(font_size, 600.).family(Family::Sans))
                .color(t.text),
        )
}

pub(crate) fn section_header(title: &str, top_gap: bool, t: Theme) -> El {
    col()
        .w_full()
        .pt(if top_gap { 28. } else { 0. })
        .pb(6.)
        .gap(10.)
        .child(
            text(title)
                .font(TextStyle::new(16., 600.).family(Family::Sans).lh(22.))
                .color(t.text),
        )
        .child(el().h(1.).w_full().fill(t.line))
}

pub(crate) fn setting_row(title: &str, description: El, control: El, t: Theme) -> El {
    row()
        .w_full()
        .py(11.)
        .gap(24.)
        .center_y()
        .child(
            col()
                .grow(1.)
                .min_w(0.)
                .gap(3.)
                .child(
                    text(title)
                        .font(TextStyle::new(14., 400.).family(Family::Sans).lh(20.))
                        .color(t.text),
                )
                .child(description),
        )
        .child(control)
}

pub(crate) fn description_text(copy: impl Into<String>, t: Theme) -> El {
    text(copy)
        .font(TextStyle::new(12.5, 400.).family(Family::Sans).lh(17.))
        .color(t.muted)
        .wrap()
}

pub(crate) fn toggle_switch(key: &'static str, label: &'static str, checked: bool, t: Theme) -> El {
    let track = if checked {
        t.blue
    } else if t.dark {
        color::hex("#4A4A4A").unwrap()
    } else {
        color::hex("#D3D3CE").unwrap()
    };
    el().key(key)
        .size(30., 18.)
        .radius(9.)
        .role("switch")
        .label(label)
        .checked(checked)
        .focusable(true)
        .fill(track)
        .child(
            el().size(14., 14.)
                .radius(7.)
                .fill(Color::WHITE)
                .absolute(if checked { 14. } else { 2. }, 2.),
        )
}

fn dropdown_trigger(
    key: &'static str,
    label: &'static str,
    value: &str,
    open: bool,
    t: Theme,
) -> El {
    row()
        .key(key)
        .role("button")
        .label(format!("{label}: {value}"))
        .expanded(open)
        .focusable(true)
        .h(30.)
        .px(10.)
        .mr(-10.)
        .gap(6.)
        .center_y()
        .radius(6.)
        .fill(if open { t.hover } else { Color::TRANSPARENT })
        .hover(t.hover)
        .press(t.selected)
        .child(
            text(value)
                .font(TextStyle::new(13.5, 400.).family(Family::Sans).lh(18.))
                .color(t.text),
        )
        .child(rune::icon(&phosphor::CARET_DOWN, 12.).color(t.muted))
}

pub(crate) fn action_button(key: &'static str, title: &str, primary: bool, t: Theme) -> El {
    action_button_with_icon(key, title, None, primary, t)
}

fn action_button_with_icon(
    key: &'static str,
    title: &str,
    glyph: Option<&'static Icon>,
    primary: bool,
    t: Theme,
) -> El {
    let (fill, ink) = if primary {
        if t.dark {
            (
                color::hex("#4799E9").unwrap(),
                color::hex("#121212").unwrap(),
            )
        } else {
            (t.blue, Color::WHITE)
        }
    } else {
        (t.bg, t.text)
    };
    row()
        .key(key)
        .role("button")
        .label(title)
        .focusable(true)
        .h(30.)
        .px(12.)
        .gap(6.)
        .center()
        .radius(6.)
        .border(1., if primary { fill } else { t.line })
        .fill(fill)
        .hover(if primary {
            fill.mix(ink, 0.12)
        } else {
            t.hover
        })
        .press(if primary {
            fill.mix(ink, 0.2)
        } else {
            t.selected
        })
        .children(glyph.map(|icon| {
            rune::icon(icon, 14.).color(if primary { ink } else { t.muted })
        }))
        .child(
            text(title)
                .font(TextStyle::new(13., 500.).family(Family::Sans).lh(18.))
                .color(ink),
        )
}

fn preferences_page(s: &State, t: Theme) -> El {
    let appearance_label = match s.prefs.dark {
        None => "Use system setting",
        Some(false) => "Light",
        Some(true) => "Dark",
    };
    let week_day = WEEKDAYS
        .get(usize::from(s.prefs.week_start()))
        .copied()
        .unwrap_or("Sunday");

    let desktop_section = match super::software_update::settings(s, t) {
        Some(update_rows) => col()
            .w_full()
            .child(section_header("Desktop app", true, t))
            .child(update_rows),
        None => col()
            .w_full()
            .child(section_header("Desktop app", true, t))
            .child(setting_row(
                "Version",
                description_text(
                    format!(
                        "Moss {}{}",
                        option_env!("MOSS_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
                        if cfg!(debug_assertions) {
                            " · Local development build"
                        } else {
                            ""
                        }
                    ),
                    t,
                ),
                el(),
                t,
            )),
    };

    col()
        .key("settings-page-preferences")
        .w_full()
        .child(section_header("My settings", false, t))
        .child(setting_row(
            "Appearance",
            description_text("Customize how Moss looks on your device.", t),
            dropdown_trigger(
                "settings-appearance-trigger",
                "Appearance",
                appearance_label,
                s.settings.dropdown == Some(SettingsDropdown::Appearance),
                t,
            )
            .on_press_tx(|s: &mut State, tx| {
                tx.instant();
                s.toggle_settings_dropdown(SettingsDropdown::Appearance);
            }),
            t,
        ))
        .child(section_header("Date & time", true, t))
        .child(setting_row(
            "Start week on",
            description_text(
                "Set the first day of the week in calendars and date pickers.",
                t,
            ),
            dropdown_trigger(
                "settings-week-start-trigger",
                "Start week on",
                week_day,
                s.settings.dropdown == Some(SettingsDropdown::WeekStart),
                t,
            )
            .on_press_tx(|s: &mut State, tx| {
                tx.instant();
                s.toggle_settings_dropdown(SettingsDropdown::WeekStart);
            }),
            t,
        ))
        .child(desktop_section)
}

fn connections_page(s: &State, t: Theme) -> El {
    let codex_fallback;
    let codex_preview = match s.settings.codex_preview.as_ref() {
        Some(cached) => cached,
        None => {
            codex_fallback = agent_setup_text(&s.path, true);
            &codex_fallback
        }
    };
    let json_fallback;
    let json_preview = match s.settings.json_preview.as_ref() {
        Some(cached) => cached,
        None => {
            json_fallback = agent_setup_text(&s.path, false);
            &json_fallback
        }
    };

    col()
        .key("settings-page-connections")
        .w_full()
        .child(section_header("Local agent access", false, t))
        .child(setting_row(
            "Allow local agents to connect",
            description_text(
                "Allow local MCP clients on this Mac to read and change workspace data while Moss is open.",
                t,
            ),
            toggle_switch(
                "mcp-enabled",
                "Local agent access",
                s.prefs.mcp_enabled,
                t,
            )
            .on_press_tx(|s: &mut State, tx| {
                tx.instant();
                s.set_mcp_enabled(!s.prefs.mcp_enabled);
            }),
            t,
        ))
        .child(section_header("Connect an agent", true, t))
        .child(
            col()
                .w_full()
                .pt(8.)
                .gap(12.)
                .child(description_text(
                    "Add Moss to an MCP client on this Mac, then start a new session and ask it to list your Moss workspaces.",
                    t,
                ))
                .child(connection_card(
                    "Codex CLI",
                    "Paste this command into Terminal to register Moss.",
                    codex_preview.as_deref(),
                    "agent-setup-copy-codex",
                    "Copy command",
                    true,
                    t,
                ))
                .child(connection_card(
                    "MCP client JSON",
                    "Paste into a client that uses mcpServers JSON configuration.",
                    json_preview.as_deref(),
                    "agent-setup-copy-json",
                    "Copy JSON",
                    false,
                    t,
                ))
                .child(s.settings.agent_feedback.map_or_else(
                    || el().h(18.).inert().a11y_hidden(),
                    |message| {
                        text(message)
                            .key("agent-setup-feedback")
                            .font(TextStyle::new(12.5, 500.).family(Family::Sans).lh(18.))
                            .color(if message == "Copied to the clipboard." {
                                t.blue
                            } else {
                                t.error
                            })
                            .min_h(18.)
                            .live_region(LiveRegion::Polite)
                            .wrap()
                    },
                ))
                .child(description_text(
                    "If the client cannot connect, check that Moss is running and local agent access is turned on. Then restart the client.",
                    t,
                )),
        )
}

fn connection_card(
    title: &str,
    subtitle: &str,
    snippet: Result<&str, &&'static str>,
    button_key: &'static str,
    button_title: &str,
    codex: bool,
    t: Theme,
) -> El {
    let preview = match snippet {
        Ok(code) => el()
            .key(format!("{button_key}-code-box"))
            .w_full()
            .min_w(0.)
            .radius(6.)
            .fill(t.bg)
            .border(1., t.line)
            .child(
                row()
                    .key(format!("{button_key}-code-scroll"))
                    .w_full()
                    .min_w(0.)
                    .scroll_x()
                    .scroll_edge_fade(28.)
                    .child(
                        el().px(10.).py(8.).child(
                            text(code)
                                .font(TextStyle::new(12., 400.).family(Family::Mono).lh(17.))
                                .color(t.text),
                        ),
                    ),
            ),
        Err(message) => el()
            .key(format!("{button_key}-code-box"))
            .w_full()
            .min_w(0.)
            .px(10.)
            .py(8.)
            .radius(6.)
            .fill(t.bg)
            .border(1., t.line)
            .child(
                text(*message)
                    .font(TextStyle::new(12., 400.).family(Family::Sans).lh(17.))
                    .color(t.muted)
                    .wrap(),
            ),
    };
    col()
        .key(format!("{button_key}-card"))
        .w_full()
        .min_w(0.)
        .padding(14.)
        .gap(10.)
        .radius(8.)
        .fill(t.sidebar)
        .border(1., t.line)
        .child(
            row()
                .w_full()
                .gap(12.)
                .center_y()
                .child(
                    col()
                        .grow(1.)
                        .min_w(0.)
                        .gap(2.)
                        .child(
                            text(title)
                                .font(TextStyle::new(13.5, 500.).family(Family::Sans).lh(18.))
                                .color(t.text),
                        )
                        .child(description_text(subtitle, t)),
                )
                .child(
                    action_button_with_icon(
                        button_key,
                        button_title,
                        Some(&phosphor::COPY),
                        false,
                        t,
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        tx.instant();
                        s.copy_agent_setup(tx, codex);
                    }),
                ),
        )
        .child(preview)
}

fn general_page(s: &State, t: Theme) -> El {
    let name_changed = s.settings.workspace_name.text.trim() != s.workspace.name;
    let name_field = super::inputs::invalid(
        super::inputs::field(
            &s.settings.workspace_name,
            "settings-workspace-name",
            "Workspace name",
            t,
        )
        .label("Workspace name")
        .on_edit(|s: &mut State| &mut s.settings.workspace_name)
        .on_input_change(|s: &mut State, tx| {
            s.settings.workspace_name_error = None;
            s.settings.general_feedback = None;
            tx.instant();
        })
        .on_submit(|s: &mut State, tx| {
            tx.instant();
            s.save_workspace_name();
        }),
        s.settings.workspace_name_error.is_some(),
        t,
    );

    let data_path = s
        .settings
        .data_path_display
        .as_deref()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            std::path::absolute(&s.path)
                .unwrap_or_else(|_| s.path.clone())
                .display()
                .to_string()
        });
    let workspace_count = s.workspaces.len();
    let count_copy = if workspace_count == 1 {
        "1 local workspace on this Mac · Stored offline".to_owned()
    } else {
        format!("{workspace_count} local workspaces on this Mac · Stored offline")
    };

    col()
        .key("settings-page-general")
        .w_full()
        .child(section_header("Workspace settings", false, t))
        .child(
            col()
                .w_full()
                .pt(12.)
                .pb(8.)
                .gap(16.)
                .child(
                    row()
                        .w_full()
                        .gap(16.)
                        .center_y()
                        .child({
                            let glyph = if s.workspace.icon.trim().is_empty() {
                                s.workspace
                                    .name
                                    .chars()
                                    .next()
                                    .map(|c| c.to_uppercase().to_string())
                                    .unwrap_or_else(|| "M".into())
                            } else {
                                s.workspace.icon.clone()
                            };
                            el().key("settings-workspace-icon")
                                .role("button")
                                .label("Change workspace icon")
                                .expanded(
                                    s.settings.dropdown == Some(SettingsDropdown::WorkspaceIcon),
                                )
                                .focusable(true)
                                .size(56., 56.)
                                .center()
                                .radius(10.)
                                .fill(t.sidebar)
                                .hover(t.hover)
                                .press(t.selected)
                                .border(1., t.line)
                                .child(
                                    text(glyph)
                                        .font(TextStyle::new(26., 600.).family(Family::Sans))
                                        .color(t.text),
                                )
                                .on_press_tx(|s: &mut State, tx| {
                                    tx.instant();
                                    s.toggle_settings_dropdown(SettingsDropdown::WorkspaceIcon);
                                })
                        })
                        .child(
                            col()
                                .grow(1.)
                                .min_w(0.)
                                .gap(6.)
                                .child(
                                    text("Name")
                                        .font(TextStyle::new(12.5, 500.).family(Family::Sans).lh(16.))
                                        .color(t.muted),
                                )
                                .child(
                                    row()
                                        .w_full()
                                        .gap(8.)
                                        .center_y()
                                        .child(el().grow(1.).min_w(0.).child(name_field))
                                        .children(name_changed.then(|| {
                                            action_button("settings-workspace-save", "Save", true, t)
                                                .on_press_tx(|s: &mut State, tx| {
                                                    tx.instant();
                                                    s.save_workspace_name();
                                                })
                                        })),
                                )
                                .children(s.settings.workspace_name_error.as_ref().map(|err| {
                                    text(err)
                                        .key("settings-workspace-name-error")
                                        .font(TextStyle::new(12., 400.).family(Family::Sans).lh(16.))
                                        .color(t.error)
                                        .wrap()
                                })),
                        ),
                )
                .child(description_text(
                    "Click the workspace icon to choose an emoji, or edit the name and press Enter to save.",
                    t,
                )),
        )
        .child(section_header("Workspace details", true, t))
        .child(setting_row(
            "Workspace ID",
            text(&s.workspace.id)
                .font(TextStyle::new(12., 400.).family(Family::Mono).lh(17.))
                .color(t.muted)
                .wrap(),
            action_button("settings-copy-workspace-id", "Copy ID", false, t).on_press_tx(
                |s: &mut State, tx| {
                    tx.instant();
                    s.copy_workspace_id(tx);
                },
            ),
            t,
        ))
        .child(setting_row(
            "Local data folder",
            text(data_path)
                .font(TextStyle::new(12., 400.).family(Family::Mono).lh(17.))
                .color(t.muted)
                .wrap(),
            action_button("settings-copy-data-path", "Copy path", false, t).on_press_tx(
                |s: &mut State, tx| {
                    tx.instant();
                    s.copy_data_path(tx);
                },
            ),
            t,
        ))
        .child(setting_row(
            "Storage",
            description_text(count_copy, t),
            el(),
            t,
        ))
        .children(s.settings.general_feedback.map(|message| {
            text(message)
                .key("settings-general-feedback")
                .pt(8.)
                .font(TextStyle::new(12.5, 500.).family(Family::Sans).lh(18.))
                .color(t.blue)
                .live_region(LiveRegion::Polite)
        }))
}

fn import_export_page(s: &State, t: Theme) -> El {
    let busy = s.archive_transfer.busy;
    let status_card = (busy || s.archive_transfer.status.is_some()).then(|| {
        row()
            .key("settings-archive-card")
            .w_full()
            .mt(12.)
            .px(14.)
            .py(11.)
            .gap(12.)
            .center_y()
            .radius(8.)
            .fill(t.sidebar)
            .border(1., t.line)
            .child(
                text(
                    s.archive_transfer
                        .status
                        .as_deref()
                        .unwrap_or("Working on archive transfer…"),
                )
                .key("archive-transfer-status")
                .font(TextStyle::new(13., 400.).family(Family::Sans).lh(18.))
                .color(t.text)
                .grow(1.)
                .min_w(0.)
                .wrap()
                .live_region(LiveRegion::Polite),
            )
            .children(busy.then(|| {
                action_button("cancel-archive-transfer", "Cancel transfer", false, t).on_press_tx(
                    |s: &mut State, tx| {
                        tx.instant();
                        s.cancel_archive_transfer();
                    },
                )
            }))
    });

    col()
        .key("settings-page-import-export")
        .w_full()
        .child(section_header("Export", false, t))
        .child(setting_row(
            "Export workspace archive",
            description_text(
                "Save a local .moss archive of this workspace, including pages, databases, formulas, relations, comments, history, and images.",
                t,
            ),
            action_button("export-workspace", "Export archive…", false, t)
                .disabled(busy)
                .on_press_tx(|s: &mut State, tx| {
                    if s.archive_transfer.busy {
                        return;
                    }
                    let name = format!("{}.moss", s.workspace.name);
                    tx.save_file(
                        FileDialogOptions {
                            title: "Export workspace".into(),
                            file_name: Some(name),
                            ..Default::default()
                        }
                        .filter("Moss archive", &["moss"]),
                        |s: &mut State, result, _| match result {
                            Ok(Some(path)) => s.transfer_archive(path, false),
                            Ok(None) => {}
                            Err(e) => s.error = Some(e.to_string()),
                        },
                    );
                }),
            t,
        ))
        .child(section_header("Import", true, t))
        .child(setting_row(
            "Import workspace archive",
            description_text(
                "Restore a .moss archive as a separate local workspace. Existing workspaces are not modified.",
                t,
            ),
            action_button("import-workspace", "Import archive…", false, t)
                .disabled(busy)
                .on_press_tx(|s: &mut State, tx| {
                    if s.archive_transfer.busy {
                        return;
                    }
                    tx.open_file(
                        FileDialogOptions {
                            title: "Import workspace".into(),
                            ..Default::default()
                        }
                        .filter("Moss archive", &["moss"]),
                        |s: &mut State, result, _| match result {
                            Ok(Some(path)) => s.transfer_archive(path, true),
                            Ok(None) => {}
                            Err(e) => s.error = Some(e.to_string()),
                        },
                    );
                }),
            t,
        ))
        .children(status_card)
        .child(
            description_text(
                "Archives include images. Other file types are not yet supported.",
                t,
            )
            .pt(14.),
        )
}

fn active_dropdown(s: &State, t: Theme) -> Option<El> {
    match s.settings.dropdown? {
        SettingsDropdown::Appearance => Some(
            dropdown_menu("settings-appearance-menu", "Appearance", 220., t)
                .children(
                    [
                        (None, "System", "Use system setting"),
                        (Some(false), "Light", "Light"),
                        (Some(true), "Dark", "Dark"),
                    ]
                    .into_iter()
                    .map(|(dark, key_suffix, label)| {
                        let checked = s.prefs.dark == dark;
                        dropdown_option(
                            format!("theme-{key_suffix}"),
                            label,
                            checked,
                            t,
                        )
                        .on_press_tx(move |s: &mut State, tx| {
                            tx.instant();
                            s.set_theme(dark);
                            s.dismiss_settings_dropdown();
                        })
                    }),
                )
                .anchored_fit("settings-appearance-trigger", Side::Bottom, Align::End, 4.),
        ),
        SettingsDropdown::WeekStart => Some(
            dropdown_menu("settings-week-start-menu", "Start week on", 184., t)
                .children(WEEKDAYS.into_iter().enumerate().map(|(day, name)| {
                    let checked = s.prefs.week_start() == day as u8;
                    dropdown_option(
                        format!("week-start-{}", name.to_lowercase()),
                        name,
                        checked,
                        t,
                    )
                    .label(format!("Start week on {name}"))
                    .on_press_tx(move |s: &mut State, tx| {
                        tx.instant();
                        s.set_week_start(day as u8);
                        s.dismiss_settings_dropdown();
                    })
                }))
                .anchored_fit("settings-week-start-trigger", Side::Bottom, Align::End, 4.),
        ),
        SettingsDropdown::WorkspaceIcon => Some(workspace_icon_picker(s, t)),
    }
}

fn dropdown_menu(key: &'static str, label: &'static str, width: f32, t: Theme) -> El {
    col()
        .key(key)
        .w(width)
        .padding(4.)
        .gap(2.)
        .fill(t.menu)
        .radius(8.)
        .border(1., t.line)
        .shadow(6., 20., Color::BLACK.alpha(if t.dark { 0.42 } else { 0.16 }))
        .role("menu")
        .label(label)
        .focus_group()
        .modal(true, |s: &mut State| s.dismiss_settings_dropdown())
}

fn dropdown_option(key: String, title: &str, checked: bool, t: Theme) -> El {
    row()
        .key(key.clone())
        .w_full()
        .h(30.)
        .px(10.)
        .gap(8.)
        .center_y()
        .radius(5.)
        .hover(t.hover)
        .press(t.selected)
        .role("menuitemradio")
        .label(title)
        .checked(checked)
        .focusable(true)
        .child(
            text(title)
                .font(TextStyle::new(13.5, 400.).family(Family::Sans).lh(18.))
                .color(t.text)
                .grow(1.)
                .min_w(0.)
                .truncate(),
        )
        .child(menu::end(
            format!("{key}-check"),
            checked.then_some(&menu::CHECK),
            t,
        ))
}

fn workspace_icon_picker(s: &State, t: Theme) -> El {
    let matches = emoji_catalog::matches(&s.settings.icon_search.text, None);
    let visible: Vec<_> = matches.into_iter().take(48).collect();
    let rows = visible.chunks(8).enumerate().map(|(r, chunk)| {
        row().w_full().gap(2.).children(chunk.iter().enumerate().map(|(c, item)| {
            let glyph = item.glyph;
            row()
                .key(format!("settings-icon-choice-{r}-{c}"))
                .size(32., 32.)
                .center()
                .radius(5.)
                .hover(t.hover)
                .press(t.selected)
                .role("button")
                .label(item.name)
                .focusable(true)
                .child(text(glyph).font(TextStyle::new(18., 400.)))
                .on_press_tx(move |s: &mut State, tx| {
                    tx.instant();
                    s.set_workspace_icon(glyph);
                })
        }))
    });

    col()
        .key("settings-workspace-icon-picker")
        .w(288.)
        .padding(8.)
        .gap(8.)
        .fill(t.menu)
        .radius(10.)
        .border(1., t.line)
        .shadow(8., 24., Color::BLACK.alpha(if t.dark { 0.45 } else { 0.18 }))
        .role("dialog")
        .label("Choose workspace icon")
        .focus_group()
        .modal(true, |s: &mut State| s.dismiss_settings_dropdown())
        .child(
            row()
                .w_full()
                .gap(6.)
                .center_y()
                .child(
                    el().grow(1.).min_w(0.).child(
                        super::inputs::field(
                            &s.settings.icon_search,
                            "settings-icon-search",
                            "Filter emoji…",
                            t,
                        )
                        .label("Filter emoji")
                        .on_edit(|s: &mut State| &mut s.settings.icon_search)
                        .on_input_change(|_: &mut State, tx| tx.instant()),
                    ),
                )
                .children((!s.workspace.icon.is_empty()).then(|| {
                    action_button("settings-icon-remove", "Remove", false, t).on_press_tx(
                        |s: &mut State, tx| {
                            tx.instant();
                            s.set_workspace_icon("");
                        },
                    )
                })),
        )
        .child(
            col()
                .w_full()
                .max_h(210.)
                .gap(2.)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(rows)
                .children(visible.is_empty().then(|| {
                    description_text("No matching emoji", t).padding(10.)
                })),
        )
        .anchored_fit("settings-workspace-icon", Side::Bottom, Align::Start, 6.)
}
