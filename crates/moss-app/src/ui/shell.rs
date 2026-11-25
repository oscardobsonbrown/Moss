use super::theme::{Theme, button, label};
use crate::state::{Modal, State};
use rune::*;
use std::hash::{Hash, Hasher};

use super::phosphor::{
    CARET_LEFT as BACK_ICON, CARET_RIGHT as FORWARD_ICON, NOTE_PENCIL as COMPOSE_ICON,
    SIDEBAR_SIMPLE as SIDEBAR_ICON,
};
fn shell_icon(
    key: impl Into<String>,
    name: &str,
    icon: &'static Icon,
    enabled: bool,
    t: Theme,
) -> El {
    row()
        .key(key)
        .size(28., 28.)
        .center()
        .radius(5.)
        .hover(t.hover)
        .role("button")
        .label(name)
        .focusable(enabled)
        .child(rune::icon(icon, 18.).color(if enabled {
            t.muted
        } else {
            t.faint.alpha(0.45)
        }))
}

pub fn titlebar(s: &State, cx: &mut Cx<State>, t: Theme, sidebar_width: f32) -> El {
    let insets = cx.chrome_insets();
    let collapsed = s.sidebar_collapsed();
    let back_enabled = s.navigation_position > 0 && !s.navigation_history.is_empty();
    let forward_enabled = s.navigation_position + 1 < s.navigation_history.len();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    insets.leading.to_bits().hash(&mut hasher);
    insets.trailing.to_bits().hash(&mut hasher);
    sidebar_width.to_bits().hash(&mut hasher);
    collapsed.hash(&mut hasher);
    s.active.hash(&mut hasher);
    back_enabled.hash(&mut hasher);
    forward_enabled.hash(&mut hasher);
    t.dark.hash(&mut hasher);
    t.bg.r.to_bits().hash(&mut hasher);
    t.text_scale.to_bits().hash(&mut hasher);
    t.scale.to_bits().hash(&mut hasher);
    s.tabs.len().hash(&mut hasher);
    for tab in &s.tabs {
        tab.page.id.hash(&mut hasher);
        tab.page.icon.hash(&mut hasher);
        tab.title.text.hash(&mut hasher);
    }
    let version = hasher.finish();
    rune::memo("titlebar", version, || {
        let light = t.bg.r >= 0.1;
        let tabs =
            s.tabs.iter().enumerate().map(|(i, tab)| {
                let title = if tab.title.text.is_empty() {
                    "Untitled"
                } else {
                    &tab.title.text
                };
                row()
                    .key(format!("tab-{i}"))
                    .h(28.)
                    .min_w(100.)
                    .max_w(240.)
                    .px(5.)
                    .gap(7.)
                    .center_y()
                    .radius(7.)
                    .hover(t.hover)
                    .hover_group()
                    .child({
                        let indicator = el()
                            .key(format!("tab-indicator-{i}"))
                            .pin(Some(0.), Some(0.), Some(0.), Some(0.))
                            .radius(7.)
                            .fill(if light { t.bg } else { t.selected })
                            .border(1., t.line)
                            .opacity(if i == s.active { 1. } else { 0. })
                            .opacity_motion(Spring::new(0.15, 0.));
                        if light {
                            indicator.shadow(1., 3., Color::BLACK.alpha(0.08))
                        } else {
                            indicator
                        }
                    })
                    .child(
                        row()
                            .min_w(0.)
                            .max_w(187.)
                            .h_full()
                            .gap(7.)
                            .center_y()
                            .role("tab")
                            .label(title)
                            .focus_selected(i == s.active)
                            .focusable(true)
                            .child(row().size(20., 20.).center().child(
                                if tab.page.icon.is_empty() {
                                    rune::icon(&super::icons::PAGE_BLANK, 16.).color(t.muted)
                                } else {
                                    super::theme::sized_label(&tab.page.icon, 14., t.muted)
                                },
                            ))
                            .child({
                                let title =
                                    label(title, if i == s.active { t.text } else { t.muted });
                                if light {
                                    title.min_w(0.).grow(1.).truncate()
                                } else {
                                    title.truncate()
                                }
                            })
                            .on_press_tx(move |s: &mut State, _| {
                                if !s.flush_current() {
                                    return;
                                }
                                s.active = i;
                                s.filter = TextEdit::new(&s.tabs[i].title_search);
                                let page = s.tabs[i].page.clone();
                                s.load_comments(&page.id);
                                s.record_navigation_deferred(Some(&page), &page.id);
                                s.defer_save_preferences();
                            }),
                    )
                    .child({
                        let close = row()
                            .key(format!("close-tab-{i}"))
                            .size(20., 20.)
                            .center()
                            .radius(5.)
                            .hover(t.hover)
                            .role("button")
                            .label("Close tab")
                            .focusable(true)
                            .child(rune::icon(&super::phosphor::X, 14.).color(t.muted))
                            .on_press_tx(move |s: &mut State, tx| {
                                tx.instant();
                                s.close_tab(i);
                            });
                        if light && i != s.active {
                            close.focusable(false).group_opacity(0., 1.)
                        } else {
                            close
                        }
                    })
            });
        row()
            .key("titlebar")
            .w_full()
            .h(super::CHROME_EDGE)
            .fill(t.sidebar)
            .window_drag()
            .center_y()
            .pl((sidebar_width + 8.).max(insets.leading + 8.))
            .pr(insets.trailing + 8.)
            .children(collapsed.then(|| {
                shell_icon("sidebar-expand", "Expand sidebar", &SIDEBAR_ICON, true, t).on_press_tx(
                    |s: &mut State, tx| {
                        s.toggle_sidebar();
                        tx.instant();
                    },
                )
            }))
            .children(collapsed.then(|| {
                shell_icon("sidebar-compose", "New page", &COMPOSE_ICON, true, t).on_press_tx(
                    |s: &mut State, tx| {
                        s.begin_page(None);
                        tx.instant();
                    },
                )
            }))
            .children(collapsed.then(|| {
                row().w(16.).h(28.).center().child(
                    el().key("sidebar-header-divider")
                        .size(1., 18.)
                        .fill(t.line)
                        .role("separator"),
                )
            }))
            .child({
                let button = shell_icon("history-back", "Back", &BACK_ICON, back_enabled, t);
                if back_enabled {
                    button.on_press_tx(|s: &mut State, tx| {
                        s.navigate_history(false);
                        tx.instant();
                    })
                } else {
                    button
                }
            })
            .child({
                let button = shell_icon(
                    "history-forward",
                    "Forward",
                    &FORWARD_ICON,
                    forward_enabled,
                    t,
                );
                if forward_enabled {
                    button.on_press_tx(|s: &mut State, tx| {
                        s.navigate_history(true);
                        tx.instant();
                    })
                } else {
                    button
                }
            })
            .child(
                row()
                    .key("tab-strip")
                    .grow(1.)
                    .min_w(0.)
                    .h_full()
                    .scroll_x()
                    .child(
                        row().h_full().gap(4.).center_y().children(tabs).child(
                            shell_icon("new-tab", "New tab", &super::phosphor::PLUS, true, t)
                                .size(28., 28.)
                                .on_press(|s: &mut State| s.open_new_tab_search()),
                        ),
                    ),
            )
            .children((!collapsed).then(|| {
                el().key("sidebar-header-divider")
                    .absolute(sidebar_width - 0.5, (super::CHROME_EDGE - 18.) / 2.)
                    .size(1., 18.)
                    .fill(t.line)
                    .role("separator")
            }))
            .children((!collapsed).then(|| {
                row()
                    .key("sidebar-header")
                    .absolute(insets.leading + 8., (super::CHROME_EDGE - 28.) / 2.)
                    .w((sidebar_width - insets.leading - 16.).max(0.))
                    .h(28.)
                    .pr(8.)
                    .center_y()
                    .child(
                        shell_icon(
                            "sidebar-collapse",
                            "Collapse sidebar",
                            &SIDEBAR_ICON,
                            true,
                            t,
                        )
                        .on_press_tx(|s: &mut State, tx| {
                            s.toggle_sidebar();
                            tx.instant();
                        }),
                    )
                    .child(el().grow(1.))
                    .child(
                        shell_icon("sidebar-compose", "New page", &COMPOSE_ICON, true, t)
                            .on_press_tx(|s: &mut State, tx| {
                                s.begin_page(None);
                                tx.instant();
                            }),
                    )
            }))
    })
}
fn nav(key: &str, icon: &'static Icon, name: &str, compact: bool, t: Theme) -> El {
    row()
        .key(key)
        .h(30.)
        .px(12.)
        .gap(9.)
        .center_y()
        .radius(5.)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label(name)
        .focusable(true)
        .child(
            rune::icon(icon, 18.)
                .color(t.muted)
                .w(20.)
                .offset(if compact { -8. } else { 0. }, 0.)
                .offset_motion(tok::motion::SNAP_EXIT)
                .key(format!("sidebar-icon-{key}")),
        )
        .child(
            label(name, t.muted)
                .font(TextStyle::new(14., 500.).family(Family::Sans))
                .key(format!("sidebar-label-{key}"))
                .opacity(if compact { 0. } else { 1. })
                .opacity_motion(tok::motion::SNAP_EXIT),
        )
}
pub fn sidebar(s: &State, t: Theme, width: f32) -> El {
    let compact = s.sidebar_collapsed() && !s.sidebar_reveal.visible;
    let theme_key = (
        t.dark,
        t.bg.r.to_bits(),
        t.text_scale.to_bits(),
        t.scale.to_bits(),
    );
    let search = rune::memo("search-button", (compact, theme_key), || {
        row()
            .key("search-button")
            .w_full()
            .h(32.)
            .px(12.)
            .gap(8.)
            .center_y()
            .radius(6.)
            .border(1., if compact { Color::TRANSPARENT } else { t.line })
            .hover(t.hover)
            .role("button")
            .label("Search")
            .focusable(true)
            .child(
                rune::icon(&super::sidebar_actions::SEARCH, 16.)
                    .color(t.faint)
                    .key("sidebar-search-icon")
                    .offset(if compact { -8. } else { 0. }, 0.)
                    .offset_motion(tok::motion::SNAP_EXIT)
                    .w(20.),
            )
            .child(
                t.label("Search", t.faint)
                    .font(TextStyle::new(14., 500.).family(Family::Sans))
                    .key("sidebar-search-label")
                    .grow(1.)
                    .opacity(if compact { 0. } else { 1. })
                    .opacity_motion(tok::motion::SNAP_EXIT),
            )
            .child(
                t.label("⌘K", t.faint)
                    .key("sidebar-search-shortcut")
                    .opacity(if compact { 0. } else { 1. })
                    .opacity_motion(tok::motion::SNAP_EXIT),
            )
            .on_press(|s: &mut State| s.open_search())
    });
    let workspace_open = s.modal == Some(Modal::Workspace);
    let footer = rune::memo(
        "sidebar-footer",
        (
            compact,
            workspace_open,
            &s.workspace.id,
            &s.workspace.name,
            &s.workspace.icon,
            theme_key,
        ),
        || {
            col()
                .w_full()
                .gap(2.)
                .child(if compact {
                    row()
                        .h(12.)
                        .w(28.)
                        .center()
                        .child(el().key("rail-group-utilities").size(20., 1.).fill(t.line))
                } else {
                    el().h(12.)
                })
                .child(
                    nav(
                        "settings-button",
                        &super::sidebar_actions::GEAR,
                        "Settings",
                        compact,
                        t,
                    )
                    .w_full()
                    .on_press(|s: &mut State| s.open_settings()),
                )
                .child(
                    nav(
                        "trash-button",
                        &super::sidebar_actions::TRASH,
                        "Trash",
                        compact,
                        t,
                    )
                    .w_full()
                    .on_press(|s: &mut State| s.open_trash()),
                )
                .child(
                    row()
                        .key("workspace-picker")
                        .h(40.)
                        .w_full()
                        .px(12.)
                        .gap(8.)
                        .center_y()
                        .radius(5.)
                        .fill(if workspace_open {
                            t.selected
                        } else {
                            t.sidebar
                        })
                        .hover(t.hover)
                        .role("button")
                        .label("Switch workspace")
                        .expanded(workspace_open)
                        .focusable(true)
                        .child(
                            el().key("sidebar-workspace-icon")
                                .size(20., 24.)
                                .offset(if compact { -8. } else { 0. }, 0.)
                                .offset_motion(tok::motion::SNAP_EXIT)
                                .center()
                                .child(super::theme::sized_label(&s.workspace.icon, 16., t.text)),
                        )
                        .child(
                            text(&s.workspace.name)
                                .key("sidebar-workspace-name")
                                .font(TextStyle::new(14., 500.))
                                .color(t.text)
                                .truncate()
                                .grow(1.)
                                .min_w(0.)
                                .opacity(if compact { 0. } else { 1. })
                                .opacity_motion(tok::motion::SNAP_EXIT),
                        )
                        .child(
                            rune::icon(&super::view_icons::TOOLBAR_CARET, 16.)
                                .key("sidebar-workspace-chevron")
                                .color(t.text)
                                .opacity(if compact { 0. } else { 1. })
                                .opacity_motion(tok::motion::SNAP_EXIT),
                        )
                        .on_press(|s: &mut State| {
                            if s.sidebar_collapsed() {
                                s.open_sidebar_reveal_for_keyboard();
                            }
                            s.modal = Some(Modal::Workspace);
                        }),
                )
                .child(el().h(8.))
        },
    );
    col()
        .key("sidebar")
        .motion(Spring::NONE)
        .w(width)
        .min_w(width)
        .h_full()
        .fill(t.sidebar)
        .px(8.)
        .gap(2.)
        .child(el().h(6.))
        .child(search)
        .child(el().h(18.))
        .child(
            col()
                .key("sidebar-scroll")
                .drawn_rect_into(&s.sidebar_sections.viewport_rect)
                .grow(1.)
                .min_h(0.)
                .w_full()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .on_scroll_offset({
                    let scroll = s.sidebar_sections.scroll.clone();
                    move |position| scroll.set(position)
                })
                .child(super::sidebar_sections::view(s, t, width)),
        )
        .child(footer)
}
pub fn content(s: &State, cx: &Cx<State>, t: Theme, width: f32) -> El {
    let mut toolbar_hasher = std::collections::hash_map::DefaultHasher::new();
    t.dark.hash(&mut toolbar_hasher);
    t.bg.r.to_bits().hash(&mut toolbar_hasher);
    t.text_scale.to_bits().hash(&mut toolbar_hasher);
    t.scale.to_bits().hash(&mut toolbar_hasher);
    s.presence_open.hash(&mut toolbar_hasher);
    if let Some(tab) = s.tab() {
        true.hash(&mut toolbar_hasher);
        tab.page.id.hash(&mut toolbar_hasher);
        tab.page.icon.hash(&mut toolbar_hasher);
        tab.title.text.hash(&mut toolbar_hasher);
        s.agent_presence.get(&tab.page.id).hash(&mut toolbar_hasher);
    } else {
        false.hash(&mut toolbar_hasher);
    }
    let toolbar_version = toolbar_hasher.finish();
    let top = rune::memo("page-toolbar", toolbar_version, || {
        row()
            .key("page-toolbar")
            .h(42.)
            .w_full()
            .px(16.)
            .center_y()
            .gap(6.)
            .children(s.tab().map(|tab| {
                let id = tab.page.id.clone();
                let anchor = format!("page-toolbar-icon-{id}");
                let key = anchor.clone();
                row()
                    .gap(6.)
                    .center_y()
                    .min_w(0.)
                    .child(
                        el().key(key)
                            .size(24., 26.)
                            .center()
                            .radius(4.)
                            .hover(t.hover)
                            .role("button")
                            .label("Change page icon")
                            .focusable(true)
                            .child(if tab.page.icon.is_empty() {
                                rune::icon(&super::icons::PAGE_BLANK, 16.).color(t.muted)
                            } else {
                                super::theme::sized_label(&tab.page.icon, 16., t.text)
                            })
                            .on_press(move |s: &mut State| s.open_page_icon_picker(&id, &anchor)),
                    )
                    .child(
                        label(
                            if tab.title.text.is_empty() {
                                "Untitled"
                            } else {
                                &tab.title.text
                            },
                            t.text,
                        )
                        .truncate()
                        .min_w(0.),
                    )
            }))
            .child(el().grow(1.))
            .children(s.tab().and_then(|tab| presence(s, &tab.page.id, "", t)))
            .children(s.tab().map(|_| {
                shell_icon(
                    "page-actions",
                    "Actions",
                    &super::phosphor::DOTS_THREE,
                    true,
                    t,
                )
                .on_press(|s: &mut State| s.modal = Some(Modal::PageActions))
            }))
    });
    let body = match s.tab() {
        Some(tab) if tab.choosing_type => super::page_creation::view(s, t, width),
        Some(tab) if tab.database.is_some() => super::database::view(s, t),
        Some(_) => super::editor::view(s, cx, t, width),
        None => col()
            .grow(1.)
            .w_full()
            .center()
            .gap(16.)
            .child(label("Your workspace", t.text))
            .child(
                super::database::new_page_button("empty-create", t).on_press_tx(
                    |s: &mut State, tx| {
                        s.begin_page(None);
                        tx.instant();
                    },
                ),
            ),
    };
    let content = col().key("content");
    let content = if s
        .database_views
        .table_move
        .as_ref()
        .is_some_and(|drag| drag.active)
    {
        content
    } else {
        content.instant_changes()
    };
    content
        .grow(1.)
        .min_w(0.)
        .h_full()
        .fill(t.bg)
        .corners([10., 0., 0., 0.])
        .clip_corners([10., 0., 0., 0.])
        .child(top)
        .child(body)
        .children(s.error.as_ref().map(|err| {
            row()
                .w_full()
                .padding(10.)
                .gap(12.)
                .fill(t.sidebar)
                .child(label(err, t.text).wrap().grow(1.))
                .child(
                    button("dismiss-error", "Dismiss", t).on_press(|s: &mut State| s.error = None),
                )
        }))
}

pub fn sidebar_resize(s: &State, t: Theme, width: f32, window_width: f32) -> El {
    let original = s
        .prefs
        .sidebar_widths
        .get(&s.workspace.id)
        .copied()
        .filter(|w| w.is_finite())
        .unwrap_or(270.);
    el().key("sidebar-resize")
        .pin(Some(width - 3.), Some(super::CHROME_EDGE), None, Some(0.))
        .w(6.)
        .cursor(Cursor::ResizeColumn)
        .hover_group()
        .role("sidebar-grip")
        .label("Resize sidebar. Use Left and Right arrows.")
        .focusable(true)
        .on_press_tx(|_: &mut State, tx| tx.instant())
        .on_drag(move |s: &mut State, drag, tx| {
            tx.instant();
            if drag.phase == Phase::Down {
                drag.data = original;
                return;
            }
            if drag.cancelled() || drag.dx.abs() < 0.5 {
                s.prefs
                    .sidebar_widths
                    .insert(s.workspace.id.clone(), drag.data);
            } else {
                // Use window coordinates: the handle itself moves during the drag.
                s.set_sidebar_width(drag.window_point()[0] + 3. - drag.start_x, window_width);
            }
            if drag.phase == Phase::Up {
                s.save_preferences();
            }
        })
        .on_step(move |s: &mut State, step, tx| {
            s.set_sidebar_width(
                s.sidebar_width(window_width) + step as f32 * 10.,
                window_width,
            );
            s.save_preferences();
            tx.instant();
        })
        .on_step_limit(move |s: &mut State, end, tx| {
            s.set_sidebar_width(if end { 480. } else { 200. }, window_width);
            s.save_preferences();
            tx.instant();
        })
        .child(
            el().w(1.)
                .h_full()
                .absolute(2.5, 0.)
                .fill(t.line)
                .group_opacity(0., 1.),
        )
}

pub fn presence(s: &State, page_id: &str, prefix: &str, t: Theme) -> Option<El> {
    let agents = s.agent_presence.get(page_id)?;
    Some(
        row()
            .center_y()
            .gap(4.)
            .children(agents.iter().map(|(session, name)| {
                let session_id = session.clone();
                let key = format!("{prefix}presence-agent-{session}");
                let initial = name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                let mut avatar = row()
                    .key(&key)
                    .size(28., 28.)
                    .center()
                    .role("button")
                    .label(name)
                    .focusable(true)
                    .child(super::comments::agent_avatar(initial, 28., t).a11y_hidden())
                    .on_press(move |s: &mut State| {
                        s.presence_open = if s.presence_open.as_ref() == Some(&session_id) {
                            None
                        } else {
                            Some(session_id.clone())
                        };
                    });
                if s.presence_open.as_ref() == Some(session) {
                    avatar = avatar.child(
                        col()
                            .key(format!("{prefix}presence-name-{session}"))
                            .padding(8.)
                            .fill(t.bg)
                            .border(1., t.line)
                            .radius(6.)
                            .child(t.label(name, t.text))
                            .anchored(&key, Side::Bottom, Align::End, 4.)
                            .raise(),
                    );
                }
                avatar
            })),
    )
}
