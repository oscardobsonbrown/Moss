use super::theme::{Theme, icon_button};
use crate::text_colors::{self, Channel};
use crate::{block_commands::BlockCommandMode, state::State};
use moss_core::BlockKind;
use rune::*;

pub(super) fn wire_title_arrows(el: El, peek: bool, at_last: bool) -> El {
    el.on_input_key(move |s: &mut State, event, tx| {
        matches!(event.key, Key::Down | Key::Right)
            && !event.mods.shift
            && !event.mods.cmd
            && !event.mods.alt
            && !event.mods.ctrl
            && s.editor_title_arrow_key(peek, event.key.clone(), at_last, tx)
    })
}

pub(super) fn wire_file_drop(el: El, page_id: &str, peek: bool) -> El {
    let hover_page = page_id.to_owned();
    let move_page = page_id.to_owned();
    let drop_page = page_id.to_owned();
    el.accepts_extensions(&["png", "jpg", "jpeg", "webp"])
        .on_drop_hover(move |s: &mut State, over, tx| {
            if over {
                let point = tx.drop_point().map(|p| [p.window_x, p.window_y]);
                s.hover_editor_file_drop(&hover_page, peek, point);
            } else {
                s.clear_editor_file_drop();
            }
            tx.instant();
        })
        .on_drop_move(move |s: &mut State, point, tx| {
            s.hover_editor_file_drop(&move_page, peek, Some([point.window_x, point.window_y]));
            tx.instant();
        })
        .on_drop_files(move |s: &mut State, files, tx| {
            let point = tx.drop_point().map(|p| [p.window_x, p.window_y]);
            s.drop_editor_image_files(&drop_page, peek, files, point);
            tx.instant();
        })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_text_block(
    s: &State,
    cx: &Cx<State>,
    page_id: &str,
    settings: moss_core::PageSettings,
    b: &crate::state::EditableBlock,
    i: usize,
    number: usize,
    marker_width: f32,
    t: Theme,
    rect: Option<&rune::Live<[f32; 4]>>,
    peek: bool,
) -> El {
    let kind = b.block.kind;
    let scale = crate::fonts::scale(settings);
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
    let key_id = b.block.id.clone();
    let submit_id = b.block.id.clone();
    let paste_id = b.block.id.clone();
    let input_key = crate::editor_commands::input_key(&b.block.id, peek);
    let (at_first_line, at_last_line) = super::editor_layout::line_edges(cx, &input_key, &b.edit);
    let links = if kind != BlockKind::Code {
        crate::inline_links::links(&b.edit.text)
    } else {
        Vec::new()
    };
    let hidden = crate::inline_links::hidden_markdown_syntax(&b.edit, &links);
    let comment_ranges = super::comments::keyed_ranges(s, page_id, &b.block.id, &b.edit.text);
    let comment_color = rune::color::oklch(0.68, 0.14, 80.);
    let open_link = move |s: &mut State, target: &str, tx: &mut Tx| {
        if peek {
            s.open_peek_inline_link(target, tx);
        } else {
            s.open_inline_link(target, tx);
        }
    };
    let overlay = (!links.is_empty()).then(|| {
        let mut underlines: Vec<_> = links
            .iter()
            .map(|(range, _)| Span::new(range.start, range.end).underline(t.muted))
            .collect();
        underlines.extend(hidden.iter().cloned());
        let mut overlay = rich(&b.edit.text, underlines)
            .font(style)
            .wrap()
            .color(Color::TRANSPARENT)
            .absolute(0., 0.)
            .w_full()
            .min_h(line * scale);
        for (range, target) in links {
            overlay = overlay
                .link(Link::new(range, target).hover(LinkPaint::default().underline(t.text)));
        }
        overlay.on_link(open_link)
    });
    let rich_links = super::inputs::link_hit_layer(&b.edit, style, &hidden, t)
        .map(|layer| layer.on_link(open_link));
    let background = super::editor_colors::block_paint(&b.block, Channel::Background, t);
    let foreground = super::editor_colors::block_paint(&b.block, Channel::Text, t);
    let mut spans: Vec<_> = text_colors::spans(&b.block, t.dark).collect();
    spans.extend(hidden);
    spans.extend(
        comment_ranges
            .iter()
            .map(|(_, range)| Span::new(range.start, range.end).underline(comment_color)),
    );
    let mut input = super::inputs::text(&b.edit, style, t)
        .spans(spans)
        .text_highlights(comment_ranges.into_iter().map(|(id, range)| {
            rune::text::TextHighlight::new(range, comment_color.alpha(0.23))
                .key(super::comments::highlight_key(&id, peek))
        }))
        .key(input_key)
        .w_full()
        .min_w(0.)
        .min_h(line * scale)
        .color(
            foreground.unwrap_or(if kind == BlockKind::Todo && b.block.checked {
                t.faint
            } else {
                t.text
            }),
        )
        .label("Block text");
    if peek {
        input = input.align(Align::Start);
    }
    let input = input
        .on_edit(move |s: &mut State| {
            if peek {
                &mut s.record_peek.as_mut().unwrap().blocks[i].edit
            } else {
                &mut s.tabs[s.active].blocks[i].edit
            }
        })
        .on_input_change(move |s: &mut State, _| {
            s.editor_selection = None;
            s.save_editor_blocks(peek);
        })
        .on_input_paste(move |s: &mut State, content, tx| {
            if s.paste_editor_image_content(Some(&paste_id), peek, content) {
                tx.instant();
                return true;
            }
            let handled = s.paste_simple_table_into_paragraph(&paste_id, peek, content.text());
            if handled {
                tx.instant();
            }
            handled
        })
        .on_input_key(move |s: &mut State, event, tx| {
            if super::text_toolbar::shortcut(s, &key_id, peek, event, tx) {
                return true;
            }
            if event.key == Key::Text("-".into()) && s.editor_markdown_divider(&key_id, peek) {
                tx.instant();
                return true;
            }
            if matches!(event.key, Key::Command('b' | 'i' | 'u' | 'z')) {
                if let Some(blocks) = s.editor_blocks_mut(peek)
                    && let Some(block) = blocks.get_mut(i)
                {
                    block.edit.key(event, None);
                }
                s.save_editor_blocks(peek);
                tx.instant();
                return true;
            }
            if event.key == Key::Command('/') {
                let anchor = crate::editor_commands::input_key(&key_id, peek);
                s.open_block_commands_in(&key_id, &anchor, BlockCommandMode::Change, peek);
                tx.instant();
                return true;
            }
            if event.key == Key::Text(" ".into()) && s.editor_markdown_space_in(&key_id, peek) {
                tx.instant();
                return true;
            }
            if event.key == Key::Text("/".into())
                && kind != BlockKind::Code
                && s.editor_surface(peek)
                    .and_then(|(_, _, blocks, _)| blocks.get(i))
                    .is_some_and(|block| block.edit.text.is_empty())
            {
                let anchor = crate::editor_commands::input_key(&key_id, peek);
                s.open_block_commands_in(&key_id, &anchor, BlockCommandMode::Slash, peek);
                tx.instant();
                return true;
            }
            if event.key == Key::Enter && event.mods.shift {
                if let Some(blocks) = s.editor_blocks_mut(peek)
                    && let Some(block) = blocks.get_mut(i)
                {
                    block.edit.insert("\n");
                }
                s.save_editor_blocks(peek);
                tx.instant();
                return true;
            }
            if event.key == Key::Escape && s.editor_escape_select_block(&key_id, peek, tx) {
                return true;
            }
            let plain = !event.mods.cmd && !event.mods.alt && !event.mods.ctrl;
            if event.key == Key::Backspace
                && plain
                && s.editor_backspace_in(&key_id, peek, Some(tx))
            {
                tx.instant();
                return true;
            }
            if event.key == Key::Delete && plain && s.editor_delete_in(&key_id, peek, Some(tx)) {
                tx.instant();
                return true;
            }
            if matches!(event.key, Key::Up | Key::Down | Key::Left | Key::Right)
                && plain
                && !event.mods.shift
                && s.editor_arrow_key(
                    &key_id,
                    peek,
                    event.key.clone(),
                    at_first_line,
                    at_last_line,
                    tx,
                )
            {
                return true;
            }
            false
        })
        .on_submit(move |s: &mut State, tx| {
            tx.instant();
            if kind == BlockKind::Code {
                if let Some(blocks) = s.editor_blocks_mut(peek)
                    && let Some(block) = blocks.get_mut(i)
                {
                    block.edit.insert("\n");
                }
                s.save_editor_blocks(peek);
            } else {
                s.editor_enter_in(&submit_id, peek);
            }
        });
    let list_color = foreground.unwrap_or(if peek { t.muted } else { t.text });
    let marker = match kind {
        BlockKind::Todo => Some(todo_checkbox(
            &b.block.id,
            b.block.checked,
            i,
            peek,
            [marker_width, style.line_px()],
            t,
        )),
        BlockKind::BulletedList => Some(super::editor_layout::list_label(
            &b.block.id,
            "•",
            style,
            marker_width,
            list_color,
        )),
        BlockKind::NumberedList => Some(super::editor_layout::list_label(
            &b.block.id,
            format!("{number}."),
            style,
            marker_width,
            list_color,
        )),
        BlockKind::Quote => Some(el().w(3.).h(26.).fill(t.text)),
        BlockKind::Callout => Some(if peek {
            t.sized_label("💡", 16., t.muted).w(24.)
        } else {
            super::theme::sized_label("💡", 20., t.text).w(28.)
        }),
        BlockKind::Toggle => Some(
            row()
                .key(if peek {
                    format!("peek-toggle-{i}")
                } else {
                    format!("toggle-{i}")
                })
                .size(if peek { 24. } else { 26. }, if peek { 26. } else { 30. })
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
                    if let Some(blocks) = s.editor_blocks_mut(peek)
                        && let Some(block) = blocks.get_mut(i)
                    {
                        block.block.config["collapsed"] = serde_json::json!(
                            !block.block.config["collapsed"].as_bool().unwrap_or(false)
                        );
                        s.save_editor_blocks(peek);
                    }
                }),
        ),
        _ => None,
    };
    let text_col = col()
        .grow(1.)
        .min_w(0.)
        .child(input)
        .children(overlay)
        .children(rich_links)
        .children(b.edit.text.is_empty().then(|| {
            text("Press / to insert blocks")
                .key(format!(
                    "{}block-hint-{}",
                    if peek { "peek-" } else { "" },
                    b.block.id
                ))
                .font(style)
                .color(t.input_hint)
                .absolute(0., 0.)
                .w_full()
                .group_opacity(if b.edit.focused { 1. } else { 0. }, 1.)
                .a11y_hidden()
        }));
    let body = if peek {
        row()
            .w_full()
            .py(2. * scale)
            .gap(6.)
            .children(marker)
            .child(text_col)
    } else {
        let mut r = row()
            .key(format!("block-{}", b.block.id))
            .hover_group_shared(format!("block-hover-{}", b.block.id))
            .w_full()
            .min_h(30. * scale)
            .py(2. * scale)
            .gap(6.)
            .child(gutter(s, &b.block.id, i, b.edit.focused, line * scale, t))
            .children(marker)
            .child(text_col);
        if let Some(rect) = rect {
            r = r.drawn_rect_into(rect);
        }
        r
    };
    if matches!(kind, BlockKind::Code | BlockKind::Callout) {
        body.padding(if peek { 12. } else { 14. })
            .radius(4.)
            .fill(background.unwrap_or(t.sidebar))
    } else if !peek && let Some(background) = background {
        body.radius(6.).fill(background)
    } else {
        body
    }
}

pub fn view(s: &State, cx: &Cx<State>, t: Theme, available_width: f32) -> El {
    let tab = s.tab().unwrap();
    let title_key = format!("title-{}", tab.page.id);
    let (_, title_at_last) = super::editor_layout::line_edges(cx, &title_key, &tab.title);
    let title = wire_title_arrows(
        super::inputs::text(
            &tab.title,
            crate::fonts::page_style(tab.page.settings, 40., 700.)
                .lh(48. * crate::fonts::scale(tab.page.settings)),
            t,
        )
        .key(title_key)
        .w_full()
        .placeholder("Untitled", t.title_hint)
        .label("Page title")
        .on_edit(|s: &mut State| &mut s.tabs[s.active].title)
        .on_input_change(|s: &mut State, _| s.save_title()),
        false,
        title_at_last,
    )
    .on_submit(|s: &mut State, tx| {
        s.tabs[s.active].title.focused = false;
        s.focus_first_page_block(false);
        tx.instant();
    });
    let has_comments = super::comments::page_has_active_comments(s, &tab.page.id);
    let marker_width = super::editor_layout::list_marker_width(
        &tab.blocks,
        crate::fonts::scale(tab.page.settings),
    );
    let blocks: Vec<_> = super::editor_layout::with_list_numbers(&tab.blocks)
        .enumerate()
        .map(|(i, (b, number))| {
            let render_item = || {
                let rect = tab.block_rects.slot(&b.block.id);
                match b.block.kind {
                    BlockKind::ChildPage => page_link(s, tab, b, i, t, &rect),
                    BlockKind::Bookmark => website_link(s, b, i, t, &rect),
                    BlockKind::Image => {
                        super::image_block::view(s, &b.block, i, t, available_width, &rect)
                    }
                    BlockKind::SimpleTable => row()
                        .key(format!("block-{}", b.block.id))
                        .drawn_rect_into(&rect)
                        .w_full()
                        .min_w(0.)
                        .gap(6.)
                        .child(gutter(s, &b.block.id, i, false, 26., t))
                        .child(
                            col()
                                .grow(1.)
                                .min_w(0.)
                                .child(super::simple_table::view(s, &b.block, false, t)),
                        ),
                    BlockKind::Divider => row()
                        .key(format!("block-{}", b.block.id))
                        .drawn_rect_into(&rect)
                        .w_full()
                        .h(24.)
                        .center_y()
                        .child(el().h(1.).grow(1.).fill(t.line))
                        .child(gutter(s, &b.block.id, i, b.edit.focused, 26., t)),
                    _ => render_text_block(
                        s,
                        cx,
                        &tab.page.id,
                        tab.page.settings,
                        b,
                        i,
                        number,
                        marker_width,
                        t,
                        Some(&rect),
                        false,
                    ),
                }
            };
            let render = || super::editor_layout::decorate(s, &b.block.id, render_item(), t);
            let memoizable = matches!(
                b.block.kind,
                BlockKind::Paragraph
                    | BlockKind::Heading1
                    | BlockKind::Heading2
                    | BlockKind::Heading3
                    | BlockKind::Todo
                    | BlockKind::BulletedList
                    | BlockKind::NumberedList
                    | BlockKind::Quote
                    | BlockKind::Callout
                    | BlockKind::Code
                    | BlockKind::Toggle
                    | BlockKind::Divider
            );
            if memoizable
                && !b.edit.focused
                && s.editor_drag.is_none()
                && s.editor_file_drop.is_none()
                && s.editor_selection.is_none()
                && (!has_comments
                    || !super::comments::block_has_active_comments(s, &tab.page.id, &b.block.id))
            {
                let menu_open = s
                    .block_commands
                    .as_ref()
                    .is_some_and(|menu| menu.block_id == b.block.id);
                let settings = tab.page.settings;
                rune::memo(
                    format!("editor-block-slot-{}", b.block.id),
                    (
                        (i, std::mem::discriminant(&b.block.kind), number),
                        &b.edit,
                        &b.block.config,
                        (b.block.checked, marker_width.to_bits(), menu_open),
                        (
                            settings.font as u8,
                            settings.small_text,
                            settings.full_width,
                        ),
                        (
                            t.dark,
                            t.bg.r.to_bits(),
                            t.text_scale.to_bits(),
                            t.scale.to_bits(),
                        ),
                    ),
                    render,
                )
            } else {
                render()
            }
        })
        .collect();
    let blocks = super::editor_layout::group(s, tab, blocks, t);
    let selection_page = tab.page.id.clone();
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
    let viewport = format!("editor-selection-viewport-{}", tab.page.id);
    let scroll = col()
        .key(format!("document-scroll-{}", tab.page.id))
        .grow(1.)
        .min_h(0.)
        .min_w(0.)
        .w_full()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .on_drag(move |s: &mut State, drag, tx| {
            s.select_editor_blocks(&selection_page, false, drag, tx);
        })
        .child(super::page_cover::view(s, &tab.page, t, "document"))
        .child(
            content
                .child(
                    col()
                        .w_full()
                        .gap(4.)
                        .hover_group()
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
                                            .group_opacity(
                                                super::page_cover::header_action_visibility(
                                                    s,
                                                    &tab.page.id,
                                                    tab.title.focused,
                                                ),
                                                1.,
                                            )
                                    } else {
                                        el().key(key)
                                            .radius(6.)
                                            .hover(t.hover)
                                            .size(64., 84.)
                                            .center_y()
                                            .child(super::theme::sized_label(
                                                &tab.page.icon,
                                                64.,
                                                t.text,
                                            ))
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
                                .child(super::page_cover::controls(
                                    s,
                                    &tab.page,
                                    t,
                                    "document",
                                    tab.title.focused,
                                )),
                        )
                        .child(title),
                )
                .children(tab.record_context.as_ref().map(|(db, record)| {
                    super::record_metadata::view(s, db, record, tab.page.settings, 160., "page", t)
                }))
                .child(el().h(24.))
                .children(blocks)
                .children(super::child_pages::view(tab, t))
                .child(
                    row()
                        .key("append-block")
                        .drawn_rect_into(&tab.block_rects.slot("@end"))
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
                        })
                        .children(super::editor_layout::end_drop_guide(
                            s,
                            &tab.page.id,
                            false,
                            t,
                        )),
                ),
        );
    wire_file_drop(
        col().key(&viewport).grow(1.).min_h(0.).min_w(0.).w_full(),
        &tab.page.id,
        false,
    )
    .child(scroll)
    .children(super::editor_layout::selection_box(
        s,
        &tab.page.id,
        false,
        &viewport,
        t,
    ))
}

pub(super) fn todo_checkbox(
    id: &str,
    checked: bool,
    index: usize,
    peek: bool,
    [width, line]: [f32; 2],
    t: Theme,
) -> El {
    let id = id.to_owned();
    row()
        .key(if peek {
            format!("peek-todo-{index}")
        } else {
            format!("todo-{index}")
        })
        .size(width, line)
        .center_y()
        .role("checkbox")
        .label("Complete task")
        .checked(checked)
        .focusable(true)
        .child(
            row()
                .key(format!("{}todo-box-{id}", if peek { "peek-" } else { "" }))
                .size(16., 16.)
                .center()
                .radius(3.)
                .border(1., if checked { t.blue } else { t.muted })
                .fill(if checked { t.blue } else { Color::TRANSPARENT })
                .children(
                    checked.then(|| rune::icon(&super::phosphor::CHECK, 12.).color(Color::WHITE)),
                ),
        )
        .on_press_tx(move |s: &mut State, tx| {
            s.toggle_todo(&id, peek);
            tx.instant();
        })
}

pub(super) fn gutter(s: &State, id: &str, index: usize, focused: bool, line: f32, t: Theme) -> El {
    let visible = focused
        || s.block_commands
            .as_ref()
            .is_some_and(|menu| menu.block_id == id);
    let add_id = id.to_owned();
    let add_key = format!("newblock-add-{id}");
    let add_anchor = add_key.clone();
    let menu_id = id.to_owned();
    let drag_id = id.to_owned();
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
                .on_drag(move |s: &mut State, drag, tx| {
                    s.drag_editor_block(&drag_id, drag, false);
                    tx.instant();
                })
                .on_press(move |s: &mut State| {
                    if s.editor_drag_suppress_press {
                        s.editor_drag_suppress_press = false;
                        return;
                    }
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
    rect: &rune::Live<[f32; 4]>,
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
        .drawn_rect_into(rect)
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
                .role("link")
                .label(title)
                .focusable(true)
                .child(el().size(20., 20.).center().child(
                    if let Some(page) = page.filter(|p| !p.icon.is_empty()) {
                        t.sized_label(&page.icon, 16., t.text)
                    } else {
                        rune::icon(&super::phosphor::FILE, 16.).color(t.muted)
                    },
                ))
                .child(t.link_label(title, 16.).truncate().min_w(0.))
                .on_press_tx(move |s: &mut State, tx| {
                    if !id.is_empty() {
                        s.open_page(&id, false);
                        tx.instant();
                    }
                }),
        )
}

fn website_link(
    s: &State,
    b: &crate::state::EditableBlock,
    index: usize,
    t: Theme,
    rect: &rune::Live<[f32; 4]>,
) -> El {
    let url = b
        .block
        .config
        .get("url")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_owned();
    let label = if url.is_empty() {
        "Website unavailable"
    } else {
        &url
    };
    row()
        .key(format!("block-{}", b.block.id))
        .drawn_rect_into(rect)
        .w_full()
        .min_h(32.)
        .center_y()
        .gap(8.)
        .child(gutter(s, &b.block.id, index, false, 26., t))
        .child(
            row()
                .key(format!("website-link-{}", b.block.id))
                .grow(1.)
                .min_w(0.)
                .min_h(32.)
                .center_y()
                .gap(8.)
                .role("link")
                .focusable(true)
                .label(label)
                .hover(t.hover)
                .radius(4.)
                .child(rune::icon(&super::phosphor::LINK, 16.).color(t.muted))
                .child(t.link_label(label, 16.).truncate().min_w(0.))
                .on_press_tx(move |s: &mut State, tx| {
                    if let Err(error) = tx.open_url(&url) {
                        s.error = Some(error.to_string());
                    }
                }),
        )
}
