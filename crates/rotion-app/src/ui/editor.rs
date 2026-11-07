use super::theme::{Theme, button, icon_button, label};
use crate::state::{Modal, State};
use rotion_core::BlockKind;
use rune::*;

pub fn view(s: &State, t: Theme) -> El {
    let tab = s.tab().unwrap();
    let title = text_input(
        &tab.title,
        crate::fonts::page_style(tab.page.settings, 40., 700.)
            .lh(48. * crate::fonts::scale(tab.page.settings)),
    )
    .key(format!("title-{}", tab.page.id))
    .single_line()
    .w_full()
    .color(t.text)
    .placeholder("Untitled", t.faint)
    .label("Page title")
    .on_edit(|s: &mut State| &mut s.tabs[s.active].title)
    .on_input_change(|s: &mut State, _| s.save_title());
    let blocks = tab.blocks.iter().enumerate().map(|(i, b)| {
        let kind = b.block.kind;
        if kind == BlockKind::Divider {
            return row()
                .key(format!("block-{}", b.block.id))
                .w_full()
                .h(24.)
                .center_y()
                .child(el().h(1.).grow(1.).fill(t.line))
                .child(
                    icon_button(format!("block-menu-{i}"), "Block actions", "⋮", t)
                        .on_press(move |s: &mut State| s.modal = Some(Modal::Block(i))),
                );
        }
        let (size, weight, line) = match kind {
            BlockKind::Heading1 => (30., 600., 38.),
            BlockKind::Heading2 => (24., 600., 32.),
            BlockKind::Heading3 => (20., 600., 28.),
            BlockKind::Code => (14., 400., 23.),
            _ => (16., 400., 26.),
        };
        let style = if kind == BlockKind::Code {
            crate::fonts::page_style(tab.page.settings, size, weight)
                .lh(line * crate::fonts::scale(tab.page.settings))
                .mono()
        } else {
            crate::fonts::page_style(tab.page.settings, size, weight)
                .lh(line * crate::fonts::scale(tab.page.settings))
        };
        let input = text_input(&b.edit, style)
            .key(format!("block-input-{}", b.block.id))
            .w_full()
            .min_h(line * crate::fonts::scale(tab.page.settings))
            .color(if b.block.checked { t.faint } else { t.text })
            .placeholder(if i == 0 { "Type / for commands" } else { "" }, t.faint)
            .label("Block text")
            .on_edit(move |s: &mut State| &mut s.tabs[s.active].blocks[i].edit)
            .on_input_change(|s: &mut State, _| s.save_blocks())
            .on_input_key(move |s: &mut State, event, tx| {
                if matches!(event.key, Key::Command('b' | 'i' | 'u' | 'z')) {
                    s.tabs[s.active].blocks[i].edit.key(event, None);
                    s.save_blocks();
                    tx.instant();
                    return true;
                }
                if event.key == Key::Text("/".into())
                    && s.tabs[s.active].blocks[i].edit.text.is_empty()
                {
                    tx.instant();
                    s.modal = Some(Modal::Block(i));
                    return true;
                }
                if event.key == Key::Enter && event.mods.shift {
                    s.tabs[s.active].blocks[i].edit.insert("\n");
                    s.save_blocks();
                    tx.instant();
                    return true;
                }
                if event.key == Key::Backspace
                    && s.tabs[s.active].blocks[i].edit.text.is_empty()
                    && i > 0
                {
                    s.tabs[s.active].blocks.remove(i);
                    let edit = &mut s.tabs[s.active].blocks[i - 1].edit;
                    edit.focused = true;
                    edit.set_caret(edit.text.len());
                    s.save_blocks();
                    tx.instant();
                    return true;
                }
                false
            })
            .on_submit(move |s: &mut State, tx| {
                tx.instant();
                if kind == BlockKind::Code {
                    s.tabs[s.active].blocks[i].edit.insert("\n");
                    s.save_blocks();
                } else {
                    s.add_block(i);
                }
            });
        let marker = match kind {
            BlockKind::Todo => row()
                .key(format!("todo-{i}"))
                .size(24., 26.)
                .center()
                .role("checkbox")
                .label("Complete task")
                .checked(b.block.checked)
                .focusable(true)
                .child(label(
                    if b.block.checked { "☑" } else { "☐" },
                    20.,
                    if b.block.checked { t.blue } else { t.muted },
                ))
                .on_press_tx(move |s: &mut State, tx| {
                    tx.instant();
                    s.tabs[s.active].blocks[i].block.checked =
                        !s.tabs[s.active].blocks[i].block.checked;
                    s.save_blocks();
                }),
            BlockKind::BulletedList => label("•", 18., t.text).w(24.),
            BlockKind::NumberedList => label(format!("{}.", i + 1), 16., t.text).w(30.),
            BlockKind::Quote => el().w(3.).h(26.).fill(t.text),
            BlockKind::Callout => label("💡", 20., t.text).w(28.),
            BlockKind::Toggle => button(
                format!("toggle-{i}"),
                if b.block.config["collapsed"].as_bool().unwrap_or(false) {
                    "▸"
                } else {
                    "▾"
                },
                t,
            )
            .w(26.)
            .on_press_tx(move |s: &mut State, tx| {
                tx.instant();
                let b = &mut s.tabs[s.active].blocks[i].block;
                b.config["collapsed"] =
                    serde_json::json!(!b.config["collapsed"].as_bool().unwrap_or(false));
                s.save_blocks();
            }),
            _ => el().w(0.),
        };
        let row = row()
            .key(format!("block-{}", b.block.id))
            .hover_group()
            .w_full()
            .min_h(30. * crate::fonts::scale(tab.page.settings))
            .gap(6.)
            .child(
                icon_button(format!("block-menu-{i}"), "Block actions", "⠿", t)
                    .size(24., 26.)
                    .absolute(-30., 0.)
                    .group_opacity(0., 1.)
                    .on_press(move |s: &mut State| s.modal = Some(Modal::Block(i))),
            )
            .children(
                matches!(
                    kind,
                    BlockKind::Todo
                        | BlockKind::BulletedList
                        | BlockKind::NumberedList
                        | BlockKind::Quote
                        | BlockKind::Callout
                        | BlockKind::Toggle
                )
                .then_some(marker),
            )
            .child(input);
        if matches!(kind, BlockKind::Code | BlockKind::Callout) {
            row.padding(14.).radius(4.).fill(t.sidebar)
        } else {
            row
        }
    });
    let mut content = col()
        .key(format!("document-{}", tab.page.id))
        .w_full()
        .self_center()
        .px(70.)
        .pt(54.)
        .pb(220.)
        .gap(4.);
    if !tab.page.settings.full_width {
        content = content.max_w(860.);
    }
    col()
        .key(format!("document-scroll-{}", tab.page.id))
        .grow(1.)
        .min_h(0.)
        .w_full()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(
            content
                .child(label(&tab.page.icon, 64., t.text).h(84.))
                .child(title)
                .child(el().h(24.))
                .children(blocks)
                .child(
                    button("append-block", "+", t)
                        .w(32.)
                        .on_press_tx(|s: &mut State, tx| {
                            tx.instant();
                            let i = s.tabs[s.active].blocks.len().saturating_sub(1);
                            s.add_block(i);
                        }),
                ),
        )
}
