use std::hash::{Hash, Hasher};
use std::time::Duration;

use crate::state::State;
use rune::{TextEdit, Tx};

const INTERVAL: f64 = 0.5;

pub struct CaretBlink {
    pub visible: bool,
    window_focused: bool,
    fingerprint: Option<u64>,
    changed_at: f64,
}
impl Default for CaretBlink {
    fn default() -> Self {
        Self {
            visible: true,
            window_focused: true,
            fingerprint: None,
            changed_at: 0.,
        }
    }
}
impl CaretBlink {
    pub fn set_window_focused(&mut self, focused: bool) {
        self.window_focused = focused;
        self.fingerprint = None;
        self.visible = true;
    }
}

fn focused(s: &State) -> Option<(u64, bool)> {
    let kind = s
        .tab()
        .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
        .map(|v| v.kind);
    let map = kind == Some(moss_core::ViewKind::Map);
    let dashboard = kind == Some(moss_core::ViewKind::Dashboard);
    let fields = [
        (true, "simple-table-cell", &s.simple_table.input),
        (map, "map-search", &s.database_views.map.place_query),
        (map, "map-name", &s.database_views.map.place_name),
        (map, "map-latitude", &s.database_views.map.latitude),
        (map, "map-longitude", &s.database_views.map.longitude),
        (map, "map-record-search", &s.database_views.map.record_query),
        (
            dashboard,
            "dashboard-filter",
            &s.database_views.dashboard.filter_input,
        ),
        (
            dashboard,
            "dashboard-source",
            &s.database_views.dashboard.source_input,
        ),
        (
            s.sidebar_section_menu.is_some(),
            "sidebar-section-name",
            &s.sidebar_section_name,
        ),
        (
            s.sidebar_section_menu.is_some(),
            "sidebar-section-search",
            &s.sidebar_section_search,
        ),
        (
            s.block_commands.is_some(),
            "block-command-search",
            &s.block_command_search,
        ),
        (
            s.page_icon_picker.is_some(),
            "page-icon-search",
            &s.page_icon_search,
        ),
        (
            s.sidebar_page_menu.is_some(),
            "sidebar-name",
            &s.sidebar_page_name,
        ),
        (s.undated_open, "undated", &s.undated_search),
        (
            s.modal == Some(crate::state::Modal::Search),
            "search",
            &s.search,
        ),
        (
            s.modal == Some(crate::state::Modal::Views),
            "view-search",
            &s.view_search,
        ),
        (
            s.modal.is_none() && s.tab().is_some_and(|tab| tab.database.is_some()),
            "filter",
            &s.filter,
        ),
        (
            s.modal.is_none() && s.editing_cell.is_some() && s.property_picker().is_none(),
            "cell",
            &s.cell,
        ),
    ];
    fn fingerprint(id: &str, edit: &TextEdit) -> (u64, bool) {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut h);
        edit.hash(&mut h);
        (h.finish(), edit.preedit.is_some())
    }
    if let Some(editor) = &s.database_property_editor {
        for (id, edit) in [
            ("property-name", &editor.name_input),
            ("property-options", &editor.options_input),
            ("table-filter", &editor.filter_input),
            ("formula-expression", &editor.advanced.expression),
            ("relation-database-search", &editor.advanced.database_search),
        ] {
            if edit.focused {
                return Some(fingerprint(id, edit));
            }
        }
    }
    if let Some(visibility) = &s.property_visibility {
        if visibility.search.focused {
            return Some(fingerprint("visibility", &visibility.search));
        }
    }
    if let Some(picker) = &s.cell_picker {
        match picker {
            crate::property_controls::CellPicker::Property(picker) => {
                if let Some(editor) = &picker.option_editor {
                    if editor.name_input.focused {
                        return Some(fingerprint("picker-option-name", &editor.name_input));
                    }
                }
                if picker.input.focused {
                    return Some(fingerprint("picker", &picker.input));
                }
                if let Some(date) = &picker.date {
                    for (id, edit) in [
                        ("date-end", &date.end_input),
                        ("date-start-time", &date.start_time_input),
                        ("date-end-time", &date.end_time_input),
                    ] {
                        if edit.focused {
                            return Some(fingerprint(id, edit));
                        }
                    }
                }
            }
            crate::property_controls::CellPicker::Relation(picker) => {
                if picker.search.focused {
                    return Some(fingerprint("relation-search", &picker.search));
                }
            }
            crate::property_controls::CellPicker::Place(_) => {
                if s.database_views.map.place_query.focused {
                    return Some(fingerprint("place-search", &s.database_views.map.place_query));
                }
            }
        }
    }
    if let Some(cover) = &s.page_cover.editor {
        if cover.gradient_picker {
            for (id, edit) in [
                ("cover-gradient-start", &cover.gradient_start),
                ("cover-gradient-end", &cover.gradient_end),
            ] {
                if edit.focused {
                    return Some(fingerprint(id, edit));
                }
            }
        }
    }
    if let Some((_, id, edit)) = fields
        .into_iter()
        .find(|(visible, _, edit)| *visible && edit.focused)
    {
        return Some(fingerprint(id, edit));
    }
    if s.modal.is_some() {
        return None;
    }
    if let Some(peek) = &s.record_peek {
        if peek.title.focused {
            return Some(fingerprint(
                &format!("peek-title-{}", peek.page.id),
                &peek.title,
            ));
        }
        if let Some(block) = peek.blocks.iter().find(|b| b.edit.focused) {
            return Some(fingerprint(&block.block.id, &block.edit));
        }
    }
    let tab = s.tab()?;
    if tab.title.focused {
        return Some(fingerprint(&format!("title-{}", tab.page.id), &tab.title));
    }
    tab.blocks
        .iter()
        .find(|b| b.edit.focused)
        .map(|block| fingerprint(&block.block.id, &block.edit))
}

pub fn poll(s: &mut State, time: f64, tx: &mut Tx) -> bool {
    let current = if s.caret.window_focused {
        focused(s)
    } else {
        None
    };
    let old = s.caret.visible;
    match current {
        None => {
            s.caret.fingerprint = None;
            s.caret.visible = true;
        }
        Some((fingerprint, composing)) => {
            if s.caret.fingerprint != Some(fingerprint) || composing {
                s.caret.fingerprint = Some(fingerprint);
                s.caret.changed_at = time;
            }
            let elapsed = (time - s.caret.changed_at).max(0.);
            s.caret.visible = composing || ((elapsed / INTERVAL).floor() as u64).is_multiple_of(2);
            if !composing {
                let remaining = INTERVAL - elapsed.rem_euclid(INTERVAL);
                tx.wake_after(Duration::from_secs_f64(remaining.max(0.001)));
            }
        }
    }
    old != s.caret.visible
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::new_id;
    use rune::{Env, Runtime};

    fn painted(rt: &Runtime<State>) -> bool {
        let caret = rt
            .elements()
            .iter()
            .find(|e| e.key.as_deref() == Some("property-name-input"))
            .and_then(|e| e.caret)
            .unwrap();
        let drawn = [
            caret[0].round(),
            caret[1] + caret[3] * 0.1,
            2.,
            caret[3] * 0.8,
        ];
        rt.instances
            .iter()
            .any(|i| i.rect.iter().zip(drawn).all(|(a, b)| (*a - b).abs() < 0.2) && i.fill[3] > 0.9)
    }
    #[test]
    fn property_caret_blinks_and_resets_for_selection_and_focus_changes() {
        for selected in [false, true] {
            let path = std::env::temp_dir().join(format!("moss-caret-{}", new_id()));
            let mut state = State::open(path.clone(), None).unwrap();
            state.new_page(true, None);
            let id = state.tab().unwrap().database.as_ref().unwrap().properties[1]
                .id
                .clone();
            state.open_database_property(Some(&id));
            if selected {
                let edit = &mut state.database_property_editor.as_mut().unwrap().name_input;
                let len = edit.text.len();
                edit.set_selection(0, len);
            } else {
                state
                    .database_property_editor
                    .as_mut()
                    .unwrap()
                    .name_input
                    .set_caret(0);
            }
            let mut rt = Runtime::new(
                state,
                crate::ui::view,
                (1440., 900.),
                Env {
                    system_dark: false,
                    scale: 1.,
                },
            );
            rt.poll = Some(crate::records::poll);
            rt.frame(0.);
            rt.frame(0.2);
            assert!(painted(&rt));
            assert!(rt.wake_deadline().is_some());
            rt.frame(0.51);
            assert!(!painted(&rt));
            rt.frame(0.5);
            assert!(painted(&rt));
            rt.frame(0.5);
            assert!(!painted(&rt));
            rt.update(|s, _| {
                s.database_property_editor
                    .as_mut()
                    .unwrap()
                    .name_input
                    .set_caret(1)
            });
            rt.frame(0.);
            assert!(painted(&rt));
            rt.update(|s, _| {
                s.database_property_editor
                    .as_mut()
                    .unwrap()
                    .name_input
                    .preedit = Some(("composition".into(), None))
            });
            rt.frame(0.);
            rt.frame(1.5);
            assert!(painted(&rt));
            rt.update(|s, _| {
                s.database_property_editor
                    .as_mut()
                    .unwrap()
                    .name_input
                    .preedit = None
            });
            rt.frame(0.);
            rt.update(|s, _| s.caret.set_window_focused(false));
            rt.frame(0.);
            assert!(rt.state.caret.visible);
            rt.frame(1.);
            assert!(rt.wake_deadline().is_none());
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}
