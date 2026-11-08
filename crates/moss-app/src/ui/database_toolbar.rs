use super::theme::{Theme, button};
use crate::state::State;
use moss_core::ViewKind;
use rune::*;

fn text_width(value: &str) -> f32 {
    rune::text::measure(
        value,
        &[],
        TextStyle::new(14., 400.).family(Family::Sans),
        None,
    )
    .0
}

fn fitted_tabs(widths: &[f32], active: usize, available: f32) -> Vec<usize> {
    let required = |indices: &[usize], overflow: bool| {
        indices.iter().map(|i| widths[*i]).sum::<f32>()
            + 4. * indices.len().saturating_sub(1) as f32
            + 32.
            + if overflow {
                text_width(&format!("{} more…", widths.len() - indices.len())) + 22.
            } else {
                0.
            }
    };
    let all: Vec<_> = (0..widths.len()).collect();
    if required(&all, false) <= available {
        return all;
    }
    if widths.is_empty() {
        return all;
    }
    let mut visible = vec![active.min(widths.len() - 1)];
    for index in 0..widths.len() {
        if visible.contains(&index) {
            continue;
        }
        let mut next = visible.clone();
        next.push(index);
        next.sort_unstable();
        if required(&next, next.len() < widths.len()) <= available {
            visible = next;
        }
    }
    visible
}

pub fn view(s: &State, t: Theme) -> El {
    let t = Theme {
        family: Family::Sans,
        text_scale: 1.,
        ..t
    };
    let Some(tab) = s.tab() else { return row() };
    let Some(db) = tab.database.as_ref() else {
        return row();
    };
    let kind = db.views.get(tab.view).map(|v| v.kind);
    let is_gallery = kind == Some(ViewKind::Gallery);
    let is_list = kind == Some(ViewKind::List);
    let width = rune::memo::layout_with("db-toolbar", |r| r[2]).unwrap_or(360.);
    let action_width = 140.
        + 32.
        + text_width("New")
        + 55.
        + 18.
        + if is_gallery {
            text_width("Layout") + 24.
        } else {
            0.
        }
        + if is_list {
            text_width("Properties") + 24.
        } else {
            0.
        };
    let widths: Vec<_> = db
        .views
        .iter()
        .map(|v| (text_width(&v.name) + 51.).clamp(70., 160.))
        .collect();
    let minimum_lane = widths.get(tab.view).copied().unwrap_or(70.)
        + 32.
        + if widths.len() > 1 {
            text_width(&format!("{} more…", widths.len() - 1)) + 22.
        } else {
            0.
        };
    let stacked = width - action_width - 6. < minimum_lane;
    let lane_width = if stacked {
        width
    } else {
        width - action_width - 6.
    };
    let indices = fitted_tabs(&widths, tab.view, lane_width);
    let hidden = db.views.len().saturating_sub(indices.len());
    let mut tabs = row().key("db-view-tabs").gap(4.).center_y().min_w(0.);
    for index in indices {
        let item = &db.views[index];
        let active = index == tab.view;
        tabs = tabs.child(
            row()
                .key(format!("db-view-{index}"))
                .w(widths[index])
                .h(32.)
                .px(14.)
                .gap(7.)
                .center_y()
                .radius(999.)
                .fill(if active {
                    t.selected
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .role("tab")
                .label(&item.name)
                .focusable(true)
                .focus_selected(active)
                .child(super::view_icons::element(item.kind, 16., t.muted))
                .child(
                    t.label(&item.name, if active { t.text } else { t.muted })
                        .truncate()
                        .min_w(0.)
                        .grow(1.),
                )
                .on_press_tx(move |s: &mut State, tx| {
                    s.select_database_view(index);
                    tx.instant();
                }),
        );
    }
    if hidden > 0 {
        tabs = tabs.child(
            button("db-views", format!("{hidden} more…"), t)
                .on_press(|s: &mut State| s.open_view_chooser()),
        );
    }
    tabs = tabs.child(
        row()
            .key("db-add-view")
            .size(28., 32.)
            .center()
            .radius(5.)
            .hover(t.hover)
            .role("button")
            .label("Add view")
            .focusable(true)
            .child(super::icons::glyph("+", 18., t.muted))
            .on_press(|s: &mut State| s.open_new_view()),
    );
    let search = super::inputs::field(&s.filter, "db-filter", "Search records", t)
        .w(140.)
        .label("Search records")
        .on_edit(|s: &mut State| &mut s.filter)
        .on_input_change(|s: &mut State, tx| {
            s.move_record_page(0);
            tx.instant();
        });
    let actions = row()
        .key("db-view-actions")
        .gap(6.)
        .center_y()
        .flex_wrap()
        .min_w(0.)
        .child(search)
        .child(
            row()
                .key("db-view-settings")
                .size(32., 32.)
                .center()
                .radius(6.)
                .hover(t.hover)
                .role("button")
                .label("View settings")
                .focusable(true)
                .child(rune::icon(&super::view_icons::SETTINGS, 18.).color(t.muted))
                .on_press(|s: &mut State| s.open_view_settings()),
        )
        .children(is_list.then(|| {
            button("list-properties-button", "Properties", t)
                .on_press(|s: &mut State| super::property_visibility::open(s))
        }))
        .children(is_gallery.then(|| {
            button("gallery-settings-button", "Layout", t)
                .on_press(|s: &mut State| s.gallery_settings_open = !s.gallery_settings_open)
        }))
        .child(super::database_new::button(s, t));
    if stacked {
        col()
            .key("db-toolbar")
            .w_full()
            .gap(6.)
            .child(tabs.w_full())
            .child(actions.w_full())
    } else {
        row()
            .key("db-toolbar")
            .w_full()
            .gap(6.)
            .center_y()
            .child(tabs.grow(1.).min_w(0.))
            .child(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::new_id;

    #[test]
    fn view_overflow_tracks_available_width_and_keeps_active_view() {
        let path = std::env::temp_dir().join(format!("moss-view-overflow-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(true, None);
        let active = state.active;
        state.tabs[active].page.settings.full_width = true;
        let mut rt = Runtime::new(
            state,
            crate::ui::view,
            (1800., 700.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.frame(0.);
        rt.frame(0.);
        let count = rt
            .state
            .tab()
            .unwrap()
            .database
            .as_ref()
            .unwrap()
            .views
            .len();
        assert!(
            rt.find("db-views").is_none(),
            "all tabs should fit without a more button"
        );
        for index in 0..count {
            assert!(rt.find(&format!("db-view-{index}")).is_some());
        }
        rt.set_viewport((850., 700.), 1.);
        rt.state.select_database_view(count - 1);
        rt.invalidate();
        rt.frame(0.);
        rt.frame(0.);
        assert!(rt.find(&format!("db-view-{}", count - 1)).is_some());
        let elements = rt.elements();
        let shown = (0..count)
            .filter(|i| rt.find(&format!("db-view-{i}")).is_some())
            .count();
        let more = elements
            .iter()
            .find(|e| e.key.as_deref() == Some("db-views"))
            .unwrap();
        assert_eq!(more.label, Some(format!("{} more…", count - shown)));
        for key in ["db-view-settings", "db-add-view", "db-new", "db-filter"] {
            let e = elements
                .iter()
                .find(|e| e.key.as_deref() == Some(key))
                .unwrap();
            assert!(
                e.visible[2] > 0. && e.rect[0] + e.rect[2] <= 850.1,
                "{key}: {:?}",
                e.rect
            );
        }
        rt.set_viewport((2000., 700.), 1.);
        rt.frame(0.);
        rt.frame(0.);
        assert!(rt.find("db-views").is_none());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn compact_toolbar_keeps_controls_visible_and_searches_all_views() {
        let path = std::env::temp_dir().join(format!("moss-toolbar-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        let gallery = s
            .tab()
            .unwrap()
            .database
            .as_ref()
            .unwrap()
            .views
            .iter()
            .position(|v| v.kind == ViewKind::Gallery)
            .unwrap();
        assert!(s.select_database_view(gallery));
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (850., 700.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.on_key = Some(crate::key);
        rt.frame(0.);
        let elements = rt.elements();
        let element = |key: &str| {
            elements
                .iter()
                .find(|e| e.key.as_deref() == Some(key))
                .unwrap()
        };
        let more = element("db-views");
        let new = element("db-new");
        let layout = element("gallery-settings-button");
        let shown = elements
            .iter()
            .filter(|e| {
                e.key.as_deref().is_some_and(|k| {
                    k.strip_prefix("db-view-")
                        .is_some_and(|s| s.parse::<usize>().is_ok())
                })
            })
            .count();
        assert_eq!(
            more.label.as_deref(),
            Some(
                format!(
                    "{} more…",
                    rt.state
                        .tab()
                        .unwrap()
                        .database
                        .as_ref()
                        .unwrap()
                        .views
                        .len()
                        - shown
                )
                .as_str()
            )
        );
        assert!(more.visible[2] > 0. && new.visible[2] > 0. && layout.visible[2] > 0.);
        let follows = |a: &ElementInfo, b: &ElementInfo| {
            a.rect[1] + a.rect[3] <= b.rect[1] + 0.1 || a.rect[0] + a.rect[2] <= b.rect[0] + 0.1
        };
        assert!(follows(more, layout));
        assert!(follows(layout, new));
        let (x, y) = rt.find("db-views").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 120.);
        assert!(rt.find("view-search").is_some());
        rt.key(KeyEvent {
            key: Key::Text("Timeline".into()),
            mods: Mods::default(),
        });
        rt.frame(1. / 120.);
        rt.key(KeyEvent {
            key: Key::Enter,
            mods: Mods::default(),
        });
        rt.frame(1. / 120.);
        let tab = rt.state.tab().unwrap();
        assert_eq!(
            tab.database.as_ref().unwrap().views[tab.view].kind,
            ViewKind::Timeline
        );
        assert!(rt.state.modal.is_none());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
