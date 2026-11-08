use super::theme::Theme;
use crate::state::State;
use moss_core::ViewKind;
use rune::*;
use serde_json::json;

pub fn button(s: &State, t: Theme) -> El {
    let shade = |factor: f32| Color {
        r: t.blue.r * factor,
        g: t.blue.g * factor,
        b: t.blue.b * factor,
        a: t.blue.a,
    };
    row()
        .key("db-new-split")
        .h(32.)
        .center_y()
        .radius(999.)
        .clip_rounded(16.)
        .fill(t.blue)
        .border(1., t.blue)
        .child(
            row()
                .key("db-new")
                .h_full()
                .px(12.)
                .center_y()
                .hover(shade(0.86))
                .press(shade(0.72))
                .role("button")
                .label("New")
                .focusable(true)
                .child(
                    text("New")
                        .font(TextStyle::new(14., 500.))
                        .color(Color::WHITE),
                )
                .on_press(|s: &mut State| {
                    s.database_new_open = false;
                    s.add_record();
                }),
        )
        .child(el().w(1.).h(16.).fill(Color::WHITE.alpha(0.35)))
        .child(
            row()
                .key("db-new-options")
                .size(30., 32.)
                .center()
                .hover(shade(0.86))
                .press(shade(0.72))
                .role("button")
                .label("New item options")
                .expanded(s.database_new_open)
                .focusable(true)
                .child(super::icons::glyph("⌄", 24., Color::WHITE))
                .on_press(|s: &mut State| {
                    s.dismiss_property_picker();
                    if s.property_picker.is_some() {
                        return;
                    }
                    s.gallery_settings_open = false;
                    s.property_visibility_open = false;
                    s.undated_open = false;
                    s.modal = None;
                    s.database_new_open = !s.database_new_open;
                }),
        )
}

pub fn menu(s: &State, t: Theme) -> Option<El> {
    if !s.database_new_open {
        return None;
    }
    let tab = s.tab()?;
    let view = tab.database.as_ref()?.views.get(tab.view)?;
    let mode = view
        .config
        .get("open_mode")
        .and_then(|v| v.as_str())
        .filter(|mode| matches!(*mode, "side" | "center" | "full"))
        .unwrap_or(
            if matches!(view.kind, ViewKind::Gallery | ViewKind::Calendar) {
                "center"
            } else {
                "side"
            },
        );
    let mut body = col()
        .w_full()
        .gap(3.)
        .child(
            text("New database item")
                .font(TextStyle::new(14., 500.))
                .color(t.muted)
                .px(8.)
                .py(5.),
        )
        .child(
            menu_item("db-new-empty", "Empty item", t).on_press(|s: &mut State| {
                s.database_new_open = false;
                s.add_record();
            }),
        )
        .child(el().w_full().h(1.).fill(t.line).my(5.))
        .child(
            text("Open pages in")
                .font(TextStyle::new(14., 500.))
                .color(t.muted)
                .px(8.)
                .py(4.),
        );
    for (value, label) in [
        ("side", "Side peek"),
        ("center", "Center peek"),
        ("full", "Full page"),
    ] {
        let checked = mode == value;
        body = body.child(
            menu_item(format!("db-new-mode-{value}"), label, t)
                .role("menuitemradio")
                .checked(checked)
                .child(el().grow(1.))
                .children(checked.then(|| super::icons::glyph("✓", 14., t.text)))
                .on_press(move |s: &mut State| {
                    save_mode(s, value);
                }),
        );
    }
    body = body.children(s.error.as_ref().map(|error| {
        text(error)
            .font(TextStyle::new(14., 400.))
            .color(t.muted)
            .wrap()
            .px(8.)
            .py(4.)
    }));
    Some(
        col()
            .key("db-new-menu")
            .w(280.)
            .padding(6.)
            .fill(t.menu)
            .border(1., t.line)
            .radius(9.)
            .shadow(4., 16., Color::BLACK.alpha(0.15))
            .role("menu")
            .label("New item options")
            .focus_group()
            .focus_return("db-new-options")
            .modal(true, |s: &mut State| s.database_new_open = false)
            .transition(Transition::opacity())
            .child(body.instant_changes())
            .anchored_fit("db-new-options", Side::Bottom, Align::End, 4.),
    )
}

fn menu_item(key: impl Into<String>, title: &str, t: Theme) -> El {
    row()
        .key(key)
        .w_full()
        .h(30.)
        .px(8.)
        .gap(8.)
        .center_y()
        .radius(5.)
        .hover(t.hover)
        .press(t.selected)
        .role("menuitem")
        .label(title)
        .focusable(true)
        .child(text(title).font(TextStyle::new(14., 400.)).color(t.text))
}

fn save_mode(s: &mut State, mode: &str) {
    let Some(tab) = s.tab() else { return };
    let Some(db) = tab.database.as_ref() else {
        return;
    };
    let Some(view) = db.views.get(tab.view) else {
        return;
    };
    let (db_id, view_id, kind) = (db.id.clone(), view.id.clone(), view.kind);
    match s.store.edit_view_config(&db_id, &view_id, kind, |config| {
        if config.is_null() {
            *config = json!({});
        }
        config
            .as_object_mut()
            .ok_or("invalid view configuration")?
            .insert("open_mode".into(), json!(mode));
        Ok(())
    }) {
        Ok(db) => {
            super::property_visibility::sync_database(s, &db);
            s.error = None;
            s.database_new_open = false;
        }
        Err(e) => s.error = Some(format!("Could not save the opening option: {e}")),
    }
}
