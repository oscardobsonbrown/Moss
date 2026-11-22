use super::theme::Theme;
use crate::state::{State, display_value};
use chrono::{DateTime, NaiveDate, NaiveDateTime};
use moss_core::{Property, PropertyKind};
use rune::*;
use serde_json::Value;

pub fn text(s: &State, prop: &Property, value: Option<&Value>) -> String {
    let Some(value) = value.filter(|v| !v.is_null()) else {
        return String::new();
    };
    if matches!(prop.kind, PropertyKind::Formula | PropertyKind::Rollup) {
        if let Some(error) = moss_core::calculation_error(value).and_then(Value::as_str) {
            return format!("Error: {error}");
        }
    }
    match prop.kind {
        PropertyKind::Date => date_value(value, &prop.config),
        PropertyKind::Relation => relation_labels(s, prop, value).join(", "),
        PropertyKind::MultiSelect => multi_select_values(value).join(", "),
        _ => display_value(value),
    }
}

pub(super) fn scalar_label(value: impl Into<String>, t: Theme) -> El {
    rune::text(value)
        .font(
            TextStyle::new(14. * t.text_scale, 400.)
                .family(t.family)
                .lh(20. * t.text_scale),
        )
        .color(t.text)
}

pub fn view(s: &State, prop: &Property, value: Option<&Value>, t: Theme) -> El {
    view_with_size(s, prop, value, t, None)
}

pub fn compact_view(s: &State, prop: &Property, value: Option<&Value>, t: Theme) -> El {
    view_with_size(s, prop, value, t, Some(13.))
}

pub fn board_view(s: &State, prop: &Property, value: Option<&Value>, t: Theme) -> El {
    if prop.kind != PropertyKind::Relation {
        return compact_view(s, prop, value, t);
    }
    let mut links = col().min_w(0.).gap(2.);
    if let Some(value) = value {
        for (index, name) in relation_labels(s, prop, value).into_iter().enumerate() {
            links = links.child(relation_link("board", prop, index, name, t));
        }
    }
    links
}

fn relation_link(view: &str, prop: &Property, index: usize, name: String, t: Theme) -> El {
    let underline = Span::new(0, name.len()).underline(t.line);
    row()
        .key(format!("{view}-relation-{}-{index}", prop.id))
        .min_w(0.)
        .h(20.)
        .center_y()
        .gap(4.)
        .child(rune::icon(&super::icons::PAGE_BLANK, 16.).color(t.muted))
        .child(
            rune::rich(name, vec![underline])
                .font(
                    TextStyle::new(13. * t.text_scale, 600.)
                        .family(t.family)
                        .lh(20. * t.text_scale),
                )
                .color(t.text)
                .min_w(0.)
                .truncate(),
        )
}

fn view_with_size(
    s: &State,
    prop: &Property,
    value: Option<&Value>,
    t: Theme,
    size: Option<f32>,
) -> El {
    let label = text(s, prop, value);
    if matches!(prop.kind, PropertyKind::Formula | PropertyKind::Rollup) {
        if let Some(error) = value
            .and_then(moss_core::calculation_error)
            .and_then(Value::as_str)
        {
            return rune::text(&label)
                .key(format!("property-value-{}", prop.id))
                .font(
                    TextStyle::new(size.unwrap_or(14.) * t.text_scale, 400.)
                        .family(t.family)
                        .lh(20. * t.text_scale),
                )
                .color(t.error)
                .help(error)
                .truncate();
        }
    }
    if label.is_empty() {
        return el();
    }
    if prop.kind == PropertyKind::Relation {
        let target_db = prop
            .config
            .get("target_database_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if size.is_some() {
            let mut links = row().min_w(0.).h(20.).center_y().gap(8.).clip_rounded(0.);
            for (index, name) in relation_labels(s, prop, value.unwrap())
                .into_iter()
                .enumerate()
            {
                links = links.child(relation_link("calendar", prop, index, name, t));
            }
            return links;
        }
        let mut links = row()
            .key(format!("property-value-{}", prop.id))
            .grow(1.)
            .min_w(0.)
            .center_y()
            .gap(6.)
            .clip_rounded(0.);
        for (index, (record_id, name)) in relation_items(s, prop, value.unwrap())
            .into_iter()
            .enumerate()
        {
            let underline = Span::new(0, name.len()).underline(t.muted.alpha(0.5));
            let label_el = rune::rich(name.clone(), vec![underline])
                .key(format!("relation-link-text-{}-{index}", prop.id))
                .font(
                    TextStyle::new(14. * t.text_scale, 500.)
                        .family(t.family)
                        .lh(20. * t.text_scale),
                )
                .color(t.text)
                .min_w(0.)
                .truncate();
            let mut chip = row()
                .key(format!("relation-link-{}-{index}", prop.id))
                .min_w(0.)
                .h(22.)
                .px(3.)
                .gap(3.)
                .center_y()
                .radius(4.)
                .child(rune::icon(&super::phosphor::ARROW_UP_RIGHT, 13.).color(t.muted))
                .child(label_el);
            if let (Some(db_id), Some(rec_id)) = (target_db.clone(), record_id) {
                chip = chip
                    .hover(t.hover)
                    .press(t.selected)
                    .role("link")
                    .label(format!("Open {name} in new tab"))
                    .help("Open in new tab")
                    .focusable(true)
                    .on_press_tx(move |s: &mut State, tx| {
                        s.open_related_record_in_new_tab(&db_id, &rec_id);
                        tx.instant();
                    });
            }
            links = links.child(chip);
        }
        return links;
    }
    if prop.kind == PropertyKind::Status {
        let (_, bg, dot) = if let Some(explicit) = explicit_option_color(prop, &label) {
            (
                None,
                option_tag_fill(explicit, t.dark),
                option_tag_dot(explicit, t.dark),
            )
        } else if moss_core::default_status_option_color(&label).is_some() {
            status_style(&label, t)
        } else {
            let color_name = moss_core::resolved_option_color(prop, &label);
            (
                None,
                option_tag_fill(color_name, t.dark),
                option_tag_dot(color_name, t.dark),
            )
        };
        let text_color = if size.is_some()
            && explicit_option_color(prop, &label).is_none()
            && label == "In progress"
            && t.bg.r >= 0.1
        {
            color(0x31577f)
        } else {
            t.text
        };
        row()
            .key(format!("property-value-{}", prop.id))
            .h(if size.is_some() { 18. } else { 20. })
            .max_w(170.)
            .min_w(0.)
            .px(6.)
            .gap(5.)
            .center_y()
            .radius(10.)
            .fill(bg)
            .child(el().size(7., 7.).radius(4.).fill(dot))
            .child(
                match size {
                    Some(size) => t.sized_label(label, size, text_color),
                    None => t.label(label, text_color),
                }
                .min_w(0.)
                .truncate(),
            )
    } else if prop.kind == PropertyKind::Select {
        let bg = option_tag_style(prop, &label, t);
        row()
            .key(format!("property-value-{}", prop.id))
            .h(if size.is_some() { 18. } else { 20. })
            .max_w(170.)
            .min_w(0.)
            .px(6.)
            .center_y()
            .radius(4.)
            .fill(bg)
            .child(
                match size {
                    Some(size) => t.sized_label(label, size, t.text),
                    None => t.label(label, t.text),
                }
                .min_w(0.)
                .truncate(),
            )
    } else if prop.kind == PropertyKind::MultiSelect {
        let items = value.map(multi_select_values).unwrap_or_default();
        if items.is_empty() {
            return el();
        }
        let mut tags = row()
            .key(format!("property-value-{}", prop.id))
            .min_w(0.)
            .center_y()
            .gap(4.)
            .clip_rounded(0.);
        for (idx, item) in items.into_iter().enumerate() {
            let bg = option_tag_style(prop, &item, t);
            tags = tags.child(
                row()
                    .key(format!("property-tag-{}-{idx}", prop.id))
                    .h(if size.is_some() { 18. } else { 20. })
                    .max_w(150.)
                    .min_w(0.)
                    .px(6.)
                    .center_y()
                    .radius(4.)
                    .fill(bg)
                    .child(
                        match size {
                            Some(size) => t.sized_label(item, size, t.text),
                            None => t.label(item, t.text),
                        }
                        .min_w(0.)
                        .truncate(),
                    ),
            );
        }
        tags
    } else {
        let value = scalar_label(label, t)
            .font(
                TextStyle::new(size.unwrap_or(14.) * t.text_scale, 400.)
                    .family(t.family)
                    .lh(20. * t.text_scale),
            )
            .key(format!("property-value-{}", prop.id));
        if size.is_some() {
            value
        } else {
            value.truncate()
        }
    }
}

fn date_value(value: &Value, config: &Value) -> String {
    let format = config
        .get("date_format")
        .and_then(Value::as_str)
        .unwrap_or("full");
    let render = |s: &str| format_date(s, format);
    match value {
        Value::String(s) => render(s),
        Value::Object(map) => {
            let start = map.get("start").and_then(Value::as_str);
            let end = map.get("end").and_then(Value::as_str);
            let mut label = match (start, end) {
                (Some(start), Some(end)) => format!("{} → {}", render(start), render(end)),
                (Some(start), None) => render(start),
                (None, Some(end)) => format!("Until {}", render(end)),
                _ => "Date value".into(),
            };
            if let Some(time) = map.get("time").and_then(Value::as_str) {
                label.push_str(&format!(" · {time}"));
            }
            if let Some(zone) = map.get("time_zone").and_then(Value::as_str) {
                label.push_str(&format!(" · {zone}"));
            }
            if map.iter().any(|(key, value)| {
                !matches!(key.as_str(), "start" | "end" | "time" | "time_zone") && !value.is_null()
            }) {
                label.push_str(" · Additional date details");
            }
            label
        }
        _ => "Date value".into(),
    }
}
fn format_naive_date(date: NaiveDate, format: &str) -> String {
    if format == "relative" {
        let today = chrono::Local::now().date_naive();
        let diff = (date - today).num_days();
        return match diff {
            0 => "Today".into(),
            1 => "Tomorrow".into(),
            -1 => "Yesterday".into(),
            _ => date.format("%-d %b %Y").to_string(),
        };
    }
    let pattern = match format {
        "month_day_year" | "MM/DD/YYYY" => "%m/%d/%Y",
        "month_day_year_full" => "%B %-d, %Y",
        "year_month_day" | "YYYY-MM-DD" | "iso" => "%Y-%m-%d",
        "full" => "%-d %B %Y",
        _ => "%d/%m/%Y",
    };
    date.format(pattern).to_string()
}

fn format_date(value: &str, format: &str) -> String {
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return format_naive_date(date, format);
    }
    if let Ok(date) = DateTime::parse_from_rfc3339(value) {
        return format!(
            "{} {}",
            format_naive_date(date.date_naive(), format),
            value.get(11..).unwrap_or("")
        );
    }
    if let Ok(date) = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M"))
    {
        return format!(
            "{} {}",
            format_naive_date(date.date(), format),
            value.get(11..).unwrap_or("")
        );
    }
    value.to_owned()
}

pub(super) fn date_format_label(format: &str) -> &'static str {
    moss_core::DATE_FORMATS
        .iter()
        .find(|(id, _)| *id == format)
        .map(|(_, label)| *label)
        .unwrap_or("Full date")
}

pub(super) fn date_format_choices<F>(
    key_prefix: &str,
    active_format: &str,
    t: Theme,
    on_pick: F,
) -> El
where
    F: Fn(&mut State, &'static str) + Clone + 'static,
{
    let sample = chrono::NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
    col().w_full().gap(2.).children(
        moss_core::DATE_FORMATS.into_iter().map(move |(id, name)| {
            let selected = active_format == id;
            let preview = if id == "relative" {
                "Today".to_owned()
            } else {
                format_naive_date(sample, id)
            };
            let on_pick = on_pick.clone();
            row()
                .key(format!("{key_prefix}-{id}"))
                .w_full()
                .h(28.)
                .px(8.)
                .gap(8.)
                .center_y()
                .radius(4.)
                .fill(if selected {
                    t.selected
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .role("menuitemradio")
                .label(name)
                .checked(selected)
                .focusable(true)
                .child(t.label(name, t.text).grow(1.).truncate())
                .child(t.label(preview, t.muted).shrink())
                .children(selected.then(|| super::icons::glyph("✓", 14., t.text)))
                .on_press_tx(move |s: &mut State, tx| {
                    on_pick(s, id);
                    tx.instant();
                })
        }),
    )
}
fn relation_items(s: &State, prop: &Property, value: &Value) -> Vec<(Option<String>, String)> {
    let values: Vec<&Value> = match value {
        Value::Array(items) => items.iter().collect(),
        _ => vec![value],
    };
    if values.is_empty() {
        return Vec::new();
    }
    let mut items = Vec::new();
    let mut unknown = 0;
    for value in values {
        let id = value
            .as_str()
            .or_else(|| value.get("id").and_then(Value::as_str));
        let name = id.and_then(|id| {
            s.tab()
                .and_then(|tab| tab.relation_titles.get(id).map(String::as_str))
                .or_else(|| {
                    if prop
                        .config
                        .get("target_database_id")
                        .and_then(Value::as_str)
                        .is_some()
                    {
                        return None;
                    }
                    s.tabs
                        .iter()
                        .flat_map(|t| t.records.iter())
                        .find(|r| r.id == id || r.page_id.as_deref() == Some(id))
                        .map(|r| r.title.as_str())
                })
                .or_else(|| {
                    s.pages
                        .iter()
                        .find(|p| p.id == id)
                        .map(|p| p.title.as_str())
                })
        });
        match name {
            Some(name) if !name.is_empty() => items.push((id.map(str::to_owned), name.to_owned())),
            _ => unknown += 1,
        }
    }
    if unknown > 0 {
        items.push((
            None,
            format!(
                "{unknown} linked page{}",
                if unknown == 1 { "" } else { "s" }
            ),
        ));
    }
    items
}

fn relation_labels(s: &State, prop: &Property, value: &Value) -> Vec<String> {
    relation_items(s, prop, value)
        .into_iter()
        .map(|(_, name)| name)
        .collect()
}

pub(super) fn color(rgb: u32) -> Color {
    Color {
        r: rune::color::srgb_decode(((rgb >> 16) & 255) as f32 / 255.),
        g: rune::color::srgb_decode(((rgb >> 8) & 255) as f32 / 255.),
        b: rune::color::srgb_decode((rgb & 255) as f32 / 255.),
        a: 1.,
    }
}
pub(super) fn multi_select_values(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        Value::String(s) if !s.trim().is_empty() => vec![s.trim().to_owned()],
        _ => Vec::new(),
    }
}

pub(super) fn explicit_option_color<'a>(prop: &'a Property, name: &str) -> Option<&'a str> {
    prop.config
        .get("options")
        .and_then(Value::as_array)?
        .iter()
        .find(|entry| moss_core::option_name(entry) == Some(name))?
        .get("color")
        .and_then(Value::as_str)
        .filter(|c| moss_core::is_valid_option_color(c))
}

pub(super) fn option_tag_fill(color_name: &str, dark: bool) -> Color {
    let rgb = match (color_name, dark) {
        ("default", false) => 0xededeb,
        ("gray", false) => 0xe3e2e0,
        ("brown", false) => 0xeee0da,
        ("orange", false) => 0xfadec9,
        ("yellow", false) => 0xfdecc8,
        ("green", false) => 0xdbeddb,
        ("blue", false) => 0xd3e5ef,
        ("purple", false) => 0xe8deee,
        ("pink", false) => 0xf5e0e9,
        ("red", false) => 0xffe2dd,
        ("default", true) => 0x373737,
        ("gray", true) => 0x5a5a5a,
        ("brown", true) => 0x603b2c,
        ("orange", true) => 0x854c1d,
        ("yellow", true) => 0x89632a,
        ("green", true) => 0x2b593f,
        ("blue", true) => 0x28456c,
        ("purple", true) => 0x492f64,
        ("pink", true) => 0x69314c,
        ("red", true) => 0x6e3630,
        (_, false) => 0xe3e2e0,
        (_, true) => 0x5a5a5a,
    };
    color(rgb)
}

pub(super) fn option_tag_dot(color_name: &str, dark: bool) -> Color {
    let rgb = match (color_name, dark) {
        ("default", false) | ("gray", false) => 0x9b9b98,
        ("brown", _) => 0x9f6b53,
        ("orange", _) => 0xd9730d,
        ("yellow", _) => 0xcb912f,
        ("green", _) => 0x448361,
        ("blue", _) => 0x2e7cd1,
        ("purple", _) => 0x9065b0,
        ("pink", _) => 0xc14c8a,
        ("red", _) => 0xd44c47,
        _ => 0x9b9b98,
    };
    color(rgb)
}

pub(super) fn option_tag_style(prop: &Property, name: &str, t: Theme) -> Color {
    option_tag_fill(moss_core::resolved_option_color(prop, name), t.dark)
}

pub(super) fn option_color_label(color_name: &str) -> &'static str {
    match color_name {
        "default" => "Default",
        "gray" => "Gray",
        "brown" => "Brown",
        "orange" => "Orange",
        "yellow" => "Yellow",
        "green" => "Green",
        "blue" => "Blue",
        "purple" => "Purple",
        "pink" => "Pink",
        "red" => "Red",
        _ => "Gray",
    }
}

pub(super) fn option_color_choices<F>(
    key_prefix: &str,
    active_color: &str,
    t: Theme,
    on_pick: F,
) -> El
where
    F: Fn(&mut State, &'static str) + Clone + 'static,
{
    let mut grid = col().w_full().gap(2.);
    for pair in moss_core::OPTION_COLORS.chunks(2) {
        let mut line = row().w_full().gap(2.);
        for &color_name in pair {
            let selected = active_color == color_name;
            let label_text = option_color_label(color_name);
            let on_pick = on_pick.clone();
            line = line.child(
                row()
                    .key(format!("{key_prefix}-{color_name}"))
                    .grow(1.)
                    .min_w(0.)
                    .h(28.)
                    .px(8.)
                    .gap(6.)
                    .center_y()
                    .radius(4.)
                    .fill(if selected {
                        t.selected
                    } else {
                        Color::TRANSPARENT
                    })
                    .hover(t.hover)
                    .role("menuitemradio")
                    .label(label_text)
                    .checked(selected)
                    .focusable(true)
                    .child(
                        el().size(14., 14.)
                            .radius(4.)
                            .fill(option_tag_fill(color_name, t.dark))
                            .border(1., t.line),
                    )
                    .child(t.label(label_text, t.text).grow(1.).min_w(0.).truncate())
                    .children(selected.then(|| super::icons::glyph("✓", 13., t.text)))
                    .on_press_tx(move |s: &mut State, tx| {
                        on_pick(s, color_name);
                        tx.instant();
                    }),
            );
        }
        grid = grid.child(line);
    }
    grid
}

pub(super) fn status_style(name: &str, t: Theme) -> (Option<&'static str>, Color, Color) {
    let dark = t.bg.r < 0.1;
    match name {
        "Not started" => (
            Some("To-do"),
            color(if dark { 0x5f5e58 } else { 0xe9e9e7 }),
            color(0x9b9b98),
        ),
        "In progress" => (
            Some("In progress"),
            color(if dark { 0x385f8c } else { 0xd3e5ef }),
            color(0x2784e0),
        ),
        "Done" => (
            Some("Complete"),
            color(if dark { 0x38664b } else { 0xdbeddb }),
            color(0x4eaa78),
        ),
        _ => (None, t.selected, t.muted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::{PropertyKind, new_id};
    use serde_json::json;

    fn prop(kind: PropertyKind, config: Value) -> Property {
        Property {
            id: new_id(),
            name: "Property".into(),
            kind,
            config,
        }
    }

    #[test]
    fn computed_errors_are_visible_and_not_rendered_as_json() {
        let path = std::env::temp_dir().join(format!("moss-value-error-{}", new_id()));
        let state = State::open(path.clone(), None).unwrap();
        fn view_error(s: &State, _: &mut Cx<State>) -> El {
            let t = Theme::new(false);
            view(
                s,
                &Property {
                    id: "computed".into(),
                    name: "Formula".into(),
                    kind: PropertyKind::Formula,
                    config: json!({}),
                },
                Some(&json!({"_moss_error":"Division by zero"})),
                t,
            )
        }
        let mut rt = Runtime::new(
            state,
            view_error,
            (300., 100.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.frame(0.);
        let e = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("property-value-computed"))
            .unwrap();
        assert_eq!(e.text.as_deref(), Some("Error: Division by zero"));
        assert_eq!(e.help.as_deref(), Some("Division by zero"));
        assert_eq!(e.text_color, Some(Theme::new(false).error));
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn formats_dates_ranges_and_safe_relation_labels() {
        let path = std::env::temp_dir().join(format!("moss-value-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        s.create_inline(Default::default(), false);
        s.cell.replace_text("Planning");
        s.finish_cell(false);
        let record = s.tab().unwrap().records[0].id.clone();
        let relation = prop(PropertyKind::Relation, json!({}));
        assert_eq!(
            text(&s, &relation, Some(&json!([record, "unknown-id"]))),
            "Planning, 1 linked page"
        );
        assert_eq!(
            text(&s, &relation, Some(&json!(["unknown-a", "unknown-b"]))),
            "2 linked pages"
        );
        assert_eq!(text(&s, &relation, None), "");
        let date = prop(PropertyKind::Date, json!({"date_format":"month_day_year"}));
        assert_eq!(text(&s, &date, Some(&json!("2026-11-09"))), "11/09/2026");
        assert_eq!(
            text(
                &s,
                &date,
                Some(&json!({"start":"2026-11-09", "end":"2026-11-12", "time":"09:30"}))
            ),
            "11/09/2026 → 11/12/2026 · 09:30"
        );
        assert_eq!(
            text(&s, &date, Some(&json!("2026-11-09T12:30:00+08:00"))),
            "11/09/2026 12:30:00+08:00"
        );
        assert_eq!(
            text(&s, &date, Some(&json!("2026-11-09T12:30"))),
            "11/09/2026 12:30"
        );
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn peek_renders_status_pill_and_formatted_date() {
        let path = std::env::temp_dir().join(format!("moss-value-ui-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        s.create_inline(Default::default(), false);
        s.finish_cell(false);
        let db = s.tab().unwrap().database.as_ref().unwrap();
        let date = db
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Date)
            .unwrap()
            .id
            .clone();
        let status = db
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Status)
            .unwrap()
            .id
            .clone();
        let mut record = s.tab().unwrap().records[0].clone();
        record.values.insert(date.clone(), json!("2026-11-09"));
        s.store.save_record(&record).unwrap();
        s.sync_record(&record);
        s.open_record(0);
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (1440., 900.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.frame(0.);
        let elements = rt.elements();
        assert!(elements.iter().any(|e| e.key.as_deref()
            == Some(&format!("property-value-{status}"))
            && e.radius > 0.));
        assert!(elements.iter().any(|e| e.key.as_deref()
            == Some(&format!("property-value-{date}"))
            && e.text.as_deref() == Some("9 November 2026")));
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn peek_checkbox_saves_a_boolean_and_rejects_invalid_values() {
        let path = std::env::temp_dir().join(format!("moss-peek-checkbox-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        let checkbox = prop(PropertyKind::Checkbox, json!({}));
        let mut db = s.tab().unwrap().database.clone().unwrap();
        db.properties.push(checkbox.clone());
        s.store.save_database(&db).unwrap();
        s.tabs[s.active].database = Some(db);
        s.create_inline(Default::default(), false);
        s.finish_cell(false);
        let id = s.tab().unwrap().records[0].id.clone();
        s.open_record(0);
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (1440., 900.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.frame(0.);
        let (x, y) = rt.find(&format!("peek-property-{}", checkbox.id)).unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 120.);
        assert_eq!(
            rt.state.store.get_record(&id).unwrap().unwrap().values[&checkbox.id],
            true
        );
        let mut record = rt.state.store.get_record(&id).unwrap().unwrap();
        record.values.insert(checkbox.id.clone(), json!("invalid"));
        rt.state.store.save_record(&record).unwrap();
        rt.update(|s, _| s.sync_record(&record));
        rt.frame(1. / 120.);
        assert!(
            rt.elements()
                .iter()
                .any(|e| e.text.as_deref() == Some("Invalid checkbox value"))
        );
        assert!(rt.find(&format!("peek-property-{}", checkbox.id)).is_none());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
