use super::{CHROME_EDGE, COLLAPSED_SIDEBAR_WIDTH, theme::Theme};
use crate::state::State;
use rune::*;

pub fn view(s: &State, t: Theme, width: f32) -> El {
    let compact = s.sidebar_collapsed() && !s.sidebar_reveal.visible;
    let surface = col()
        .key("sidebar-surface")
        .pin(Some(0.), Some(CHROME_EDGE), None, Some(0.))
        .w(if compact {
            COLLAPSED_SIDEBAR_WIDTH
        } else {
            width
        })
        .min_h(0.)
        .motion(tok::motion::SNAP_EXIT)
        .fill(t.sidebar)
        .clip_corners([0., 0., 0., 0.])
        .on_hover_change(|s: &mut State, inside, tx| {
            s.sidebar_reveal_hover(inside, tx);
            if inside && s.sidebar_collapsed() {
                tx.animated();
            }
        })
        .on_key(|s: &mut State, event, tx| {
            if event.key == Key::Escape {
                if s.sidebar_collapsed()
                    && s.sidebar_reveal.visible
                    && s.sidebar_page_menu.is_none()
                    && s.sidebar_section_menu.is_none()
                    && s.page_icon_picker.is_none()
                    && s.modal.is_none()
                {
                    s.dismiss_sidebar_reveal_by_key();
                    tx.instant();
                    return true;
                }
            } else if s.sidebar_collapsed() && s.sidebar_reveal.visible {
                s.open_sidebar_reveal_for_keyboard();
            }
            false
        })
        .child(super::shell::sidebar(s, t, width));
    if s.sidebar_reveal.keyboard_open {
        surface.modal(true, |s: &mut State| s.dismiss_sidebar_reveal_by_key())
    } else {
        surface
    }
}
