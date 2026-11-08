use moss_core::{Database, Property, PropertyKind};
use rune::{Color, El, Phase, el, row};
use std::rc::Rc;

use super::phosphor::{
    ARROWS_LEFT_RIGHT as RELATION, CALENDAR_BLANK as CALENDAR, CHECK_SQUARE as CHECKBOX,
    CIRCLE_HALF as STATUS, CLOCK as CREATED, CLOCK_COUNTER_CLOCKWISE as EDITED, ENVELOPE as MAIL,
    FILE, FUNCTION, HASH, IDENTIFICATION_CARD, LINK, LIST_CHECKS as TAGS, LIST_NUMBERS as ROLLUP,
    MAP_PIN as PLACE, PHONE, TAG, TEXT_AA, TOGGLE_LEFT as BUTTON, USER as PERSON,
};

pub fn view(kind: PropertyKind, color: Color) -> El {
    use PropertyKind::*;
    let icon = match kind {
        Title | Text => &TEXT_AA,
        Number => &HASH,
        Formula => &FUNCTION,
        UniqueId => &IDENTIFICATION_CARD,
        Select => &TAG,
        MultiSelect => &TAGS,
        Status => &STATUS,
        Date => &CALENDAR,
        Person => &PERSON,
        Files => &FILE,
        Checkbox => &CHECKBOX,
        Url => &LINK,
        Email => &MAIL,
        Phone => &PHONE,
        Relation => &RELATION,
        Rollup => &ROLLUP,
        CreatedTime => &CREATED,
        EditedTime => &EDITED,
        Place => &PLACE,
        Button => &BUTTON,
    };
    rune::icon(icon, 16.).color(color)
}

pub fn metadata_name(
    s: &crate::state::State,
    db: &Database,
    prop: &Property,
    width: f32,
    prefix: &str,
    zones: Rc<Vec<(String, f32)>>,
    t: super::theme::Theme,
) -> El {
    let key = format!("{prefix}-property-name-{}", prop.id);
    let database = db.id.clone();
    let id = prop.id.clone();
    let click_db = database.clone();
    let click_id = id.clone();
    let click_key = key.clone();
    let context_db = database.clone();
    let context_id = id.clone();
    let context_key = key.clone();
    let marker = s
        .metadata_drag
        .as_ref()
        .filter(|dragged| *dragged != &id)
        .and_then(|dragged| {
            if s.metadata_drop_before.as_deref() == Some(id.as_str()) {
                Some(0.)
            } else if s.metadata_drop_before.is_none()
                && zones
                    .iter()
                    .rev()
                    .find(|(other, _)| other != dragged)
                    .is_some_and(|(other, _)| other == &id)
            {
                Some(33.)
            } else {
                None
            }
        });
    row()
        .key(key)
        .w(width)
        .min_w(0.)
        .shrink()
        .h(34.)
        .gap(8.)
        .center_y()
        .radius(4.)
        .hover(t.hover)
        .hover_group()
        .role("button")
        .label(format!("{} property options", prop.name))
        .focusable(true)
        .on_press(move |s: &mut crate::state::State| {
            if s.metadata_suppress_press {
                s.metadata_suppress_press = false;
                return;
            }
            s.open_metadata_menu(&click_db, &click_id, &click_key);
        })
        .on_secondary_press(move |s: &mut crate::state::State, _, _| {
            s.open_metadata_menu(&context_db, &context_id, &context_key)
        })
        .on_drag(move |s: &mut crate::state::State, drag, tx| {
            if drag.phase == Phase::Down {
                s.metadata_suppress_press = false;
                return;
            }
            if drag.dy.abs() >= 4. && !drag.cancelled() {
                s.metadata_drag = Some(id.clone());
                s.metadata_suppress_press = true;
                let y = drag.window_point()[1];
                let before = zones
                    .iter()
                    .filter(|(other, _)| other != &id)
                    .find(|(_, center)| y < *center)
                    .map(|(other, _)| other.clone());
                s.metadata_drop_before = before.clone();
                if drag.phase == Phase::Up {
                    s.move_metadata_property(&database, &id, before.as_deref());
                }
            }
            if drag.phase == Phase::Up || drag.cancelled() {
                s.metadata_drag = None;
                s.metadata_drop_before = None;
            }
            tx.instant();
        })
        .child(
            el().key(format!("{prefix}-property-icon-{}", prop.id))
                .size(16., 20.)
                .center()
                .child(view(prop.kind, t.muted).group_opacity(1., 0.))
                .child(
                    rune::icon(&super::phosphor::DOTS_SIX_VERTICAL, 16.)
                        .key(format!("{prefix}-property-grip-{}", prop.id))
                        .color(t.muted)
                        .absolute(0., 2.)
                        .group_opacity(0., 1.),
                ),
        )
        .child(t.label(&prop.name, t.muted).min_w(0.).grow(1.).truncate())
        .children(marker.map(|y| {
            el().key("metadata-drop-marker")
                .absolute(0., y)
                .w_full()
                .h(2.)
                .fill(t.blue)
        }))
}
#[cfg(test)]
mod tests {
    use crate::{state::State, ui};
    use moss_core::new_id;
    use rune::{Env, Runtime};

    #[test]
    fn table_header_keeps_column_width_and_row_height() {
        let path = std::env::temp_dir().join(format!("moss-table-header-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(true, None);
        let status = state.tab().unwrap().database.as_ref().unwrap().properties[1]
            .id
            .clone();
        let mut rt = Runtime::new(
            state,
            ui::view,
            (1440., 900.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.frame(0.);
        let title = rt.presented("table-header-title").unwrap().0;
        let property = rt.presented(&format!("table-header-{status}")).unwrap().0;
        assert_eq!(title[2], 280.);
        assert_eq!(property[2], 200.);
        assert_eq!(title[3], 38.);
        assert_eq!(property[3], 38.);
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
