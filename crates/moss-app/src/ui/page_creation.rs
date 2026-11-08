use super::theme::{Theme, button};
use crate::state::State;
use rune::*;

pub fn view(s: &State, t: Theme, width: f32) -> El {
    let tab = s.tab().unwrap();
    col()
        .key("page-type-scroll")
        .grow(1.)
        .min_h(0.)
        .min_w(0.)
        .w_full()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(
            col()
                .key("page-type-chooser")
                .w_full()
                .min_w(0.)
                .max_w(if tab.page.settings.full_width {
                    width
                } else {
                    860.
                })
                .self_center()
                .px((width * 0.1).clamp(56., 70.))
                .pt(86.)
                .pb(80.)
                .gap(24.)
                .child(
                    super::inputs::text(
                        &tab.title,
                        crate::fonts::page_style(tab.page.settings, 40., 700.)
                            .lh(48. * crate::fonts::scale(tab.page.settings)),
                        t,
                    )
                    .key("new-page-title")
                    .w_full()
                    .min_w(0.)
                    .placeholder("Untitled", t.title_hint)
                    .label("Page title")
                    .on_edit(|s: &mut State| &mut s.tabs[s.active].title)
                    .on_input_change(|s: &mut State, _| s.save_title())
                    .on_submit(|s: &mut State, tx| {
                        s.choose_page_type(false);
                        tx.instant();
                    }),
                )
                .child(
                    row()
                        .w_full()
                        .min_w(0.)
                        .flex_wrap()
                        .gap(10.)
                        .child(
                            button("create-document", "Page", t)
                                .w(132.)
                                .h(38.)
                                .border(1., t.line)
                                .radius(6.)
                                .on_press_tx(|s: &mut State, tx| {
                                    s.choose_page_type(false);
                                    tx.instant();
                                }),
                        )
                        .child(
                            button("create-dashboard", "Dashboard", t)
                                .w(132.)
                                .h(38.)
                                .border(1., t.line)
                                .radius(6.)
                                .on_press_tx(|s: &mut State, tx| {
                                    s.choose_dashboard_page();
                                    tx.instant();
                                }),
                        )
                        .child(
                            button("create-database", "Database", t)
                                .w(132.)
                                .h(38.)
                                .border(1., t.line)
                                .radius(6.)
                                .on_press_tx(|s: &mut State, tx| {
                                    s.choose_page_type(true);
                                    tx.instant();
                                }),
                        ),
                ),
        )
}
