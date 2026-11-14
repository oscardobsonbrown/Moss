use super::theme::Theme;
use crate::state::State;
use rune::*;

macro_rules! icons {
    ($($id:ident => $name:literal),+ $(,)?) => {
        $(pub const $id: Icon = Icon {
            name: concat!("sidebar-action-", $name),
            view: 256.,
            body: include_str!(concat!("../../assets/phosphor/toolbar/", $name, ".svg")),
        };)+
    };
}
icons! {
    GEAR => "gear-six",
    TRASH => "trash",
    SEARCH => "magnifying-glass",
    MORE => "dots-three",
    RIGHT => "caret-right",
}

pub fn button(
    s: &State,
    key: String,
    name: &str,
    icon: &'static Icon,
    size: f32,
    fill: Color,
    t: Theme,
) -> El {
    // Keep the hit target outside the hidden paint so fast clicks still work.
    el().key(key.clone())
        .size(20., 20.)
        .role("button")
        .label(name)
        .focusable(true)
        .child(
            el().key(format!("{key}-visual"))
                .size(20., 20.)
                .center()
                .radius(5.)
                .group_opacity(if s.sidebar_keyboard_controls { 1. } else { 0. }, 1.)
                .hover(fill)
                .hover_group()
                .child(
                    rune::icon(icon, size)
                        .key(format!("{key}-glyph"))
                        .color(t.muted)
                        .group_color(t.text),
                ),
        )
}
