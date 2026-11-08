use crate::state::{Modal, State};
use rune::Tx;
use std::time::Duration;

const GRACE: f64 = 0.12;

#[derive(Clone, Copy)]
pub enum SidebarSurface {
    Rail,
    Panel,
}

#[derive(Default)]
pub struct SidebarReveal {
    pub visible: bool,
    rail_hover: bool,
    pointer_in_rail: bool,
    panel_hover: bool,
    pub keyboard_open: bool,
    leave_since: Option<f64>,
    suppress_rail_until_leave: bool,
}

impl State {
    pub fn sidebar_reveal_hover(&mut self, surface: SidebarSurface, inside: bool, tx: &mut Tx) {
        if !self.sidebar_collapsed() {
            return;
        }
        match surface {
            SidebarSurface::Rail => {
                self.sidebar_reveal.rail_hover = inside;
            }
            SidebarSurface::Panel => self.sidebar_reveal.panel_hover = inside,
        }
        if inside && self.sidebar_reveal.suppress_rail_until_leave {
            return;
        }
        if inside {
            self.sidebar_reveal.visible = true;
            self.sidebar_reveal.leave_since = None;
        } else if self.sidebar_reveal.visible {
            tx.wake_after(Duration::ZERO);
        }
    }

    pub fn close_sidebar_reveal(&mut self) {
        self.sidebar_reveal = SidebarReveal::default();
    }

    pub fn dismiss_sidebar_reveal_by_key(&mut self) {
        // A modal can clear hover while the pointer still rests over the rail.
        let suppress = self.sidebar_reveal.pointer_in_rail || self.sidebar_reveal.rail_hover;
        self.sidebar_reveal = SidebarReveal {
            pointer_in_rail: self.sidebar_reveal.pointer_in_rail,
            suppress_rail_until_leave: suppress,
            ..Default::default()
        };
    }

    pub fn sidebar_reveal_pointer(&mut self, x: f32, y: f32) {
        let edge = crate::ui::CHROME_EDGE;
        self.sidebar_reveal.pointer_in_rail =
            (0. ..crate::ui::COLLAPSED_SIDEBAR_WIDTH).contains(&x) && y >= edge;
        if !self.sidebar_reveal.pointer_in_rail {
            self.sidebar_reveal.suppress_rail_until_leave = false;
        }
    }

    pub fn open_sidebar_reveal_for_keyboard(&mut self) {
        if self.sidebar_collapsed() {
            self.sidebar_reveal.visible = true;
            self.sidebar_reveal.keyboard_open = true;
            self.sidebar_reveal.leave_since = None;
            self.sidebar_reveal.suppress_rail_until_leave = false;
        }
    }
}

pub fn poll(s: &mut State, time: f64, tx: &mut Tx) -> bool {
    if !s.sidebar_reveal.visible {
        return false;
    }
    if !s.sidebar_collapsed() {
        s.close_sidebar_reveal();
        return true;
    }
    let protected = s.sidebar_page_menu.is_some()
        || s.sidebar_section_menu.is_some()
        || s.page_icon_picker.is_some()
        || matches!(s.modal, Some(Modal::Workspace | Modal::Search));
    let state = &mut s.sidebar_reveal;
    if state.rail_hover || state.panel_hover || state.keyboard_open || protected {
        state.leave_since = None;
        // Menu closing is not a hover event, so recheck until it closes.
        if protected {
            tx.wake_after(Duration::from_secs_f64(GRACE));
        }
        return false;
    }
    let since = *state.leave_since.get_or_insert(time);
    let remaining = GRACE - (time - since);
    if remaining > 0. {
        tx.wake_after(Duration::from_secs_f64(remaining));
        return false;
    }
    s.close_sidebar_reveal();
    tx.animated();
    true
}
