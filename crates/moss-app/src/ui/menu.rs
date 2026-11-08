use super::theme::Theme;
use rune::*;

pub const WIDTH: f32 = 288.;

pub use super::phosphor::{
    ARROW_CLOCKWISE as RETRY, ARROW_LEFT as BACK, CARET_RIGHT as CHEVRON, CHECK, EYE,
    EYE_SLASH as EYE_OFF, X as CLOSE,
};

pub fn icon_slot(key: impl Into<String>, icon: El) -> El {
    row().key(key).size(20., 20.).center().child(icon)
}

pub fn row_item(key: impl Into<String>, name: &str, icon: El, t: Theme) -> El {
    let key = key.into();
    row()
        .key(key.clone())
        .w_full()
        .h(32.)
        .px(10.)
        .gap(8.)
        .center_y()
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label(name)
        .focusable(true)
        .child(icon_slot(format!("{key}-icon"), icon))
        .child(
            text(name)
                .key(format!("{key}-label"))
                .font(TextStyle::new(14., 400.).family(Family::Sans).lh(20.))
                .color(t.text)
                .grow(1.)
                .min_w(0.)
                .truncate(),
        )
}

pub fn end(key: impl Into<String>, mark: Option<&'static Icon>, t: Theme) -> El {
    icon_slot(
        key,
        mark.map_or_else(el, |icon| rune::icon(icon, 16.).color(t.muted)),
    )
}
