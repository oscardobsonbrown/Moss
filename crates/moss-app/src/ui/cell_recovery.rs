use super::theme::{Theme, button};
use crate::state::State;
use rune::*;

pub fn view(s: &State, t: Theme) -> Option<El> {
    if s.property_picker().is_some()
        || !s.editing_cell.as_ref().is_some_and(|cell| cell.save_failed)
    {
        return None;
    }
    // This is Moss's recovery control; its source appearance is not yet observed.
    Some(
        row()
            .key("cell-recovery")
            .w_full()
            .px(8.)
            .gap(8.)
            .center_y()
            .fill(t.sidebar)
            .radius(4.)
            .child(t.label("Changes are not saved", t.text).grow(1.).truncate())
            .child(
                button("cell-retry", "Retry", t).on_press_tx(|s: &mut State, tx| {
                    s.finish_cell(false);
                    tx.instant();
                }),
            )
            .child(button("cell-discard", "Discard and reload", t).on_press_tx(
                |s: &mut State, tx| {
                    s.discard_cell_and_reload();
                    tx.instant();
                },
            )),
    )
}
