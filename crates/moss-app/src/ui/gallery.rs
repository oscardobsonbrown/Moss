use super::theme::{Theme, button};
use crate::state::{State, Tab};
use moss_core::{Database, PageCover, PropertyKind, Record, ViewKind};
use rune::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CardSize {
    Small,
    Medium,
    Large,
}
impl CardSize {
    fn from_view(config: &Value) -> Self {
        match config.get("gallery_card_size").and_then(Value::as_str) {
            Some("small") => Self::Small,
            Some("large") => Self::Large,
            _ => Self::Medium,
        }
    }
    fn dimensions(self) -> (f32, f32) {
        match self {
            Self::Small => (220., 125.),
            Self::Medium => (346., 192.),
            // shortcut: Large geometry is provisional until a reference capture is available.
            Self::Large => (460., 256.),
        }
    }
}

fn page_preview(config: &Value) -> bool {
    matches!(
        config.get("gallery_preview").and_then(Value::as_str),
        None | Some("page_content") | Some("page_cover")
    )
}

pub fn view(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let Some(view) = db.views.get(tab.view) else {
        return col();
    };
    let (width, preview_height) = CardSize::from_view(&view.config).dimensions();
    let preview = page_preview(&view.config);
    let properties = moss_core::record_layout::properties(db);
    let mut grid = row()
        .key("gallery-grid")
        .w_full()
        .min_w(0.)
        .flex_wrap()
        .items_start()
        .gap(12.);
    for (index, record) in tab.records.iter().enumerate() {
        grid = grid.child(card(
            s,
            tab,
            db,
            &properties,
            record,
            tab.gallery_previews
                .get(&record.id)
                .map(String::as_str)
                .unwrap_or(""),
            index,
            width,
            preview_height,
            preview,
            t,
        ));
    }
    let tile_height = if preview { preview_height + 40. } else { 40. };
    grid.child(
        row()
            .key("gallery-new")
            .w(width)
            .h(tile_height)
            .center()
            .border(1., t.line)
            .radius(10.)
            .hover(t.hover)
            .role("button")
            .label("New page")
            .focusable(true)
            .gap(4.)
            .child(super::icons::glyph("+", 14., t.muted))
            .child(t.label("New page", t.muted))
            .on_press(|s: &mut State| s.create_peek(BTreeMap::new())),
    )
}

fn card(
    s: &State,
    tab: &Tab,
    db: &Database,
    properties: &[&moss_core::Property],
    record: &Record,
    preview_text: &str,
    index: usize,
    width: f32,
    preview_height: f32,
    preview: bool,
    t: Theme,
) -> El {
    let mut content = col()
        .key(format!("record-card-{index}"))
        .w(width)
        .min_w(0.)
        .border(1., t.line)
        .radius(10.)
        .clip_rounded(10.)
        .fill(t.bg)
        .hover(t.hover)
        .role("button")
        .label(&record.title)
        .focusable(true);
    if preview {
        let cover = tab.gallery_covers.get(&record.id);
        let cover_mode = db.views[tab.view]
            .config
            .get("gallery_preview")
            .and_then(Value::as_str)
            == Some("page_cover");
        let show_cover = cover_mode || (preview_text.trim().is_empty() && cover.is_some());
        let mut preview_frame = col()
            .key(format!("gallery-preview-{index}"))
            .w_full()
            .h(preview_height)
            .clip_rounded(0.)
            .fill(t.sidebar);
        if show_cover {
            if let Some(cover) = cover {
                match cover {
                    PageCover::Color { color } => {
                        if let Some(color) = super::page_cover::parse_color(color) {
                            preview_frame = preview_frame.fill(color);
                        }
                    }
                    PageCover::Gradient { start, end } => {
                        if let (Some(a), Some(b)) = (
                            super::page_cover::parse_color(start),
                            super::page_cover::parse_color(end),
                        ) {
                            preview_frame = preview_frame.gradient(a, b);
                        }
                    }
                    PageCover::Image { asset, position } => {
                        let id = record.id.clone();
                        let drawn_asset = asset.clone();
                        preview_frame =
                            preview_frame.on_drawn_layout_tx(move |s: &mut State, rect, tx| {
                                s.page_cover
                                    .gallery_covers
                                    .insert(id.clone(), (drawn_asset.clone(), rect));
                                tx.live_only();
                            });
                        if let Some(image) = s.page_cover.images.get(asset) {
                            if let Some([iw, ih]) = image.size() {
                                let scale = (width / iw as f32).max(preview_height / ih as f32);
                                let iw = iw as f32 * scale;
                                let ih = ih as f32 * scale;
                                preview_frame = preview_frame.child(
                                    el().key(format!("gallery-cover-image-{index}"))
                                        .absolute(
                                            (width - iw) / 2.,
                                            -(ih - preview_height) * position.clamp(0., 1.),
                                        )
                                        .size(iw, ih)
                                        .pixels(image.pixels()),
                                );
                            }
                        }
                    }
                }
            }
        } else {
            preview_frame = preview_frame
                .padding(18.)
                .child(t.label(preview_text, t.muted).w_full().wrap());
        }
        content = content.child(preview_frame);
    }
    content = content.child(
        row()
            .w_full()
            .min_h(40.)
            .px(12.)
            .py(10.)
            .gap(6.)
            .items_start()
            .child(super::record_icon::view(
                tab,
                record,
                format!("record-icon-gallery-{index}"),
                t,
            ))
            .child(
                text(if record.title.is_empty() {
                    "Untitled"
                } else {
                    &record.title
                })
                .key(format!("gallery-title-{index}"))
                .font(TextStyle::new(14. * t.text_scale, 600.).family(t.family))
                .color(t.text)
                .min_w(0.)
                .grow(1.)
                .wrap(),
            ),
    );
    for property in properties
        .iter()
        .copied()
        .filter(|p| super::property_visibility::visible(db, tab.view, &p.id))
    {
        let value = record.values.get(&property.id);
        if property.kind == PropertyKind::Place || value.is_some_and(|value| !value.is_null()) {
            let mut property_row = row()
                .w_full()
                .px(12.)
                .pb(6.)
                .gap(6.)
                .center_y()
                .child(t.label(format!("{}:", property.name), t.muted));
            let label = if value.is_none_or(Value::is_null) {
                t.label("Empty", t.faint)
            } else {
                super::property_value::view(s, property, value, t)
            };
            property_row = property_row.child(label);
            if property.kind == PropertyKind::Place {
                let record_id = record.id.clone();
                let property_id = property.id.clone();
                property_row = property_row
                    .key(format!("gallery-value-{index}-{property_id}"))
                    .role("button")
                    .label(format!("Edit {}", property.name))
                    .focusable(true)
                    .on_press(move |s: &mut State| s.open_place_picker(&record_id, &property_id));
            } else {
                property_row = property_row.key(format!("gallery-value-{index}-{}", property.id));
            }
            content = content.child(property_row);
        }
    }
    content.on_press(move |s: &mut State| s.open_record(index))
}

fn selected_button(key: impl Into<String>, label: &str, selected: bool, t: Theme) -> El {
    button(key, label, t)
        .label(if selected {
            format!("{label} ✓")
        } else {
            label.to_owned()
        })
        .gap(6.)
        .children(selected.then(|| super::icons::glyph("✓", 14., t.text)))
}

pub fn settings(s: &State, t: Theme) -> Option<El> {
    if !s.gallery_settings_open {
        return None;
    }
    let config = &s.tab()?.database.as_ref()?.views.get(s.tab()?.view)?.config;
    let size = CardSize::from_view(config);
    let preview = config.get("gallery_preview").and_then(Value::as_str);
    let mut panel = col()
        .key("gallery-settings")
        .w(250.)
        .padding(8.)
        .gap(4.)
        .fill(t.menu)
        .radius(8.)
        .border(1., t.line)
        .shadow(4., 16., Color::BLACK.alpha(0.15))
        .role("dialog")
        .label("Gallery layout")
        .modal(true, |s: &mut State| s.gallery_settings_open = false)
        .child(t.label("Card size", t.muted));
    for (label, value, selected) in [
        ("Small", "small", size == CardSize::Small),
        ("Medium", "medium", size == CardSize::Medium),
        ("Large", "large", size == CardSize::Large),
    ] {
        panel = panel.child(
            selected_button(format!("gallery-size-{value}"), label, selected, t)
                .w_full()
                .on_press_tx(move |s: &mut State, tx| {
                    save_setting(s, "gallery_card_size", json!(value));
                    tx.instant();
                }),
        );
    }
    panel = panel.child(t.label("Card preview", t.muted));
    for (label, value, selected) in [
        (
            "Page content",
            "page_content",
            matches!(preview, None | Some("page_content")),
        ),
        ("Page cover", "page_cover", preview == Some("page_cover")),
        ("None", "none", preview == Some("none")),
    ] {
        panel = panel.child(
            selected_button(format!("gallery-preview-{value}"), label, selected, t)
                .w_full()
                .on_press_tx(move |s: &mut State, tx| {
                    save_setting(s, "gallery_preview", json!(value));
                    tx.instant();
                }),
        );
    }
    if preview
        .is_some_and(|value| value != "page_content" && value != "page_cover" && value != "none")
    {
        panel = panel.child(
            t.label("This preview type is not available yet.", t.muted)
                .wrap(),
        );
    }
    panel = panel.child(t.label("Load limit", t.muted));
    let limit = crate::records::view_load_limit(&s.tab()?.database.as_ref()?.views[s.tab()?.view]);
    for value in [10, 25, 50, 100] {
        panel = panel.child(
            selected_button(
                format!("gallery-limit-{value}"),
                &value.to_string(),
                value == limit,
                t,
            )
            .w_full()
            .on_press_tx(move |s: &mut State, tx| {
                if save_setting(s, "load_limit", json!(value)) {
                    s.move_record_page(0);
                }
                tx.instant();
            }),
        );
    }
    Some(panel.anchored("gallery-settings-button", Side::Bottom, Align::End, 4.))
}

fn save_setting(s: &mut State, key: &str, value: Value) -> bool {
    let Some(tab) = s.tab() else { return false };
    let Some(db) = tab.database.as_ref() else {
        return false;
    };
    let Some(view) = db.views.get(tab.view) else {
        return false;
    };
    let db_id = db.id.clone();
    let view_id = view.id.clone();
    let result = s
        .store
        .edit_view_config(&db_id, &view_id, ViewKind::Gallery, |config| {
            if config.is_null() {
                *config = json!({});
            }
            let map = config.as_object_mut().ok_or("invalid view configuration")?;
            map.insert(key.into(), value);
            Ok(())
        });
    match result {
        Ok(db) => {
            super::property_visibility::sync_database(s, &db);
            s.error = None;
            true
        }
        Err(e) => {
            s.error = Some(format!("Gallery settings are not saved: {e}"));
            false
        }
    }
}
