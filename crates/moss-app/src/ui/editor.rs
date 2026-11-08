use super::theme::{Theme, icon_button};
use crate::{block_commands::BlockCommandMode, state::State};
use moss_core::BlockKind;
use rune::*;

pub fn view(s: &State, t: Theme, available_width: f32) -> El {
    let tab = s.tab().unwrap();
    let title = super::inputs::text(
        &tab.title,
        crate::fonts::page_style(tab.page.settings, 40., 700.)
            .lh(48. * crate::fonts::scale(tab.page.settings)),
        t,
    )
    .key(format!("title-{}", tab.page.id))
    .w_full()
    .placeholder("Untitled", t.title_hint)
    .label("Page title")
    .on_edit(|s: &mut State| &mut s.tabs[s.active].title)
    .on_input_change(|s: &mut State, _| s.save_title())
    .on_submit(|s: &mut State, tx| {
        s.tabs[s.active].title.focused = false;
        s.focus_first_page_block(false);
        tx.instant();
    });
    let blocks = tab.blocks.iter().enumerate().map(|(i, b)| {
        let kind = b.block.kind;
        if kind == BlockKind::ChildPage {
            return page_link(s, tab, b, i, t);
        }
        if kind == BlockKind::SimpleTable {
            return row()
                .key(format!("block-{}", b.block.id))
                .w_full()
                .min_w(0.)
                .gap(6.)
                .child(gutter(s, &b.block.id, i, false, 26., t))
                .child(
                    col()
                        .grow(1.)
                        .min_w(0.)
                        .child(super::simple_table::view(s, &b.block, false, t)),
                );
        }
        if kind == BlockKind::Divider {
            return row()
                .key(format!("block-{}", b.block.id))
                .w_full()
                .h(24.)
                .center_y()
                .child(el().h(1.).grow(1.).fill(t.line))
                .child(gutter(s, &b.block.id, i, b.edit.focused, 26., t));
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
        let key_id = b.block.id.clone();
        let submit_id = b.block.id.clone();
        let paste_id = b.block.id.clone();
        let input = super::inputs::text(&b.edit, style, t)
            .key(format!("block-input-{}", b.block.id))
            .w_full()
            .min_w(0.)
            .min_h(line * crate::fonts::scale(tab.page.settings))
            .color(if kind == BlockKind::Todo && b.block.checked {
                t.faint
            } else {
                t.text
            })
            .label("Block text")
            .on_edit(move |s: &mut State| &mut s.tabs[s.active].blocks[i].edit)
            .on_input_change(|s: &mut State, _| s.save_blocks())
            .on_input_paste(move |s: &mut State, content, tx| {
                let handled = s.paste_simple_table_into_paragraph(&paste_id, false, content.text());
                if handled {
                    tx.instant();
                }
                handled
            })
            .on_input_key(move |s: &mut State, event, tx| {
                if matches!(event.key, Key::Command('b' | 'i' | 'u' | 'z')) {
                    s.tabs[s.active].blocks[i].edit.key(event, None);
                    s.save_blocks();
                    tx.instant();
                    return true;
                }
                if event.key == Key::Command('/') {
                    s.open_block_commands(
                        &key_id,
                        &format!("block-input-{key_id}"),
                        BlockCommandMode::Change,
                    );
                    tx.instant();
                    return true;
                }
                if event.key == Key::Text(" ".into()) && s.editor_markdown_space(&key_id) {
                    tx.instant();
                    return true;
                }
                if event.key == Key::Text("/".into())
                    && kind != BlockKind::Code
                    && s.tabs[s.active].blocks[i].edit.text.is_empty()
                {
                    tx.instant();
                    s.open_block_commands(
                        &key_id,
                        &format!("block-input-{key_id}"),
                        BlockCommandMode::Slash,
                    );
                    return true;
                }
                if event.key == Key::Enter && event.mods.shift {
                    s.tabs[s.active].blocks[i].edit.insert("\n");
                    s.save_blocks();
                    tx.instant();
                    return true;
                }
                if event.key == Key::Backspace && s.editor_backspace_empty(&key_id) {
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
                    s.editor_enter(&submit_id);
                }
            });
        let marker =
            match kind {
                BlockKind::Todo => row()
                    .key(format!("todo-{i}"))
                    .size(24., 26.)
                    .center()
                    .role("checkbox")
                    .label("Complete task")
                    .checked(b.block.checked)
                    .focusable(true)
                    .child(
                        row()
                            .size(16., 16.)
                            .center()
                            .radius(3.)
                            .border(1., if b.block.checked { t.blue } else { t.muted })
                            .fill(if b.block.checked {
                                t.blue
                            } else {
                                Color::TRANSPARENT
                            })
                            .children(b.block.checked.then(|| {
                                rune::icon(&super::phosphor::CHECK, 12.).color(Color::WHITE)
                            })),
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        tx.instant();
                        s.tabs[s.active].blocks[i].block.checked =
                            !s.tabs[s.active].blocks[i].block.checked;
                        s.save_blocks();
                    }),
                BlockKind::BulletedList => super::theme::sized_label("•", 18., t.text).w(24.),
                BlockKind::NumberedList => {
                    super::theme::sized_label(format!("{}.", i + 1), 16., t.text).w(30.)
                }
                BlockKind::Quote => el().w(3.).h(26.).fill(t.text),
                BlockKind::Callout => super::theme::sized_label("💡", 20., t.text).w(28.),
                BlockKind::Toggle => row()
                    .key(format!("toggle-{i}"))
                    .size(26., 30.)
                    .center()
                    .radius(4.)
                    .hover(t.hover)
                    .role("button")
                    .label("Toggle block")
                    .focusable(true)
                    .child(
                        rune::icon(
                            if b.block.config["collapsed"].as_bool().unwrap_or(false) {
                                &super::phosphor::CARET_RIGHT
                            } else {
                                &super::phosphor::CARET_DOWN
                            },
                            16.,
                        )
                        .color(t.muted),
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        tx.instant();
                        let b = &mut s.tabs[s.active].blocks[i].block;
                        b.config["collapsed"] =
                            serde_json::json!(!b.config["collapsed"].as_bool().unwrap_or(false));
                        s.save_blocks();
                    }),
                _ => el().w(0.),
            };
        let hover_key = format!("block-hover-{}", b.block.id);
        let row =
            row()
                .key(format!("block-{}", b.block.id))
                .hover_group_shared(hover_key)
                .w_full()
                .min_h(30. * crate::fonts::scale(tab.page.settings))
                .gap(6.)
                .child(gutter(
                    s,
                    &b.block.id,
                    i,
                    b.edit.focused,
                    line * crate::fonts::scale(tab.page.settings),
                    t,
                ))
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
                .child(col().grow(1.).min_w(0.).child(input).children(
                    b.edit.text.is_empty().then(|| {
                        text("Press / to insert blocks")
                            .key(format!("block-hint-{}", b.block.id))
                            .font(style)
                            .color(t.input_hint)
                            .absolute(0., 0.)
                            .w_full()
                            .group_opacity(if b.edit.focused { 1. } else { 0. }, 1.)
                            .a11y_hidden()
                    }),
                ));
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
        .px((available_width * 0.1).clamp(56., 70.))
        .min_w(0.)
        .pt(if tab.page.cover.is_some() { 24. } else { 54. })
        .pb(220.)
        .gap(4.);
    if !tab.page.settings.full_width {
        content = content.max_w(860.);
    }
    col()
        .key(format!("document-scroll-{}", tab.page.id))
        .grow(1.)
        .min_h(0.)
        .min_w(0.)
        .w_full()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(super::page_cover::view(s, &tab.page, t, "document"))
        .child(
            content
                .child(
                    row()
                        .gap(2.)
                        .ml(if tab.page.icon.is_empty() { -6. } else { 0. })
                        .center_y()
                        .child({
                            let id = tab.page.id.clone();
                            let key = format!("page-header-icon-{id}");
                            let anchor = key.clone();
                            let control = if tab.page.icon.is_empty() {
                                super::page_cover::header_action(key, "Add icon", t)
                            } else {
                                el().key(key)
                                    .radius(6.)
                                    .hover(t.hover)
                                    .size(64., 84.)
                                    .center_y()
                                    .child(super::theme::sized_label(&tab.page.icon, 64., t.text))
                            };
                            control
                                .role("button")
                                .label(if tab.page.icon.is_empty() {
                                    "Add page icon"
                                } else {
                                    "Change page icon"
                                })
                                .focusable(true)
                                .on_press(move |s: &mut State| {
                                    s.open_page_icon_picker(&id, &anchor)
                                })
                        })
                        .child(super::page_cover::controls(s, &tab.page, t, "document")),
                )
                .child(title)
                .children(tab.record_context.as_ref().map(|(db, record)| {
                    super::record_metadata::view(s, db, record, tab.page.settings, 160., "page", t)
                }))
                .child(el().h(24.))
                .children(blocks)
                .children(super::child_pages::view(tab, t))
                .child(
                    row()
                        .key("append-block")
                        .w_full()
                        .h(30.)
                        .center_y()
                        .hover_group()
                        .role("button")
                        .label("Start a new block")
                        .focusable(true)
                        .on_press_tx(|s: &mut State, tx| {
                            tx.instant();
                            s.append_editor_block();
                        }),
                ),
        )
}

fn gutter(s: &State, id: &str, index: usize, focused: bool, line: f32, t: Theme) -> El {
    let visible = focused
        || s.block_commands
            .as_ref()
            .is_some_and(|menu| menu.block_id == id);
    let add_id = id.to_owned();
    let add_key = format!("newblock-add-{id}");
    let add_anchor = add_key.clone();
    let menu_id = id.to_owned();
    let menu_key = format!("block-menu-{index}");
    let menu_anchor = menu_key.clone();
    row()
        .key(format!("block-gutter-{id}"))
        .absolute(-50., 0.)
        .size(50., line.max(30.))
        .px(4.)
        .gap(2.)
        .hover_group_shared(format!("block-hover-{id}"))
        .child(
            icon_button(add_key, "Insert block below", "+", t)
                .size(20., 24.)
                .mt(((line - 24.) / 2.).max(1.))
                .group_opacity(if visible { 1. } else { 0. }, 1.)
                .preserve_focus_on_press(true)
                .on_press(move |s: &mut State| {
                    s.open_block_commands(&add_id, &add_anchor, BlockCommandMode::InsertAfter)
                }),
        )
        .child(
            icon_button(menu_key, "Block actions", "⠿", t)
                .size(20., 24.)
                .mt(((line - 24.) / 2.).max(1.))
                .group_opacity(if visible { 1. } else { 0. }, 1.)
                .preserve_focus_on_press(true)
                .on_press(move |s: &mut State| {
                    s.open_block_commands(&menu_id, &menu_anchor, BlockCommandMode::Change)
                }),
        )
}

fn page_link(
    s: &State,
    tab: &crate::state::Tab,
    b: &crate::state::EditableBlock,
    index: usize,
    t: Theme,
) -> El {
    let target = b.block.config["page_id"].as_str().unwrap_or("");
    let page = tab.page_links.get(target);
    let id = target.to_owned();
    let title = page
        .map(|p| p.title.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(if page.is_some() {
            "Untitled"
        } else {
            "Page unavailable"
        });
    row()
        .key(format!("block-{}", b.block.id))
        .w_full()
        .min_h(32.)
        .center_y()
        .gap(8.)
        .child(gutter(s, &b.block.id, index, false, 26., t))
        .child(
            row()
                .key(format!("page-link-{}", b.block.id))
                .grow(1.)
                .min_w(0.)
                .min_h(32.)
                .center_y()
                .gap(8.)
                .radius(4.)
                .hover(t.hover)
                .role("button")
                .label(title)
                .focusable(true)
                .child(el().size(20., 20.).center().child(
                    if let Some(page) = page.filter(|p| !p.icon.is_empty()) {
                        t.sized_label(&page.icon, 16., t.text)
                    } else {
                        rune::icon(&super::phosphor::FILE, 16.).color(t.muted)
                    },
                ))
                .child(t.sized_label(title, 16., t.text).truncate().min_w(0.))
                .on_press_tx(move |s: &mut State, tx| {
                    if !id.is_empty() {
                        s.open_page(&id, false);
                        tx.instant();
                    }
                }),
        )
}
