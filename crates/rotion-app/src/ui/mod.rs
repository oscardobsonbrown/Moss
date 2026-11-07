mod database;
mod editor;
mod icons;
mod overlays;
mod page_settings;
mod record_peek;
mod shell;
mod theme;
use crate::state::State;
use rune::*;
pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let dark = s.prefs.dark.unwrap_or(cx.system_dark());
    let t = theme::Theme::new(dark);
    cx.caption_color(if dark { Color::WHITE } else { Color::BLACK });
    cx.titlebar_height(44.);
    col()
        .key("root")
        .w_full()
        .h_full()
        .fill(t.bg)
        .color(t.text)
        .motion(Spring::NONE)
        .hover_motion_within(Anim::INSTANT, Anim::INSTANT)
        .on_key(|_: &mut State, _, tx| {
            tx.instant();
            false
        })
        .child(shell::titlebar(s, cx, t))
        .child(
            row()
                .w_full()
                .grow(1.)
                .min_h(0.)
                .child(shell::sidebar(s, t))
                .child(shell::content(s, t))
                .children(record_peek::side(s, t)),
        )
        .children(record_peek::center(s, t))
        .children(overlays::overlay(s, t))
}
