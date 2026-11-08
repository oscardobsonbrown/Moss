use super::theme::Theme;
use crate::state::State;
use moss_core::Database;
use rune::*;
use serde_json::Value;

pub use super::map_canvas::canvas;

pub fn view(s: &State, tab: &crate::state::Tab, db: &Database, t: Theme) -> El {
    let t = Theme {
        family: Family::Sans,
        text_scale: 1.,
        ..t
    };
    let m = &s.database_views.map;
    let mode = db
        .views
        .get(tab.view)
        .and_then(|v| v.config.get("open_mode"))
        .and_then(Value::as_str)
        .unwrap_or("side");
    let map = canvas(&m.points, &m.tiles, m.viewport, t, &db.id, mode, "map")
        .child(super::map_controls::navigation_for_map(s, t))
        .cursor(if m.drag_start.is_some() {
            Cursor::Grabbing
        } else {
            Cursor::Grab
        })
        .on_layout_tx(|s: &mut State, rect, tx| {
            s.database_views.map.resize(rect[2], rect[3]);
            tx.wake_after(std::time::Duration::ZERO);
        })
        .on_drag(|s: &mut State, drag, tx| {
            if drag.phase == Phase::Down {
                let m = &mut s.database_views.map;
                m.cancel_fit();
                m.drag_start = Some(m.viewport);
                m.search_changed();
                s.database_views.map_cluster = None;
                tx.instant();
                return;
            }
            let Some(start) = s.database_views.map.drag_start else {
                return;
            };
            if drag.cancelled() {
                let m = &mut s.database_views.map;
                m.viewport = start;
                m.drag_start = None;
                m.invalidate();
                tx.instant();
                return;
            }
            let m = &mut s.database_views.map;
            m.viewport = start.pan(drag.dx, drag.dy);
            m.invalidate();
            if drag.phase == Phase::Up {
                m.drag_start = None;
                s.map_position_changed();
            }
            tx.instant();
        })
        .on_wheel(|s: &mut State, event, tx| {
            let handled = s.map_wheel(event);
            if handled {
                tx.instant();
            }
            handled
        })
        .on_magnify(|s: &mut State, event, tx| {
            s.map_magnify(event);
            tx.instant();
            true
        });
    let status = if m.fit_pending {
        Some("Fitting all pins…")
    } else if let Some(error) = m.error.as_deref().or(m.tile_error.as_deref()) {
        Some(error)
    } else if m.loading {
        Some("Updating places…")
    } else if m.property.is_none() {
        Some("Choose a location property to show your records.")
    } else if m.truncated {
        Some("More than 500 places match. Zoom in to see the rest.")
    } else {
        None
    };
    col()
        .w_full()
        .min_w(0.)
        .gap(8.)
        .child(map)
        .children(status.map(|status| {
            row()
                .w_full()
                .min_w(0.)
                .center_y()
                .gap(8.)
                .child(
                    t.label(status, if m.error.is_some() { t.error } else { t.muted })
                        .grow(1.)
                        .min_w(0.)
                        .truncate()
                        .help(status),
                )
                .children(m.tile_error.as_ref().map(|_| {
                    super::theme::button("map-tiles-retry", "Retry tiles", t)
                        .on_press(|s: &mut State| s.retry_map_tiles())
                }))
        }))
}
pub fn zoom_button(key: impl Into<String>, label: &str, icon: &'static Icon, t: Theme) -> El {
    row()
        .key(key)
        .size(28., 28.)
        .center()
        .radius(4.)
        .hover(t.hover)
        .cursor(Cursor::Pointer)
        .role("button")
        .label(label)
        .help(label)
        .focusable(true)
        .child(rune::icon(icon, 16.).color(t.muted))
}
