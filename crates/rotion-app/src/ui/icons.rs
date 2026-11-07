use rune::{Color, El, Icon};
const SEARCH: Icon = Icon {
    name: "search",
    view: 20.,
    body: r#"<circle cx="8.5" cy="8.5" r="5.7" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="m13 13 4.3 4.3" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>"#,
};
const HOME: Icon = Icon {
    name: "home",
    view: 20.,
    body: r#"<path d="M3 9.2 10 3l7 6.2V17h-5v-5H8v5H3Z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>"#,
};
const TRASH: Icon = Icon {
    name: "trash",
    view: 20.,
    body: r#"<path d="M3.5 5.5h13M7 5V3h6v2M5 6l.8 11h8.4L15 6M8 8.5v5.5m4-5.5v5.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
const SETTINGS: Icon = Icon {
    name: "settings",
    view: 20.,
    body: r#"<path d="m8 2-.5 2-1.2.7-2-.5-2 3.5 1.5 1.4v1.8L2.3 12l2 3.5 2-.5 1.2.7.5 2h4l.5-2 1.2-.7 2 .5 2-3.5-1.5-1.2V9.1l1.5-1.4-2-3.5-2 .5-1.2-.7-.5-2Z" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/><circle cx="10" cy="10" r="2.5" fill="none" stroke="currentColor" stroke-width="1.3"/>"#,
};
const PLUS: Icon = Icon {
    name: "plus",
    view: 20.,
    body: r#"<path d="M10 4v12M4 10h12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>"#,
};
const CLOSE: Icon = Icon {
    name: "close",
    view: 20.,
    body: r#"<path d="m5 5 10 10M5 15 15 5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>"#,
};
const GRIP: Icon = Icon {
    name: "grip",
    view: 20.,
    body: r#"<circle cx="7.5" cy="5" r="1.1"/><circle cx="12.5" cy="5" r="1.1"/><circle cx="7.5" cy="10" r="1.1"/><circle cx="12.5" cy="10" r="1.1"/><circle cx="7.5" cy="15" r="1.1"/><circle cx="12.5" cy="15" r="1.1"/>"#,
};
const CHEVRON: Icon = Icon {
    name: "chevron",
    view: 20.,
    body: r#"<path d="m6 8 4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
pub fn glyph(name: &str, size: f32, color: Color) -> El {
    let icon = match name {
        "⌕" => Some(&SEARCH),
        "⌂" => Some(&HOME),
        "♧" | "⌫" => Some(&TRASH),
        "⚙" => Some(&SETTINGS),
        "+" => Some(&PLUS),
        "×" => Some(&CLOSE),
        "⠿" | "⋮" => Some(&GRIP),
        "⌄" => Some(&CHEVRON),
        _ => None,
    };
    match icon {
        Some(i) => rune::icon(i, size).color(color),
        None => super::theme::label(name, size, color),
    }
}
