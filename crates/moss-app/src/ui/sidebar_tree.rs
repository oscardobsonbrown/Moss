use super::sidebar_sections::PageDropZone;
use super::theme::{Theme, label};
use crate::{sidebar_tree::SidebarRow, state::State};
use rune::*;

pub fn row_at(s: &State, i: usize, width: f32, t: Theme) -> El {
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
            width,
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

#[allow(clippy::too_many_arguments)]
pub fn page_row(
    s: &State,
    page: &moss_core::Page,
    has_children: bool,
    expanded: bool,
    depth: usize,
    prefix: &str,
    source: &str,
    width: f32,
    t: Theme,
) -> El {
    let active = s.tab().is_some_and(|tab| tab.page.id == page.id);
    let compact = s.sidebar_collapsed() && !s.sidebar_reveal.visible;
    let build = || {
        let id = page.id.clone();
        let title = if page.title.is_empty() {
            "Untitled"
        } else {
            &page.title
        };
        // Keep at least 40 pt for titles after padding and the fixed control lanes.
        let indent = if compact {
            0.
        } else {
            (depth as f32 * 16.).min(160.).min((width - 163.).max(0.))
        };
        let highlight_key = format!("{prefix}-highlight-{id}");
        let highlight_rect = s.sidebar_sections.row_rects.slot(&highlight_key);
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
            source == "private"
                && drag.source == "private"
                && drag.over.as_deref() == Some(id.as_str())
        });
        let inside = over && drop.is_some_and(|drag| drag.inside);
        let marker = if source == "private" && drop.is_some_and(|drag| drag.source == "private") {
            over && !inside
        } else {
            drop.is_some_and(|drag| drag.before.as_deref() == Some(id.as_str()))
        };
        let marker_after = over && drop.is_some_and(|drag| drag.after);
        let active_ink = super::sidebar_actions::active_ink(t);
        let inactive_ink = super::sidebar_actions::inactive_ink(t);
        let press_live = s.sidebar_sections.press_slots.slot(&highlight_key);
        let can_drag = source == "private" || (source != "recents" && depth == 0);
        let row_press_live = press_live.clone();
        let row_press_clear = press_live.clone();
        let row_secondary_clear = press_live.clone();
        let open_press_live = press_live.clone();
        let open_press_clear = press_live.clone();
        let icon_press_live = press_live.clone();
        let icon_press_clear = press_live.clone();
        let disclosure_press_live = press_live.clone();
        let disclosure_press_clear = press_live.clone();
        let actions_press_live = press_live.clone();
        let actions_press_clear = press_live.clone();
        let add_press_live = press_live.clone();
        let add_press_clear = press_live.clone();
        let disclosure_source = source.to_owned();
        let mut item = row()
            .key(highlight_key.clone())
            .drawn_rect_into(&highlight_rect)
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
            .hover(if compact {
                Color::TRANSPARENT
            } else {
                super::sidebar_actions::row_hover(t)
            })
            .hover_group()
            .hover_motion(Anim::INSTANT, Anim::INSTANT)
            .children((!compact).then(|| {
                el().key(format!("{highlight_key}-press"))
                    .pin(Some(0.), Some(0.), Some(0.), Some(0.))
                    .radius(7.)
                    .fill(super::sidebar_actions::row_press(t))
                    .opacity(0.)
                    .live_opacity(&press_live)
                    .inert()
            }))
            .on_press_tx(move |s: &mut State, tx| {
                set_press(&row_press_clear, false);
                if s.sidebar_sections.suppress_page_press {
                    s.sidebar_sections.suppress_page_press = false;
                    return;
                }
                s.open_page(&row_id, false);
                tx.instant();
            })
            .role("group")
            .on_secondary_press(move |s: &mut State, point, _| {
                set_press(&row_secondary_clear, false);
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
                        super::sidebar_actions::button_hover(t)
                    })
                    .hover_motion(Anim::INSTANT, Anim::INSTANT)
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
                            .hover(super::sidebar_actions::row_hover(t))
                            .hover_motion(Anim::INSTANT, Anim::INSTANT)
                            .press(super::sidebar_actions::row_press(t))
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
                            .radius(4.)
                            .press(super::sidebar_actions::button_press(t))
                            .role("button")
                            .label(if expanded {
                                "Collapse page"
                            } else {
                                "Expand page"
                            })
                            .help(if expanded { "Close" } else { "Open" })
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
                                set_press(&disclosure_press_clear, false);
                                if s.sidebar_sections.suppress_page_press {
                                    s.sidebar_sections.suppress_page_press = false;
                                    return;
                                }
                                toggle_page_in_source(s, &disclosure_source, &disclosure_id);
                                tx.instant();
                            }),
                        page,
                        source,
                        can_drag,
                        disclosure_press_live,
                        highlight_rect.clone(),
                    ))
                } else {
                    draggable(
                        slot.role("button")
                            .label(format!("Change icon for {title}"))
                            .help("Change icon")
                            .press(super::sidebar_actions::button_press(t))
                            .focusable(true)
                            .child(icon)
                            .on_press(move |s: &mut State| {
                                set_press(&icon_press_clear, false);
                                if s.sidebar_sections.suppress_page_press {
                                    s.sidebar_sections.suppress_page_press = false;
                                    return;
                                }
                                s.open_page_icon_picker(&icon_id, &icon_anchor)
                            }),
                        page,
                        source,
                        can_drag,
                        icon_press_live,
                        highlight_rect.clone(),
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
                        text(title)
                            .font(
                                TextStyle::new(14., if active { 500. } else { 400. })
                                    .family(Family::Sans),
                            )
                            .color(if active { active_ink } else { inactive_ink })
                            .key(format!("{prefix}-title-{id}"))
                            .truncate()
                            .grow(1.)
                            .min_w(0.),
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        set_press(&open_press_clear, false);
                        if s.sidebar_sections.suppress_page_press {
                            s.sidebar_sections.suppress_page_press = false;
                            return;
                        }
                        tx.instant();
                        s.open_page(&id, false);
                    }),
                page,
                source,
                can_drag,
                open_press_live,
                highlight_rect.clone(),
            ))
            .child(
                row()
                    .key(format!("{prefix}-trailing-{}", page.id))
                    .gap(0.)
                    .center_y()
                    .opacity(if compact { 0. } else { 1. })
                    .opacity_motion(tok::motion::SNAP_EXIT)
                    .child(track_row_press(
                        action(s, menu_key, "Page actions", &MORE, t)
                            .help("Remove, rename and more...")
                            .focusable(!compact)
                            .on_press(move |s: &mut State| {
                                set_press(&actions_press_clear, false);
                                s.open_sidebar_page_menu_at_anchor(
                                    &action_id,
                                    &action_anchor,
                                    &action_icon,
                                )
                            }),
                        actions_press_live,
                        highlight_rect.clone(),
                    ))
                    .child(track_row_press(
                        action(
                            s,
                            format!("{prefix}-add-{child_id}"),
                            "New subpage",
                            &PLUS,
                            t,
                        )
                        .help("Add a page inside")
                        .focusable(!compact)
                        .on_press_tx(move |s: &mut State, tx| {
                            set_press(&add_press_clear, false);
                            s.begin_page(Some(child_id.clone()));
                            tx.instant();
                        }),
                        add_press_live,
                        highlight_rect.clone(),
                    )),
            )
            .children(marker.then(|| {
                el().key("sidebar-page-drop-marker")
                    .absolute(0., if marker_after { 26. } else { 0. })
                    .w_full()
                    .h(2.)
                    .fill(t.blue)
            }))
            .children(inside.then(|| el().absolute(0., 4.).w(2.).h(20.).fill(t.blue)));
        item = draggable(item, page, source, can_drag, row_press_live, highlight_rect);
        row().w_full().h(30.).px(7.).center_y().child(item)
    };
    if s.sidebar_sections.page_drag.is_none() {
        rune::memo(
            format!("sidebar-page-row-memo-{prefix}-{source}-{}", page.id),
            (
                (&page.title, &page.icon),
                (has_children, expanded, depth, active),
                (compact, width.to_bits(), s.sidebar_keyboard_controls),
                (
                    t.dark,
                    t.bg.r.to_bits(),
                    t.text_scale.to_bits(),
                    t.scale.to_bits(),
                ),
            ),
            build,
        )
    } else {
        build()
    }
}

fn set_press(live: &rune::Live<f32>, active: bool) {
    let target = if active { 1. } else { 0. };
    if (live.get() - target).abs() > 0.001 {
        live.set(target);
    }
}

fn toggle_page_in_source(s: &mut State, source: &str, id: &str) {
    match source {
        "private" => s.toggle_sidebar_page(id),
        "favorites" => s.toggle_sidebar_favorite_page(id),
        "recents" => {}
        section => s.toggle_sidebar_shortcut_page(section, id),
    }
}

fn point_in_rect(point: [f32; 2], rect: [f32; 4]) -> bool {
    rect[2] <= 0.
        || (point[0] >= rect[0]
            && point[0] <= rect[0] + rect[2]
            && point[1] >= rect[1]
            && point[1] <= rect[1] + rect[3])
}

fn track_row_press(item: El, press_live: rune::Live<f32>, row_rect: rune::Live<[f32; 4]>) -> El {
    item.on_drag(move |_: &mut State, drag, tx| {
        if drag.cancelled() || drag.phase == Phase::Up {
            set_press(&press_live, false);
        } else if drag.phase == Phase::Down {
            set_press(&press_live, true);
        } else if drag.phase == Phase::Move {
            set_press(
                &press_live,
                point_in_rect(drag.window_point(), row_rect.get()),
            );
        }
        tx.live_only();
    })
}

fn draggable(
    item: El,
    page: &moss_core::Page,
    source: &str,
    can_drag: bool,
    press_live: rune::Live<f32>,
    row_rect: rune::Live<[f32; 4]>,
) -> El {
    if !can_drag {
        return track_row_press(item, press_live, row_rect);
    }
    let drag_id = page.id.clone();
    let drag_source = source.to_owned();
    let drag_title = page.title.clone();
    let drag_icon = page.icon.clone();
    item.on_drag(move |s: &mut State, drag, tx| {
        if drag.phase == Phase::Down {
            s.sidebar_sections.suppress_page_press = false;
            s.sidebar_sections.page_drag_cancelled = false;
            set_press(&press_live, true);
            tx.live_only();
            return;
        }
        if s.sidebar_sections.page_drag_cancelled {
            if drag.phase == Phase::Up || drag.cancelled() {
                s.sidebar_sections.page_drag_cancelled = false;
                set_press(&press_live, false);
            }
            tx.live_only();
            return;
        }
        if (drag.dx.abs() >= 4. || drag.dy.abs() >= 4.) && !drag.cancelled() {
            set_press(&press_live, false);
            let point = drag.window_point();
            let zones = super::sidebar_sections::page_drop_zones(s);
            let viewport = s.sidebar_sections.viewport_rect.get();
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
            if drag.phase == Phase::Up || drag.cancelled() {
                s.sidebar_sections.page_drag = None;
            }
            tx.instant();
            return;
        }
        if drag.phase == Phase::Move {
            set_press(
                &press_live,
                point_in_rect(drag.window_point(), row_rect.get()),
            );
        }
        if drag.phase == Phase::Up || drag.cancelled() {
            set_press(&press_live, false);
            if s.sidebar_sections.page_drag.take().is_some() {
                tx.instant();
                return;
            }
        }
        tx.live_only();
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
    super::sidebar_actions::button(
        s,
        key,
        name,
        icon,
        14.,
        super::sidebar_actions::button_hover(t),
        t,
    )
}
