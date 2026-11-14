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
    let compact = s.sidebar_collapsed() && !s.sidebar_reveal.visible;
    let width = rune::memo::layout_with("sidebar", |rect| rect[2]).unwrap_or(270.);
    // Keep at least 40 pt for titles after padding and the fixed control lanes.
    let indent = if compact {
        0.
    } else {
        (depth as f32 * 16.).min(160.).min((width - 163.).max(0.))
    };
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
    let drop = s
        .sidebar_sections
        .page_drag
        .as_ref()
        .filter(|drag| drag.target.as_deref() == Some(source));
    let over = drop.is_some_and(|drag| {
        source == "private" && drag.source == "private" && drag.over.as_deref() == Some(id.as_str())
    });
    let inside = over && drop.is_some_and(|drag| drag.inside);
    let marker = if source == "private" && drop.is_some_and(|drag| drag.source == "private") {
        over && !inside
    } else {
        drop.is_some_and(|drag| drag.before.as_deref() == Some(id.as_str()))
    };
    let marker_after = over && drop.is_some_and(|drag| drag.after);
    let mut item = row()
        .key(format!("{prefix}-highlight-{id}"))
        .w_full()
        .h(28.)
        .pl(5. + indent)
        .pr(5.)
        .gap(7.)
        .center_y()
        .radius(7.)
        .fill(if (active && !compact) || inside {
            t.selected
        } else {
            t.sidebar
        })
        .hover(if compact { t.sidebar } else { t.hover })
        .hover_group()
        .hover_motion(Anim::INSTANT, Anim::INSTANT)
        .on_press_tx(move |s: &mut State, tx| {
            if s.sidebar_sections.suppress_page_press {
                s.sidebar_sections.suppress_page_press = false;
                return;
            }
            s.open_page(&row_id, false);
            if has_children && !compact {
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
                .offset(if compact { -8. } else { 0. }, 0.)
                .offset_motion(tok::motion::SNAP_EXIT)
                .center()
                .radius(4.)
                .hover(if compact {
                    Color::TRANSPARENT
                } else {
                    t.selected
                })
                .children(compact.then(|| {
                    let page_id = page.id.clone();
                    el().key(format!("{prefix}-compact-tile-{id}"))
                        .size(26., 26.)
                        .a11y_hidden()
                        .absolute(-3., -3.)
                        .radius(4.)
                        .fill(if active {
                            t.selected
                        } else {
                            Color::TRANSPARENT
                        })
                        .hover(t.hover)
                        .on_press_tx(move |s: &mut State, tx| {
                            s.open_page(&page_id, false);
                            tx.instant();
                        })
                }));
            if compact {
                let page_id = page.id.clone();
                slot.role("button")
                    .label(title)
                    .focusable(true)
                    .child(icon)
                    .on_press_tx(move |s: &mut State, tx| {
                        s.open_page(&page_id, false);
                        tx.instant();
                    })
            } else if has_children {
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
                .focusable(!compact)
                .opacity(if compact { 0. } else { 1. })
                .opacity_motion(tok::motion::SNAP_EXIT)
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
                    if has_children && !compact {
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
                .opacity(if compact { 0. } else { 1. })
                .opacity_motion(tok::motion::SNAP_EXIT)
                .child(
                    action(s, menu_key, "Page actions", &MORE, t)
                        .focusable(!compact)
                        .on_press(move |s: &mut State| {
                            s.open_sidebar_page_menu_at_anchor(
                                &action_id,
                                &action_anchor,
                                &action_icon,
                            )
                        }),
                )
                .child(
                    action(
                        s,
                        format!("{prefix}-add-{child_id}"),
                        "New subpage",
                        &PLUS,
                        t,
                    )
                    .focusable(!compact)
                    .on_press_tx(move |s: &mut State, tx| {
                        s.begin_page(Some(child_id.clone()));
                        tx.instant();
                    }),
                ),
        )
        .children(marker.then(|| {
            el().key("sidebar-page-drop-marker")
                .absolute(0., if marker_after { 26. } else { 0. })
                .w_full()
                .h(2.)
                .fill(t.blue)
        }))
        .children(inside.then(|| el().absolute(0., 4.).w(2.).h(20.).fill(t.blue)));
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
            let destination = drop_at(&zones, viewport, point, &drag_source);
            s.sidebar_sections.suppress_page_press = true;
            s.sidebar_sections.page_drag = Some(crate::sidebar_sections::SidebarPageDrag {
                page_id: drag_id.clone(),
                source: drag_source.clone(),
                target: destination.as_ref().map(|d| d.section.clone()),
                before: destination.as_ref().and_then(|d| d.before.clone()),
                parent: destination.as_ref().and_then(|d| d.parent.clone()),
                over: destination.as_ref().and_then(|d| d.over.clone()),
                inside: destination.as_ref().is_some_and(|d| d.inside),
                after: destination.as_ref().is_some_and(|d| d.after),
                title: drag_title.clone(),
                icon: drag_icon.clone(),
                point,
                viewport,
            });
            if drag.phase == Phase::Up {
                if let Some(mut drop) = s.sidebar_sections.page_drag.take() {
                    if let Some(target) = drop.target.clone() {
                        if drop.source == "private" && target == "private" {
                            // The next sibling can be outside the loaded sidebar page.
                            if drop.after && drop.before.is_none() {
                                if let Some(over) = &drop.over {
                                    let next = (|| -> Result<Option<String>, String> {
                                        let page = s
                                            .store
                                            .get_page(over)
                                            .map_err(|e| e.to_string())?
                                            .ok_or("Page no longer exists")?;
                                        let group = s
                                            .store
                                            .page_children(
                                                &s.workspace.id,
                                                drop.parent.as_deref(),
                                                moss_core::PagePosition::After(
                                                    &moss_core::PageCursor::from(&page),
                                                ),
                                                2,
                                            )
                                            .map_err(|e| e.to_string())?;
                                        Ok(group
                                            .items
                                            .into_iter()
                                            .map(|item| item.page.id)
                                            .find(|id| id != &drop.page_id))
                                    })();
                                    match next {
                                        Ok(next) => drop.before = next,
                                        Err(error) => {
                                            s.error = Some(format!("Could not move page: {error}"));
                                            return;
                                        }
                                    }
                                }
                            }
                            s.reparent_sidebar_page(
                                &drop.page_id,
                                drop.parent.as_deref(),
                                drop.before.as_deref(),
                            );
                        } else {
                            s.move_sidebar_page(
                                &drop.page_id,
                                &drop.source,
                                &target,
                                drop.before.as_deref(),
                            );
                        }
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

struct DropLocation {
    section: String,
    parent: Option<String>,
    before: Option<String>,
    over: Option<String>,
    inside: bool,
    after: bool,
}

fn drop_at(
    zones: &[PageDropZone],
    viewport: [f32; 4],
    point: [f32; 2],
    source: &str,
) -> Option<DropLocation> {
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
    let after = y >= zone.rect[1] + zone.rect[3] / 2.;
    if source == "private" && zone.section == "private" {
        let inside =
            zone.page.is_some() && y >= zone.rect[1] + 7. && y < zone.rect[1] + zone.rect[3] - 7.;
        let parent = if inside {
            zone.page.clone()
        } else {
            zone.parent.clone()
        };
        let before = if inside {
            None
        } else if !after {
            zone.page.clone()
        } else {
            zones[index + 1..]
                .iter()
                .take_while(|z| z.section == "private")
                .find(|z| z.page.is_some() && z.parent == parent)
                .and_then(|z| z.page.clone())
        };
        return Some(DropLocation {
            section: zone.section.clone(),
            parent,
            before,
            over: zone.page.clone(),
            inside,
            after: after && !inside,
        });
    }
    let before = if zone.page.is_some() && after {
        zones
            .get(index + 1)
            .filter(|next| next.section == zone.section)
            .and_then(|next| next.page.clone())
    } else {
        zone.page.clone()
    };
    Some(DropLocation {
        section: zone.section.clone(),
        parent: None,
        before,
        over: zone.page.clone(),
        inside: false,
        after,
    })
}

use super::sidebar_actions::{MORE, RIGHT};
use super::view_icons::{TOOLBAR_CARET as DOWN, TOOLBAR_PLUS as PLUS};
fn action(s: &State, key: String, name: &str, icon: &'static Icon, t: Theme) -> El {
    super::sidebar_actions::button(s, key, name, icon, 14., t.selected, t)
}
