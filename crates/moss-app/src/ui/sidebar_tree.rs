use super::sidebar_sections::PageDropZone;
use super::theme::{Theme, label};
use crate::{sidebar_tree::SidebarRow, state::State};
use rune::*;
use std::rc::Rc;

pub fn row_at(
    s: &State,
    i: usize,
    t: Theme,
    zones: Rc<Vec<PageDropZone>>,
    viewport: [f32; 4],
) -> El {
    match &s.sidebar_tree.rows[i] {
        SidebarRow::Page {
            page,
            has_children,
            expanded,
            depth,
        } => page_row(
            s,
            page,
            *has_children,
            *expanded,
            *depth,
            "page",
            "private",
            zones,
            viewport,
            t,
        ),
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
    source: &str,
    zones: Rc<Vec<PageDropZone>>,
    viewport: [f32; 4],
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
    let indent = (depth as f32 * 16.).min(160.).min((width - 163.).max(0.));
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
    let marker = s.sidebar_sections.page_drag.as_ref().is_some_and(|drag| {
        drag.target.as_deref() == Some(source) && drag.before.as_deref() == Some(id.as_str())
    });
    let mut item = row()
        .key(format!("{prefix}-highlight-{id}"))
        .w_full()
        .h(26.)
        .pl(3. + indent)
        .pr(3.)
        .gap(8.)
        .center_y()
        .radius(4.)
        .fill(if active { t.selected } else { t.sidebar })
        .hover(t.hover)
        .hover_group()
        .hover_motion(Anim::INSTANT, Anim::INSTANT)
        .on_press_tx(move |s: &mut State, tx| {
            if s.sidebar_sections.suppress_page_press {
                s.sidebar_sections.suppress_page_press = false;
                return;
            }
            s.open_page(&row_id, false);
            if has_children {
                s.toggle_sidebar_page(&row_id);
            }
            tx.instant();
        })
        .role("group")
        .on_secondary_press(move |s: &mut State, point, _| {
            s.open_sidebar_page_menu(&menu_id, point);
            if let Some(menu) = &mut s.sidebar_page_menu {
                menu.icon_anchor = context_icon.clone();
            }
        })
        .child({
            let icon = if page.icon.is_empty() {
                rune::icon(&super::icons::PAGE_BLANK, 16.).color(t.muted)
            } else {
                super::theme::sized_label(&page.icon, 16., t.text)
            };
            let slot = el()
                .key(icon_key)
                .size(20., 20.)
                .center()
                .radius(4.)
                .hover(t.selected);
            if has_children {
                slot.child(
                    icon.key(format!("{prefix}-icon-glyph-{disclosure_id}"))
                        .group_opacity(if s.sidebar_keyboard_controls { 0. } else { 1. }, 0.),
                )
                .child(draggable(
                    el().key(format!("{prefix}-disclosure-{disclosure_id}"))
                        .size(20., 20.)
                        .absolute(0., 0.)
                        .center()
                        .role("button")
                        .label(if expanded {
                            "Collapse page"
                        } else {
                            "Expand page"
                        })
                        .expanded(expanded)
                        .focusable(true)
                        .child(
                            rune::icon(if expanded { &DOWN } else { &RIGHT }, 14.)
                                .key(format!("{prefix}-disclosure-glyph-{disclosure_id}"))
                                .color(t.muted)
                                .group_opacity(
                                    if s.sidebar_keyboard_controls { 1. } else { 0. },
                                    1.,
                                ),
                        )
                        .on_press_tx(move |s: &mut State, tx| {
                            if s.sidebar_sections.suppress_page_press {
                                s.sidebar_sections.suppress_page_press = false;
                                return;
                            }
                            s.toggle_sidebar_page(&disclosure_id);
                            tx.instant();
                        }),
                    page,
                    source,
                    zones.clone(),
                    viewport,
                ))
            } else {
                draggable(
                    slot.role("button")
                        .label(format!("Change icon for {title}"))
                        .focusable(true)
                        .child(icon)
                        .on_press(move |s: &mut State| {
                            if s.sidebar_sections.suppress_page_press {
                                s.sidebar_sections.suppress_page_press = false;
                                return;
                            }
                            s.open_page_icon_picker(&icon_id, &icon_anchor)
                        }),
                    page,
                    source,
                    zones.clone(),
                    viewport,
                )
            }
        })
        .child(draggable(
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
                    if s.sidebar_sections.suppress_page_press {
                        s.sidebar_sections.suppress_page_press = false;
                        return;
                    }
                    tx.instant();
                    s.open_page(&id, false);
                    if has_children {
                        s.toggle_sidebar_page(&id);
                    }
                }),
            page,
            source,
            zones.clone(),
            viewport,
        ))
        .child(
            row()
                .gap(0.)
                .center_y()
                .child(action(s, menu_key, "Page actions", &MORE, t).on_press(
                    move |s: &mut State| {
                        s.open_sidebar_page_menu_at_anchor(&action_id, &action_anchor, &action_icon)
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
        )
        .children(marker.then(|| {
            el().key("sidebar-page-drop-marker")
                .absolute(0., 0.)
                .w_full()
                .h(2.)
                .fill(t.blue)
        }));
    item = draggable(item, page, source, zones, viewport);
    row().w_full().h(30.).px(7.).center_y().child(item)
}

fn draggable(
    item: El,
    page: &moss_core::Page,
    source: &str,
    zones: Rc<Vec<PageDropZone>>,
    viewport: [f32; 4],
) -> El {
    if source == "recents" {
        return item;
    }
    let drag_id = page.id.clone();
    let drag_source = source.to_owned();
    let drag_title = page.title.clone();
    let drag_icon = page.icon.clone();
    let drag_parent = page.parent_id.clone();
    item.on_drag(move |s: &mut State, drag, tx| {
        if drag.phase == Phase::Down {
            s.sidebar_sections.suppress_page_press = false;
            s.sidebar_sections.page_drag_cancelled = false;
            return;
        }
        if s.sidebar_sections.page_drag_cancelled {
            if drag.phase == Phase::Up || drag.cancelled() {
                s.sidebar_sections.page_drag_cancelled = false;
            }
            return;
        }
        if (drag.dx.abs() >= 4. || drag.dy.abs() >= 4.) && !drag.cancelled() {
            let point = drag.window_point();
            let destination = drop_at(
                &zones,
                viewport,
                point,
                &drag_source,
                drag_parent.as_deref(),
            );
            s.sidebar_sections.suppress_page_press = true;
            s.sidebar_sections.page_drag = Some(crate::sidebar_sections::SidebarPageDrag {
                page_id: drag_id.clone(),
                source: drag_source.clone(),
                target: destination.as_ref().map(|(section, _)| section.clone()),
                before: destination.and_then(|(_, before)| before),
                title: drag_title.clone(),
                icon: drag_icon.clone(),
                point,
                viewport,
            });
            if drag.phase == Phase::Up {
                if let Some(mut drag) = s.sidebar_sections.page_drag.take() {
                    if let Some(target) = drag.target {
                        if drag.source == "private" && target == "private" && drag.before.is_none()
                        {
                            if let Some(index) = zones.iter().position(|z| {
                                point[1] >= z.rect[1] - 2. && point[1] < z.rect[1] + z.rect[3] + 2.
                            }) {
                                if zones[index].page.is_some() {
                                    if let Some(previous) = zones[..=index].iter().rev().find(|z| {
                                        z.section == "private"
                                            && z.page.is_some()
                                            && z.parent == drag_parent
                                    }) {
                                        let next = (|| -> Result<Option<String>, String> {
                                            let page = s
                                                .store
                                                .get_page(previous.page.as_deref().unwrap())
                                                .map_err(|e| e.to_string())?
                                                .ok_or("Page no longer exists")?;
                                            let group = s
                                                .store
                                                .page_children(
                                                    &s.workspace.id,
                                                    drag_parent.as_deref(),
                                                    moss_core::PagePosition::After(
                                                        &moss_core::PageCursor::from(&page),
                                                    ),
                                                    1,
                                                )
                                                .map_err(|e| e.to_string())?;
                                            Ok(group
                                                .items
                                                .into_iter()
                                                .next()
                                                .map(|item| item.page.id))
                                        })();
                                        match next {
                                            Ok(next) => drag.before = next,
                                            Err(error) => {
                                                s.error = Some(format!(
                                                    "Could not reorder page: {error}"
                                                ));
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        s.move_sidebar_page(
                            &drag.page_id,
                            &drag.source,
                            &target,
                            drag.before.as_deref(),
                        );
                    }
                }
            }
        }
        if drag.phase == Phase::Up || drag.cancelled() {
            s.sidebar_sections.page_drag = None;
        }
        tx.instant();
    })
}

fn drop_at(
    zones: &[PageDropZone],
    viewport: [f32; 4],
    point: [f32; 2],
    source: &str,
    parent: Option<&str>,
) -> Option<(String, Option<String>)> {
    let [x, y] = point;
    let [left, top, width, height] = viewport;
    if x < left || x > left + width || y < top || y > top + height {
        return None;
    }
    let index = zones
        .iter()
        .position(|z| y >= z.rect[1] - 2. && y < z.rect[1] + z.rect[3] + 2.)?;
    let zone = &zones[index];
    if zone.section == "recents" {
        return None;
    }
    if source == "private" && zone.section == "private" && zone.page.is_some() {
        let start = index
            + usize::from(
                zone.parent.as_deref() != parent || y >= zone.rect[1] + zone.rect[3] / 2.,
            );
        let before = zones[start..]
            .iter()
            .take_while(|z| z.section == "private")
            .find(|z| z.page.is_some() && z.parent.as_deref() == parent)
            .and_then(|z| z.page.clone());
        return Some((zone.section.clone(), before));
    }
    let before = if zone.page.is_some() && y >= zone.rect[1] + zone.rect[3] / 2. {
        zones
            .get(index + 1)
            .filter(|next| next.section == zone.section)
            .and_then(|next| next.page.clone())
    } else {
        zone.page.clone()
    };
    Some((zone.section.clone(), before))
}

use super::phosphor::{CARET_DOWN as DOWN, CARET_RIGHT as RIGHT, DOTS_THREE as MORE, PLUS};
fn action(s: &State, key: String, name: &str, icon: &'static Icon, t: Theme) -> El {
    super::sidebar_actions::button(s, key, name, icon, 14., t.selected, t)
}
