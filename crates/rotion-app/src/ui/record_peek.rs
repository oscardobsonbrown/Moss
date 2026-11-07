use super::theme::{Theme, button, icon_button};
use crate::state::{PeekMode, State, display_value};
use rotion_core::{BlockKind, PropertyKind};
use rune::*;

pub fn side(s: &State, t: Theme) -> Option<El> {
    let peek = s.record_peek.as_ref()?;
    if peek.mode != PeekMode::Side {
        return None;
    }
    Some(
        panel(s, t)
            .key("record-peek-side")
            .w(420.)
            .h_full()
            .border(1., t.line),
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
            .on_press_tx(|s: &mut State, tx| {
                s.close_peek();
                tx.instant();
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
    let title = text_input(
        &peek.title,
        crate::fonts::page_style(settings, 32., 700.).lh(40. * scale),
    )
    .key(format!("peek-title-{page_id}"))
    .w_full()
    .min_h(40. * scale)
    .align(Align::Start)
    .color(t.text)
    .placeholder("Untitled", t.faint)
    .label("Record title")
    .on_edit(|s: &mut State| &mut s.record_peek.as_mut().unwrap().title)
    .on_input_change(|s: &mut State, _| s.save_peek_title())
    .on_submit(|s: &mut State, tx| {
        if let Some(peek) = s.record_peek.as_mut() {
            peek.title.focused = false;
            if let Some(block) = peek.blocks.first_mut() {
                block.edit.focused = true;
                block.edit.set_caret(0);
            }
        }
        tx.instant();
    });
    let mut content = col()
        .w_full()
        .gap(4.)
        .px(32.)
        .pt(32.)
        .pb(160.)
        .child(title)
        .child(el().h(20.));
    if let Some(db) = s.tab().and_then(|tab| tab.database.as_ref()) {
        for prop in db
            .properties
            .iter()
            .filter(|p| p.kind != PropertyKind::Title)
        {
            let id = prop.id.clone();
            let record_id = peek.record.id.clone();
            let active = s.editing_cell.as_ref().is_some_and(|cell| {
                cell.record_id == record_id && cell.property_id.as_deref() == Some(id.as_str())
            });
            let displayed = peek
                .record
                .values
                .get(&id)
                .map(display_value)
                .unwrap_or_default();
            let value = if active {
                text_input(&s.cell, crate::fonts::page_style(settings, 14., 400.))
                    .key(format!("peek-cell-{id}"))
                    .single_line()
                    .w_full()
                    .h(30.)
                    .px(7.)
                    .radius(4.)
                    .fill(t.sidebar)
                    .color(t.text)
                    .label(&prop.name)
                    .placeholder("Empty", t.faint)
                    .on_edit(|s: &mut State| &mut s.cell)
                    .on_input_change(|s: &mut State, _| {
                        s.save_cell();
                    })
                    .on_input_key(|s: &mut State, event, tx| {
                        if event.key == Key::Escape {
                            s.finish_cell(false);
                            tx.instant();
                            return true;
                        }
                        false
                    })
                    .on_submit(|s: &mut State, tx| {
                        s.finish_cell(false);
                        tx.instant();
                    })
            } else {
                row()
                    .key(format!("peek-property-{id}"))
                    .w_full()
                    .h(30.)
                    .px(7.)
                    .center_y()
                    .radius(4.)
                    .hover(t.hover)
                    .role("button")
                    .label(format!("{}: {}", prop.name, displayed))
                    .focusable(true)
                    .child(
                        t.label(
                            if displayed.is_empty() {
                                "Empty"
                            } else {
                                &displayed
                            },
                            14.,
                            if displayed.is_empty() {
                                t.faint
                            } else {
                                t.text
                            },
                        )
                        .truncate(),
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        s.edit_record_property(&record_id, Some(id.clone()));
                        tx.instant();
                    })
            };
            content = content.child(
                row()
                    .w_full()
                    .center_y()
                    .gap(8.)
                    .child(t.label(&prop.name, 13., t.muted).w(130.).truncate())
                    .child(value.grow(1.)),
            );
        }
    }
    content = content
        .child(el().h(22.))
        .child(el().w_full().h(1.).fill(t.line))
        .child(el().h(22.));
    for (i, block) in peek.blocks.iter().enumerate() {
        let kind = block.block.kind;
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
        let input = text_input(&block.edit, style)
            .key(format!("peek-block-{}", block.block.id))
            .align(Align::Start)
            .w_full()
            .min_h(line * scale)
            .color(t.text)
            .placeholder(if i == 0 { "Type something…" } else { "" }, t.faint)
            .label("Block text")
            .on_edit(move |s: &mut State| &mut s.record_peek.as_mut().unwrap().blocks[i].edit)
            .on_input_change(|s: &mut State, _| s.save_peek_blocks())
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
        let marker = match kind {
            BlockKind::BulletedList => Some("•"),
            BlockKind::NumberedList => Some("1."),
            BlockKind::Todo => Some(if block.block.checked { "☑" } else { "☐" }),
            BlockKind::Quote => Some("▏"),
            BlockKind::Callout => Some("💡"),
            BlockKind::Toggle => Some("▸"),
            _ => None,
        };
        let item = row()
            .w_full()
            .gap(6.)
            .children(marker.map(|m| t.label(m, 16., t.muted).w(24.)))
            .child(input);
        content = content.child(if matches!(kind, BlockKind::Code | BlockKind::Callout) {
            item.padding(12.).radius(4.).fill(t.sidebar)
        } else {
            item
        });
    }
    let header = row()
        .w_full()
        .h(44.)
        .px(10.)
        .center_y()
        .child(t.label("Record", 13., t.muted))
        .child(el().grow(1.))
        .child(
            button("peek-full-page", "Open as page ↗", t).on_press_tx(|s: &mut State, tx| {
                s.expand_peek();
                tx.instant();
            }),
        )
        .child(
            icon_button("peek-close", "Close", "×", t).on_press_tx(|s: &mut State, tx| {
                s.close_peek();
                tx.instant();
            }),
        );
    col()
        .fill(t.bg)
        .min_h(0.)
        .child(header)
        .children(
            s.error
                .as_ref()
                .map(|err| t.label(err, 13., t.text).wrap().px(32.)),
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
                .child(content),
        )
}
