use super::theme::{Theme, icon_button};
use crate::state::{PeekMode, State};
use moss_core::BlockKind;
use rune::*;
use crate::text_colors::{self, Channel};

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
    let comments_open = s.comments.open && s.comments.page_id.as_ref() == Some(&peek.page.id);
    let history_open = s.history.open && s.history.page_id.as_ref() == Some(&peek.page.id);
    let contents = if comments_open || history_open {
        row()
            .child(panel(s, t).grow(1.).min_w(0.).h_full())
            .children(super::comments::panel(s, t, 300.))
            .children(super::history::panel(s, t, 300.))
    } else {
        panel(s, t)
    };
    Some(
        el().key("record-peek-scrim")
            .pin(Some(0.), Some(0.), Some(0.), Some(0.))
            .fill(Color::BLACK.alpha(0.28))
            .center()
            .on_press(|s: &mut State| {
                s.dismiss_cell_picker();
                s.close_peek();
            })
            .child(
                contents
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
                        s.dismiss_cell_picker();
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
            col()
                .w_full()
                .gap(4.)
                .hover_group()
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
                                    .group_opacity(
                                        super::page_cover::header_action_visibility(
                                            s,
                                            &peek.page.id,
                                            peek.title.focused,
                                        ),
                                        1.,
                                    )
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
                                .on_press(move |s: &mut State| {
                                    s.open_page_icon_picker(&id, &anchor)
                                })
                        })
                        .child(super::page_cover::controls(
                            s,
                            &peek.page,
                            t,
                            "peek",
                            peek.title.focused,
                        )),
                )
                .child(title),
        )
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
    let zones = super::editor_layout::peek_zones(peek);
    let selection_page = page_id.clone();
    let selection_zones = zones.clone();
    let marker_width = super::editor_layout::list_marker_width(&peek.blocks, scale);
    let mut block_views = Vec::with_capacity(peek.blocks.len());
    for (i, (block, number)) in super::editor_layout::with_list_numbers(&peek.blocks).enumerate() {
        let view = (|| {
            let kind = block.block.kind;
            if kind == BlockKind::Image {
                let width = rune::memo::layout_with(panel_key, |rect| rect[2]).unwrap_or(420.);
                return super::image_block::content(s, &block.block, t, width);
            }
            if kind == BlockKind::ChildPage {
                let id = block
                    .block
                    .config
                    .get("page_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let title = peek
                    .page_links
                    .get(&id)
                    .map(|p| p.title.as_str())
                    .unwrap_or("Page unavailable");
                return super::theme::link_button(
                    format!("peek-page-link-{}", block.block.id),
                    title,
                    t,
                )
                .on_press_tx(move |s: &mut State, tx| {
                    if s.close_peek() {
                        s.open_page(&id, false);
                        tx.instant();
                    }
                });
            }
            if kind == BlockKind::Bookmark {
                let url = block
                    .block
                    .config
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                return super::theme::link_button(
                    format!("peek-website-link-{}", block.block.id),
                    &url,
                    t,
                )
                .on_press_tx(move |s: &mut State, tx| {
                    if let Err(error) = tx.open_url(&url) {
                        s.error = Some(error.to_string());
                    }
                });
            }
            if kind == BlockKind::SimpleTable {
                return super::simple_table::view(s, &block.block, true, t);
            }
            if kind == BlockKind::Divider {
                return row()
                    .w_full()
                    .h(24.)
                    .center_y()
                    .child(el().w_full().h(1.).fill(t.line));
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
            let divider_id = block.block.id.clone();
            let links = if kind != BlockKind::Code {
                crate::inline_links::links(&block.edit.text)
            } else {
                Vec::new()
            };
            let hidden = crate::inline_links::hidden_markdown_syntax(&block.edit, &links);
            let comment_ranges = super::comments::keyed_ranges(s, page_id, &block.block.id, &block.edit.text);
            let comment_color = rune::color::oklch(0.68, 0.14, 80.);
            let overlay = if links.is_empty() {
                None
            } else {
                let mut underlines: Vec<_> = links
                    .iter()
                    .map(|(range, _)| Span::new(range.start, range.end).underline(t.muted))
                    .collect();
                underlines.extend(hidden.iter().cloned());
                let mut overlay = rich(&block.edit.text, underlines)
                    .font(style)
                    .wrap()
                    .color(Color::TRANSPARENT)
                    .absolute(0., 0.)
                    .w_full()
                    .min_h(line * scale);
                for (range, target) in links {
                    overlay = overlay.link(
                        Link::new(range, target).hover(LinkPaint::default().underline(t.text)),
                    );
                }
                Some(
                    overlay
                        .on_link(|s: &mut State, target, tx| s.open_peek_inline_link(target, tx)),
                )
            };
            let rich_links = super::inputs::link_hit_layer(&block.edit, style, &hidden, t)
                .map(|layer| layer.on_link(|s: &mut State, target, tx| s.open_peek_inline_link(target, tx)));
            let background = super::editor_colors::block_paint(&block.block, Channel::Background, t);
            let foreground = super::editor_colors::block_paint(&block.block, Channel::Text, t);
            let mut spans: Vec<_> = text_colors::spans(&block.block, t.dark).collect();
            spans.extend(hidden);
            spans.extend(comment_ranges.iter().map(|(_, range)| Span::new(range.start, range.end).underline(comment_color)));
            let input = super::inputs::text(&block.edit, style, t)
                .spans(spans)
                .text_highlights(comment_ranges.into_iter().map(|(id, range)| rune::text::TextHighlight::new(range, comment_color.alpha(0.23)).key(super::comments::highlight_key(&id, true))))
                .key(format!("peek-block-{}", block.block.id))
                .align(Align::Start)
                .w_full()
                .min_w(0.)
                .min_h(line * scale)
                .color(foreground.unwrap_or(if kind == BlockKind::Todo && block.block.checked {
                    t.faint
                } else {
                    t.text
                }))
                .placeholder(if i == 0 { "Type something…" } else { "" }, t.input_hint)
                .label("Block text")
                .on_edit(move |s: &mut State| &mut s.record_peek.as_mut().unwrap().blocks[i].edit)
                .on_input_change(|s: &mut State, _| {
                    s.editor_selection = None;
                    s.save_peek_blocks();
                })
                .on_input_paste(move |s: &mut State, content, tx| {
                    let handled =
                        s.paste_simple_table_into_paragraph(&paste_id, true, content.text());
                    if handled {
                        tx.instant();
                    }
                    handled
                })
                .on_input_key(move |s: &mut State, event, tx| {
                    if super::text_toolbar::shortcut(s, &divider_id, true, event, tx) {
                        return true;
                    }
                    if event.key == Key::Text("-".into())
                        && s.editor_markdown_divider(&divider_id, true)
                    {
                        tx.instant();
                        return true;
                    }
                    if matches!(event.key, Key::Command('b' | 'i' | 'u' | 'z')) {
                        s.record_peek.as_mut().unwrap().blocks[i]
                            .edit
                            .key(event, None);
                        s.save_peek_blocks();
                        tx.instant();
                        return true;
                    }
                    if event.key == Key::Text("/".into())
                        && kind != BlockKind::Code
                        && s.record_peek
                            .as_ref()
                            .is_some_and(|peek| peek.blocks[i].edit.text.is_empty())
                    {
                        let id = s.record_peek.as_ref().unwrap().blocks[i].block.id.clone();
                        s.open_peek_block_commands(
                            &id,
                            &format!("peek-block-{id}"),
                            crate::block_commands::BlockCommandMode::Slash,
                        );
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
                BlockKind::BulletedList => Some(super::editor_layout::list_label(
                    &block.block.id, "•", style, marker_width, foreground.unwrap_or(t.muted),
                )),
                BlockKind::NumberedList => Some(super::editor_layout::list_label(
                    &block.block.id, format!("{number}."), style, marker_width, foreground.unwrap_or(t.muted),
                )),
                BlockKind::Todo => Some(super::editor::todo_checkbox(
                    &block.block.id,
                    block.block.checked,
                    i,
                    true,
                    [marker_width, style.line_px()],
                    t,
                )),
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
                .py(2. * scale)
                .gap(6.)
                .children(marker)
                .child(col().grow(1.).min_w(0.).child(input).children(overlay).children(rich_links));
            if matches!(kind, BlockKind::Code | BlockKind::Callout) {
                item.padding(12.).radius(4.).fill(background.unwrap_or(t.sidebar))
            } else {
                item
            }
        })();
        let background = super::editor_colors::block_paint(&block.block, Channel::Background, t);
        let row = row()
            .key(format!("peek-drag-block-{}", block.block.id))
            .w_full()
            .min_w(0.)
            .hover_group_shared(format!("peek-block-hover-{}", block.block.id))
            .child(peek_gutter(
                s,
                &block.block.id,
                i,
                block.edit.focused,
                t,
                &zones,
            ))
            .child(view.grow(1.).min_w(0.));
        let row = if let Some(background) = background { row.radius(6.).fill(background) } else { row };
        block_views.push(super::editor_layout::decorate_for(
            s,
            page_id,
            &block.block.id,
            row,
            t,
            true,
        ));
    }
    content = content
        .children(super::editor_layout::group_peek(s, peek, block_views, t))
        .child(
            el().key("peek-append-block").w_full().h(20.).children(
                s.editor_drag
                    .as_ref()
                    .filter(|d| d.peek)
                    .and_then(|d| d.target.as_ref())
                    .filter(|target| target.id == "@end")
                    .map(|_| {
                        el().key("editor-block-drop-guide")
                            .absolute(0., 0.)
                            .w_full()
                            .h(3.)
                            .fill(t.blue)
                            .a11y_hidden()
                    }),
            ),
        );
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
                    s.dismiss_cell_picker();
                    s.expand_peek();
                    tx.instant();
                }),
        )
        .children(super::shell::presence(s, page_id, "peek-", t))
        .child(el().w(1.).h(16.).fill(t.line))
        .child(
            icon_button("peek-close", "Close", "×", t).on_press(|s: &mut State| {
                s.dismiss_cell_picker();
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
                .key("peek-selection-viewport")
                .w_full()
                .grow(1.)
                .min_h(0.)
                .child(
                    col()
                        .key("peek-scroll")
                        .w_full()
                        .grow(1.)
                        .min_h(0.)
                        .scroll(ScrollSpec {
                            stick_bottom: false,
                        })
                        .on_drag(move |s: &mut State, drag, tx| {
                            s.select_editor_blocks(
                                &selection_page,
                                true,
                                &selection_zones,
                                drag,
                                tx,
                            );
                        })
                        .child(
                            col()
                                .w_full()
                                .child(super::page_cover::view(s, &peek.page, t, "peek"))
                                .child(content),
                        ),
                )
                .children(super::editor_layout::selection_box(
                    s,
                    page_id,
                    true,
                    "peek-selection-viewport",
                    t,
                )),
        )
        .child(header)
}

fn peek_gutter(
    s: &State,
    id: &str,
    index: usize,
    focused: bool,
    t: Theme,
    zones: &std::rc::Rc<Vec<crate::editor_drag::DropZone>>,
) -> El {
    use crate::block_commands::BlockCommandMode;
    let visible = focused
        || s.block_commands
            .as_ref()
            .is_some_and(|menu| menu.peek && menu.block_id == id);
    let add_id = id.to_owned();
    let add_key = format!("peek-add-{id}");
    let anchor = add_key.clone();
    let menu_id = id.to_owned();
    let drag_id = id.to_owned();
    let drag_zones = zones.clone();
    let menu_key = format!("peek-block-menu-{index}");
    let menu_anchor = menu_key.clone();
    row()
        .key(format!("peek-gutter-{id}"))
        .absolute(-32., 0.)
        .size(32., 30.)
        .hover_group_shared(format!("peek-block-hover-{id}"))
        .child(
            icon_button(add_key, "Insert block below", "+", t)
                .size(16., 24.)
                .mt(1.)
                .group_opacity(if visible { 1. } else { 0. }, 1.)
                .preserve_focus_on_press(true)
                .on_press(move |s: &mut State| {
                    s.open_peek_block_commands(&add_id, &anchor, BlockCommandMode::InsertAfter)
                }),
        )
        .child(
            icon_button(menu_key, "Block actions", "⠿", t)
                .size(16., 24.)
                .mt(1.)
                .group_opacity(if visible { 1. } else { 0. }, 1.)
                .preserve_focus_on_press(true)
                .on_drag(move |s: &mut State, drag, tx| {
                    s.drag_editor_block(&drag_id, drag, &drag_zones, true);
                    tx.instant();
                })
                .on_press(move |s: &mut State| {
                    if s.editor_drag_suppress_press {
                        s.editor_drag_suppress_press = false;
                        return;
                    }
                    s.open_peek_block_commands(&menu_id, &menu_anchor, BlockCommandMode::Change)
                }),
        )
}
