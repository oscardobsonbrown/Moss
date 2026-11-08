use super::{CHROME_EDGE, COLLAPSED_SIDEBAR_WIDTH, phosphor, theme::Theme};
use crate::{
    sidebar_reveal::SidebarSurface,
    sidebar_tree::SidebarRow,
    state::{Modal, State},
};
use rune::*;

fn action(key: &str, name: &str, icon: &'static Icon, icon_size: f32, t: Theme) -> El {
    row()
        .key(key)
        .w_full()
        .h(30.)
        .center()
        .radius(5.)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label(name)
        .focusable(true)
        .child(rune::icon(icon, icon_size).color(t.muted))
}

fn divider(key: &str, height: f32, t: Theme) -> El {
    row()
        .w_full()
        .h(height)
        .center()
        .child(el().key(key).size(20., 1.).fill(t.line))
}

fn page(s: &State, page: &moss_core::Page, prefix: &str, t: Theme) -> El {
    let id = page.id.clone();
    let title = if page.title.is_empty() {
        "Untitled"
    } else {
        &page.title
    };
    let active = s.tab().is_some_and(|tab| tab.page.id == id);
    row()
        .key(format!("rail-{prefix}-{id}"))
        .w_full()
        .h(30.)
        .center()
        .role("button")
        .label(title)
        .focusable(true)
        .child(
            row()
                .size(26., 26.)
                .center()
                .radius(5.)
                .fill(if active {
                    t.selected
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .child(if page.icon.is_empty() {
                    rune::icon(&phosphor::FILE, 16.).color(t.muted)
                } else {
                    t.sized_label(&page.icon, 16., t.text)
                }),
        )
        .on_press_tx(move |s: &mut State, tx| {
            s.open_page(&id, false);
            tx.instant();
        })
}

fn private_row(s: &State, i: usize, t: Theme) -> El {
    match &s.sidebar_tree.rows[i] {
        SidebarRow::Page { page: item, .. } => page(s, item, "page", t),
        SidebarRow::Paging {
            parent,
            previous,
            next,
            ..
        } => {
            let mut controls = row().w_full().h(30.).center();
            if let Some(cursor) = previous.clone() {
                let parent = parent.clone();
                controls = controls.child(
                    row()
                        .key(format!(
                            "rail-previous-{}",
                            parent.as_deref().unwrap_or("root")
                        ))
                        .size(20., 26.)
                        .center()
                        .radius(4.)
                        .hover(t.hover)
                        .role("button")
                        .label("Previous pages")
                        .focusable(true)
                        .child(rune::icon(&phosphor::CARET_LEFT, 14.).color(t.muted))
                        .on_press(move |s: &mut State| {
                            s.page_sidebar_group(parent.clone(), cursor.clone(), false)
                        }),
                );
            }
            if let Some(cursor) = next.clone() {
                let parent = parent.clone();
                controls = controls.child(
                    row()
                        .key(format!("rail-next-{}", parent.as_deref().unwrap_or("root")))
                        .size(20., 26.)
                        .center()
                        .radius(4.)
                        .hover(t.hover)
                        .role("button")
                        .label("Next pages")
                        .focusable(true)
                        .child(rune::icon(&phosphor::CARET_RIGHT, 14.).color(t.muted))
                        .on_press(move |s: &mut State| {
                            s.page_sidebar_group(parent.clone(), cursor.clone(), true)
                        }),
                );
            }
            controls
        }
    }
}

fn groups(s: &State, t: Theme) -> El {
    use super::sidebar_sections::{self, Item};
    let rows = std::rc::Rc::new(sidebar_sections::items(s, true));
    let keys = rows.clone();
    col()
        .key("rail-pages")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows::<State>(
            rows.len(),
            30.,
            move |s, i| std::borrow::Cow::Owned(sidebar_sections::item_key(s, keys[i], true)),
            move |s, i| match rows[i] {
                Item::Header(i) => divider(
                    &format!("rail-group-{}", s.sidebar_sections.sections[i].id),
                    26.,
                    t,
                ),
                Item::Private(i) => private_row(s, i, t),
                Item::Favorite(i) => page(s, &s.sidebar_favorites.pages[i], "favorite", t),
                Item::Shortcut(i, index) => {
                    let section = &s.sidebar_sections.sections[i];
                    page(
                        s,
                        &s.sidebar_sections.pages[&section.id][index],
                        &sidebar_sections::shortcut_prefix(section),
                        t,
                    )
                }
                Item::New => unreachable!(),
            },
        )
}

pub fn view(s: &State, t: Theme) -> El {
    col()
        .key("sidebar-rail")
        .w(COLLAPSED_SIDEBAR_WIDTH)
        .min_w(COLLAPSED_SIDEBAR_WIDTH)
        .h_full()
        .px(8.)
        .gap(2.)
        .fill(t.sidebar)
        .on_hover_change(|s: &mut State, inside, tx| {
            s.sidebar_reveal_hover(SidebarSurface::Rail, inside, tx);
            if inside {
                tx.animated();
            }
        })
        .child(el().h(6.))
        .child(
            action("rail-search", "Search", &phosphor::MAGNIFYING_GLASS, 16., t)
                .h(32.)
                .on_press(|s: &mut State| s.open_search()),
        )
        .child(el().h(18.))
        .child(
            col()
                .key("rail-scroll")
                .grow(1.)
                .min_h(0.)
                .w_full()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(groups(s, t)),
        )
        .child(divider("rail-group-utilities", 12., t))
        .child(
            action("rail-settings", "Settings", &phosphor::GEAR_SIX, 18., t)
                .on_press(|s: &mut State| s.modal = Some(Modal::Settings)),
        )
        .child(
            action("rail-trash", "Trash", &phosphor::TRASH, 18., t)
                .on_press(|s: &mut State| s.open_trash()),
        )
        .child(
            row()
                .key("rail-workspace")
                .w_full()
                .h(40.)
                .center()
                .radius(5.)
                .hover(t.hover)
                .role("button")
                .label("Switch workspace")
                .focusable(true)
                .child(t.sized_label(&s.workspace.icon, 18., t.text))
                .on_press(|s: &mut State| {
                    // The workspace popup keeps its anchor in the revealed sidebar.
                    s.open_sidebar_reveal_for_keyboard();
                    s.modal = Some(Modal::Workspace);
                }),
        )
        .child(el().h(8.))
}

pub fn preview(s: &State, t: Theme, width: f32) -> Option<El> {
    if !s.sidebar_collapsed() || !s.sidebar_reveal.visible {
        return None;
    }
    let panel = col()
        .key("sidebar-preview")
        .pin(Some(0.), Some(CHROME_EDGE), None, Some(0.))
        .w(width)
        .min_h(0.)
        .fill(t.sidebar)
        .corners([0., 8., 8., 0.])
        .shadow(4., 16., Color::BLACK.alpha(0.14))
        .transition(Transition::opacity())
        .on_hover_change(|s: &mut State, inside, tx| {
            s.sidebar_reveal_hover(SidebarSurface::Panel, inside, tx);
            if inside {
                tx.animated();
            }
        })
        .on_key(|s: &mut State, event, tx| {
            if event.key == Key::Escape {
                if s.sidebar_page_menu.is_none()
                    && s.sidebar_section_menu.is_none()
                    && s.page_icon_picker.is_none()
                    && s.modal.is_none()
                {
                    s.dismiss_sidebar_reveal_by_key();
                    tx.instant();
                    return true;
                }
            } else {
                s.open_sidebar_reveal_for_keyboard();
            }
            false
        })
        .child(super::shell::sidebar(s, t, width));
    Some(if s.sidebar_reveal.keyboard_open {
        panel.modal(true, |s: &mut State| s.dismiss_sidebar_reveal_by_key())
    } else {
        panel
    })
}
