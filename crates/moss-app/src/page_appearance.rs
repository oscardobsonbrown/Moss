use crate::state::{Modal, State};
use rune::Tx;
use std::time::Duration;

const GRACE: f64 = 0.12;
#[derive(Default)]
pub struct PageAppearance {
    row_hover: bool,
    panel_hover: bool,
    pub keyboard_open: bool,
    leave_since: Option<f64>,
}
#[derive(Clone, Copy)]
pub enum AppearanceSurface {
    Row,
    Panel,
}
impl State {
    pub fn appearance_hover(&mut self, surface: AppearanceSurface, inside: bool, tx: &mut Tx) {
        match surface {
            AppearanceSurface::Row => self.page_appearance.row_hover = inside,
            AppearanceSurface::Panel => self.page_appearance.panel_hover = inside,
        }
        if inside {
            if matches!(self.modal, Some(Modal::PageActions | Modal::Appearance)) {
                self.modal = Some(Modal::Appearance);
                self.page_appearance.leave_since = None;
            }
        } else if self.modal == Some(Modal::Appearance) {
            tx.wake_after(Duration::ZERO);
        }
    }
    pub fn appearance_open_keyboard(&mut self) {
        self.modal = Some(Modal::Appearance);
        self.page_appearance.keyboard_open = true;
        self.page_appearance.leave_since = None;
    }
    pub fn appearance_close(&mut self) {
        self.modal = Some(Modal::PageActions);
        self.page_appearance = PageAppearance::default();
    }
    pub fn appearance_hover_sibling(&mut self, inside: bool, tx: &mut Tx) {
        if inside && self.modal == Some(Modal::Appearance) && !self.page_appearance.keyboard_open {
            self.page_appearance.row_hover = false;
            tx.wake_after(Duration::ZERO);
        }
    }
}
pub fn poll(s: &mut State, time: f64, tx: &mut Tx) -> bool {
    if s.modal != Some(Modal::Appearance) {
        s.page_appearance = PageAppearance::default();
        return false;
    }
    let state = &mut s.page_appearance;
    if state.row_hover || state.panel_hover || state.keyboard_open {
        state.leave_since = None;
        return false;
    }
    let since = *state.leave_since.get_or_insert(time);
    let remaining = GRACE - (time - since);
    if remaining > 0. {
        tx.wake_after(Duration::from_secs_f64(remaining));
        return false;
    }
    s.appearance_close();
    tx.animated();
    true
}
