use super::theme::{Theme, label};
use crate::state::State;
use rune::*;

pub fn popup(s: &State, t: Theme) -> Option<El> {
    let menu = s.sidebar_page_menu.as_ref()?;
    let id = menu.page.id.clone();
    let title = if menu.page.title.is_empty() {
        "Untitled"
    } else {
        &menu.page.title
    };
    let mut body = col()
        .w_full()
        .gap(0.)
        .instant_changes()
        .child(label("Page", t.muted).px(8.).py(4.));
    if menu.renaming {
        body = body
            .child(
                super::inputs::invalid(
                    super::inputs::field(&s.sidebar_page_name, "sidebar-page-name", "Untitled", t),
                    menu.error.is_some(),
                    t,
                )
                .label("Page name")
                .on_edit(|s: &mut State| &mut s.sidebar_page_name)
                .on_submit(|s: &mut State, tx| {
                    s.rename_sidebar_page();
                    tx.instant();
                }),
            )
            .child(
                item("sidebar-page-rename-save", "Save name", "✓", t.text, t).on_press_tx(
                    |s: &mut State, tx| {
                        s.rename_sidebar_page();
                        tx.instant();
                    },
                ),
            );
    } else {
        let target = id.clone();
        body = body.child(
            item("sidebar-page-open", "Open", "↗", t.text, t).on_press_tx(
                move |s: &mut State, tx| {
                    s.open_page(&target, false);
                    tx.instant();
                },
            ),
        );
        let target = id.clone();
        body = body.child(
            item("sidebar-page-open-tab", "Open in new tab", "↗", t.text, t).on_press_tx(
                move |s: &mut State, tx| {
                    s.open_page(&target, true);
                    tx.instant();
                },
            ),
        );
        body = body.child(rule(t)).child(
            item("sidebar-page-rename", "Rename", "Aa", t.text, t).on_press_tx(
                |s: &mut State, tx| {
                    if let Some(menu) = &mut s.sidebar_page_menu {
                        menu.renaming = true;
                    }
                    s.sidebar_page_name.focused = true;
                    s.sidebar_page_name
                        .set_selection(0, s.sidebar_page_name.text.len());
                    tx.instant();
                },
            ),
        );
        let target = id.clone();
        let anchor = menu.icon_anchor.clone();
        body = body.child(
            item("sidebar-page-icon", "Edit icon", "☺", t.text, t)
                .on_press(move |s: &mut State| s.open_page_icon_picker(&target, &anchor)),
        );
        let target = id.clone();
        body = body.child(
            item(
                "sidebar-page-favorite",
                if s.is_sidebar_favorite(&id) {
                    "Remove from Favorites"
                } else {
                    "Add to Favorites"
                },
                "☆",
                t.text,
                t,
            )
            .on_press_tx(move |s: &mut State, tx| {
                if s.toggle_sidebar_favorite(&target) {
                    s.close_sidebar_page_menu();
                }
                tx.instant();
            }),
        );
        let target = id.clone();
        let anchor = menu.icon_anchor.clone();
        body = body.child(
            item("sidebar-page-section", "Add to section", "+", t.text, t).on_press_tx(
                move |s: &mut State, tx| {
                    s.open_sidebar_section_menu(
                        crate::sidebar_section_menu::SectionMenuMode::ChooseSection(target.clone()),
                        &anchor,
                    );
                    tx.instant();
                },
            ),
        );
        let target = id.clone();
        body = body
            .child(
                item("sidebar-page-new-subpage", "New subpage", "+", t.text, t).on_press_tx(
                    move |s: &mut State, tx| {
                        s.begin_page(Some(target.clone()));
                        tx.instant();
                    },
                ),
            )
            .child(rule(t));
        body = body.child(
            item("sidebar-page-trash", "Move to Trash", "♧", t.error, t).on_press_tx(
                move |s: &mut State, tx| {
                    s.trash_page_by_id(&id);
                    tx.instant();
                },
            ),
        );
        if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&menu.page.updated_at) {
            body = body.child(rule(t)).child(
                label(
                    format!(
                        "Last edited {}",
                        date.with_timezone(&chrono::Local).format("%e %b, %H:%M")
                    ),
                    t.muted,
                )
                .px(8.)
                .py(4.),
            );
        }
    }
    body = body.children(
        menu.error
            .as_ref()
            .map(|error| label(error, t.error).wrap().px(8.).py(4.)),
    );
    let popup = col()
        .key("sidebar-page-menu")
        .w(232.)
        .max_h(500.)
        .min_h(0.)
        .padding(4.)
        .fill(t.menu)
        .border(1., t.line)
        .radius(8.)
        .shadow(4., 16., Color::BLACK.alpha(0.18))
        .role("menu")
        .label(format!("Page actions for {title}"))
        .focus_group()
        .focus_return(
            menu.anchor
                .clone()
                .unwrap_or_else(|| menu.icon_anchor.replacen("-icon-", "-open-", 1)),
        )
        .modal(true, |s: &mut State| s.close_sidebar_page_menu())
        .transition(Transition::opacity())
        .child(body);
    let popup = if let Some(anchor) = &menu.anchor {
        popup.anchored_fit(anchor, Side::Right, Align::Start, 4.)
    } else {
        popup.anchored_fit("sidebar-page-menu-anchor", Side::Bottom, Align::Start, 0.)
    };
    Some(
        el().pin(Some(0.), Some(0.), Some(0.), Some(0.))
            .child(
                el().key("sidebar-page-menu-anchor")
                    .size(0., 0.)
                    .absolute(menu.point.x, menu.point.y),
            )
            .child(popup),
    )
}

fn rule(t: Theme) -> El {
    el().w_full().h(1.).fill(t.line).my(4.)
}

fn item(key: &str, name: &str, icon: &str, ink: Color, t: Theme) -> El {
    row()
        .key(key)
        .w_full()
        .h(26.)
        .px(8.)
        .gap(8.)
        .center_y()
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .role("menuitem")
        .label(name)
        .focusable(true)
        .child(
            el().size(18., 18.)
                .center()
                .child(super::icons::glyph(icon, 16., ink)),
        )
        .child(
            text(name)
                .font(TextStyle::new(14., 400.).lh(20.))
                .color(ink),
        )
}
