use super::theme::{Theme, icon_button};
use crate::state::{PeekMode, State};
use moss_core::BlockKind;
use rune::*;

pub fn side(s: &State, t: Theme, width: f32) -> Option<El> {
    let peek = s.record_peek.as_ref()?;
    if peek.mode != PeekMode::Side {
        return None;
    }
    Some(
        panel(s, t)
            .key("record-peek-side")
            .w(width)
            .min_w(width)
            .h_full()
            .child(
                el().key("record-peek-divider")
                    .w(1.)
                    .pin(Some(0.), Some(0.), None, Some(0.))
                    .fill(t.line),
            ),
    )
}

pub fn center(s: &State, t: Theme) -> Option<El> {
    let peek = s.record_peek.as_ref()?;
    if peek.mode != PeekMode::Center {
        return None;
    }
    Some(
        el().key("record-peek-scrim")
            .pin(Some(0.), Some(0.), Some(0.), Some(0.))
            .fill(Color::BLACK.alpha(0.28))
            .center()
            .on_press(|s: &mut State| {
                s.dismiss_property_picker();
                s.close_peek();
            })
            .child(
                panel(s, t)
                    .key("record-peek-center")
                    .w_full()
                    .max_w(800.)
                    .h_full()
                    .max_h(760.)
                    .radius(8.)
                    .border(1., t.line)
                    .shadow(12., 40., Color::BLACK.alpha(0.18))
                    .role("dialog")
                    .label("Record page")
                    .modal(true, |s: &mut State| {
                        s.dismiss_property_picker();
                        s.close_peek();
                    }),
            ),
    )
}

fn panel(s: &State, t: Theme) -> El {
    let peek = s.record_peek.as_ref().unwrap();
    let page_id = &peek.page.id;
    let settings = peek.page.settings;
    let scale = crate::fonts::scale(settings);
    let panel_key = if peek.mode == PeekMode::Side {
        "record-peek-side"
    } else {
        "record-peek-center"
    };
    let property_width = rune::memo::layout_with(panel_key, |rect| rect[2])
        .map(|width| (width - 144.).clamp(80., 160.))
        .unwrap_or(160.);
    let title = super::inputs::text(
        &peek.title,
        crate::fonts::page_style(settings, 32., 700.).lh(40. * scale),
        t,
    )
    .key(format!("peek-title-{page_id}"))
    .w_full()
    .min_h(40. * scale)
    .align(Align::Start)
    .placeholder("Untitled", t.title_hint)
    .label("Record title")
    .on_edit(|s: &mut State| &mut s.record_peek.as_mut().unwrap().title)
    .on_input_change(|s: &mut State, _| s.save_peek_title())
    .on_submit(|s: &mut State, tx| {
        if let Some(peek) = s.record_peek.as_mut() {
            peek.title.focused = false;
        }
        s.focus_first_page_block(true);
        tx.instant();
    });
    let mut content = col()
        .w_full()
        .gap(4.)
        .px(32.)
        .pt(if peek.page.cover.is_some() { 32. } else { 48. })
        .pb(160.)
        .child(
            row()
                .gap(2.)
                .ml(if peek.page.icon.is_empty() { -6. } else { 0. })
                .center_y()
                .child({
                    let id = page_id.clone();
                    let anchor = format!("peek-icon-{id}");
                    let key = anchor.clone();
                    let control = if peek.page.icon.is_empty() {
                        super::page_cover::header_action(key, "Add icon", t)
                    } else {
                        el().key(key)
                            .radius(5.)
                            .hover(t.hover)
                            .size(40., 56.)
                            .center_y()
                            .child(t.sized_label(&peek.page.icon, 40., t.text))
                    };
                    control
                        .role("button")
                        .label(if peek.page.icon.is_empty() {
                            "Add page icon"
                        } else {
                            "Change page icon"
                        })
                        .focusable(true)
                        .on_press(move |s: &mut State| s.open_page_icon_picker(&id, &anchor))
                })
                .child(super::page_cover::controls(s, &peek.page, t, "peek")),
        )
        .child(title)
        .child(el().h(4.));
    content = content.child(super::record_metadata::view(
        s,
        &peek.database,
        &peek.record,
        settings,
        property_width,
        "peek",
        t,
    ));
    content = content
        .child(el().h(22.))
        .child(el().w_full().h(1.).fill(t.line))
        .child(el().h(22.));
    for (i, block) in peek.blocks.iter().enumerate() {
        let kind = block.block.kind;
        if kind == BlockKind::SimpleTable {
            content = content.child(super::simple_table::view(s, &block.block, true, t));
            continue;
        }
        if kind == BlockKind::Divider {
            content = content.child(
                row()
                    .w_full()
                    .h(24.)
                    .center_y()
                    .child(el().w_full().h(1.).fill(t.line)),
            );
            continue;
        }
        let (size, weight, line) = match kind {
            BlockKind::Heading1 => (30., 600., 38.),
            BlockKind::Heading2 => (24., 600., 32.),
            BlockKind::Heading3 => (20., 600., 28.),
            BlockKind::Code => (14., 400., 23.),
            _ => (16., 400., 26.),
        };
        let mut style = crate::fonts::page_style(settings, size, weight).lh(line * scale);
        if kind == BlockKind::Code {
            style = style.mono();
        }
        let paste_id = block.block.id.clone();
        let input = super::inputs::text(&block.edit, style, t)
            .key(format!("peek-block-{}", block.block.id))
            .align(Align::Start)
            .w_full()
            .min_w(0.)
            .min_h(line * scale)
            .placeholder(if i == 0 { "Type something…" } else { "" }, t.input_hint)
            .label("Block text")
            .on_edit(move |s: &mut State| &mut s.record_peek.as_mut().unwrap().blocks[i].edit)
            .on_input_change(|s: &mut State, _| s.save_peek_blocks())
            .on_input_paste(move |s: &mut State, content, tx| {
                let handled = s.paste_simple_table_into_paragraph(&paste_id, true, content.text());
                if handled {
                    tx.instant();
                }
                handled
            })
            .on_input_key(move |s: &mut State, event, tx| {
                if matches!(event.key, Key::Command('b' | 'i' | 'u' | 'z')) {
                    s.record_peek.as_mut().unwrap().blocks[i]
                        .edit
                        .key(event, None);
                    s.save_peek_blocks();
                    tx.instant();
                    return true;
                }
                if event.key == Key::Enter && event.mods.shift {
                    s.record_peek.as_mut().unwrap().blocks[i].edit.insert("\n");
                    s.save_peek_blocks();
                    tx.instant();
                    return true;
                }
                false
            })
            .on_submit(move |s: &mut State, tx| {
                if kind == BlockKind::Code {
                    s.record_peek.as_mut().unwrap().blocks[i].edit.insert("\n");
                    s.save_peek_blocks();
                } else {
                    s.add_peek_block(i);
                }
                tx.instant();
            });
        let marker =
            match kind {
                BlockKind::BulletedList => Some(t.sized_label("•", 16., t.muted).w(24.)),
                BlockKind::NumberedList => Some(t.sized_label("1.", 16., t.muted).w(24.)),
                BlockKind::Todo => Some(
                    row().size(24., 26.).center().child(
                        row()
                            .size(16., 16.)
                            .center()
                            .radius(3.)
                            .border(1., if block.block.checked { t.blue } else { t.muted })
                            .fill(if block.block.checked {
                                t.blue
                            } else {
                                Color::TRANSPARENT
                            })
                            .children(block.block.checked.then(|| {
                                rune::icon(&super::phosphor::CHECK, 12.).color(Color::WHITE)
                            })),
                    ),
                ),
                BlockKind::Quote => Some(t.sized_label("▏", 16., t.muted).w(24.)),
                BlockKind::Callout => Some(t.sized_label("💡", 16., t.muted).w(24.)),
                BlockKind::Toggle => Some(
                    row()
                        .size(24., 26.)
                        .center()
                        .child(rune::icon(&super::phosphor::CARET_RIGHT, 16.).color(t.muted)),
                ),
                _ => None,
            };
        let item = row()
            .w_full()
            .gap(6.)
            .children(marker)
            .child(col().grow(1.).min_w(0.).child(input));
        content = content.child(if matches!(kind, BlockKind::Code | BlockKind::Callout) {
            item.padding(12.).radius(4.).fill(t.sidebar)
        } else {
            item
        });
    }
    let header = row()
        .key("peek-actions")
        .h(32.)
        .padding(2.)
        .center_y()
        .radius(7.)
        .fill(t.bg)
        .border(1., t.line)
        .shadow(1., 3., Color::BLACK.alpha(0.08))
        .child(
            row()
                .key("peek-full-page")
                .h(28.)
                .px(9.)
                .gap(4.)
                .center_y()
                .radius(4.)
                .hover(t.hover)
                .role("button")
                .label("Open as page")
                .focusable(true)
                .child(
                    t.label("Open as page", t.text)
                        .key("peek-full-page-label")
                        .offset(0., -1.)
                        .offset_motion(Anim::INSTANT),
                )
                .on_press_tx(|s: &mut State, tx| {
                    s.dismiss_property_picker();
                    s.expand_peek();
                    tx.instant();
                }),
        )
        .child(el().w(1.).h(16.).fill(t.line))
        .child(
            icon_button("peek-close", "Close", "×", t).on_press(|s: &mut State| {
                s.dismiss_property_picker();
                s.close_peek();
            }),
        )
        .pin(None, Some(8.), Some(8.), None)
        .raise();
    col()
        .fill(t.bg)
        .min_h(0.)
        .children(super::cell_recovery::view(s, t))
        .children(
            s.error
                .as_ref()
                .map(|err| t.label(err, t.text).wrap().px(32.)),
        )
        .child(
            col()
                .key("peek-scroll")
                .w_full()
                .grow(1.)
                .min_h(0.)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(
                    col()
                        .w_full()
                        .child(super::page_cover::view(s, &peek.page, t, "peek"))
                        .child(content),
                ),
        )
        .child(header)
}
