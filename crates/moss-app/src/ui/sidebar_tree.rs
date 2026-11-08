use super::theme::{Theme, label};
use crate::{sidebar_tree::SidebarRow, state::State};
use rune::*;

pub fn row_at(s: &State, i: usize, t: Theme) -> El {
    match &s.sidebar_tree.rows[i] {
        SidebarRow::Page {
            page,
            has_children,
            expanded,
            depth,
        } => page_row(s, page, *has_children, *expanded, *depth, "page", t),
        SidebarRow::Paging {
            parent,
            previous,
            next,
            depth,
        } => {
            let mut controls = row()
                .w_full()
                .h(30.)
                .pl(28. + (*depth as f32 * 16.).min(160.))
                .gap(10.)
                .center_y();
            if let Some(cursor) = previous.clone() {
                let parent = parent.clone();
                controls = controls.child(
                    label("Previous", t.muted)
                        .key(format!(
                            "sidebar-previous-{}",
                            parent.as_deref().unwrap_or("root")
                        ))
                        .role("button")
                        .focusable(true)
                        .on_press(move |s: &mut State| {
                            s.page_sidebar_group(parent.clone(), cursor.clone(), false)
                        }),
                );
            }
            if let Some(cursor) = next.clone() {
                let parent = parent.clone();
                controls = controls.child(
                    label("Next", t.muted)
                        .key(format!(
                            "sidebar-next-{}",
                            parent.as_deref().unwrap_or("root")
                        ))
                        .role("button")
                        .focusable(true)
                        .on_press(move |s: &mut State| {
                            s.page_sidebar_group(parent.clone(), cursor.clone(), true)
                        }),
                );
            }
            controls
        }
    }
}

pub fn page_row(
    s: &State,
    page: &moss_core::Page,
    has_children: bool,
    expanded: bool,
    depth: usize,
    prefix: &str,
    t: Theme,
) -> El {
    let id = page.id.clone();
    let title = if page.title.is_empty() {
        "Untitled"
    } else {
        &page.title
    };
    let active = s.tab().is_some_and(|tab| tab.page.id == id);
    let width = rune::memo::layout_with("sidebar", |rect| rect[2]).unwrap_or(270.);
    // Keep at least 40 pt for titles after padding and the fixed control lanes.
    let indent = (depth as f32 * 16.).min(160.).min((width - 171.).max(0.));
    let icon_key = format!("{prefix}-icon-{id}");
    let menu_key = format!("{prefix}-actions-{id}");
    let menu_id = id.clone();
    let row_id = id.clone();
    let context_icon = icon_key.clone();
    let icon_id = id.clone();
    let icon_anchor = icon_key.clone();
    let action_id = id.clone();
    let action_anchor = menu_key.clone();
    let action_icon = icon_key.clone();
    let child_id = id.clone();
    let disclosure_id = id.clone();
    row().w_full().h(30.).center_y().child(
        row()
            .key(format!("{prefix}-highlight-{id}"))
            .w_full()
            .h(26.)
            .pl(10. + indent)
            .pr(3.)
            .gap(8.)
            .center_y()
            .radius(4.)
            .fill(if active { t.selected } else { t.sidebar })
            .hover(t.hover)
            .hover_group()
            .hover_motion(Anim::INSTANT, Anim::INSTANT)
            .on_press_tx(move |s: &mut State, tx| {
                s.open_page(&row_id, false);
                tx.instant();
            })
            .role("group")
            .on_secondary_press(move |s: &mut State, point, _| {
                s.open_sidebar_page_menu(&menu_id, point);
                if let Some(menu) = &mut s.sidebar_page_menu {
                    menu.icon_anchor = context_icon.clone();
                }
            })
            .child(
                el().key(icon_key)
                    .size(20., 24.)
                    .center()
                    .radius(4.)
                    .hover(t.selected)
                    .role("button")
                    .label(format!("Change icon for {title}"))
                    .focusable(true)
                    .child(if page.icon.is_empty() {
                        rune::icon(&super::icons::PAGE_BLANK, 16.).color(t.muted)
                    } else {
                        super::theme::sized_label(&page.icon, 16., t.text)
                    })
                    .on_press(move |s: &mut State| s.open_page_icon_picker(&icon_id, &icon_anchor)),
            )
            .child(
                row()
                    .key(format!("{prefix}-open-{id}"))
                    .h_full()
                    .grow(1.)
                    .min_w(0.)
                    .center_y()
                    .role("treeitem")
                    .label(title)
                    .focus_selected(active)
                    .focusable(true)
                    .child(
                        label(title, t.text)
                            .key(format!("{prefix}-title-{id}"))
                            .truncate()
                            .grow(1.)
                            .min_w(0.),
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        tx.instant();
                        s.open_page(&id, false);
                    }),
            )
            .child(
                row()
                    .gap(0.)
                    .center_y()
                    .child(if has_children {
                        action(
                            s,
                            format!("{prefix}-disclosure-{disclosure_id}"),
                            if expanded {
                                "Collapse page"
                            } else {
                                "Expand page"
                            },
                            if expanded { &DOWN } else { &RIGHT },
                            t,
                        )
                        .on_press_tx(move |s: &mut State, tx| {
                            s.toggle_sidebar_page(&disclosure_id);
                            tx.instant();
                        })
                    } else {
                        el().size(22., 24.)
                    })
                    .child(action(s, menu_key, "Page actions", &MORE, t).on_press(
                        move |s: &mut State| {
                            s.open_sidebar_page_menu_at_anchor(
                                &action_id,
                                &action_anchor,
                                &action_icon,
                            )
                        },
                    ))
                    .child(
                        action(
                            s,
                            format!("{prefix}-add-{child_id}"),
                            "New subpage",
                            &PLUS,
                            t,
                        )
                        .on_press_tx(move |s: &mut State, tx| {
                            s.begin_page(Some(child_id.clone()));
                            tx.instant();
                        }),
                    ),
            ),
    )
}

use super::phosphor::{CARET_DOWN as DOWN, CARET_RIGHT as RIGHT, DOTS_THREE as MORE, PLUS};
fn action(s: &State, key: String, name: &str, icon: &'static Icon, t: Theme) -> El {
    super::sidebar_actions::button(s, key, name, icon, 14., t.selected, t)
}
