use super::theme::{Theme, icon_button};
use crate::state::{PeekMode, State};
use crate::text_colors::Channel;
use moss_core::BlockKind;
use rune::*;

pub fn side(s: &State, cx: &Cx<State>, t: Theme, width: f32) -> Option<El> {
    let peek = s.record_peek.as_ref()?;
    if peek.mode != PeekMode::Side {
        return None;
    }
    Some(
        panel(s, cx, t)
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

pub fn center(s: &State, cx: &Cx<State>, t: Theme) -> Option<El> {
    let peek = s.record_peek.as_ref()?;
    if peek.mode != PeekMode::Center {
        return None;
    }
    let comments_open = s.comments.open && s.comments.page_id.as_ref() == Some(&peek.page.id);
    let history_open = s.history.open && s.history.page_id.as_ref() == Some(&peek.page.id);
    let contents = if comments_open || history_open {
        row()
            .child(panel(s, cx, t).grow(1.).min_w(0.).h_full())
            .children(super::comments::panel(s, t, 300.))
            .children(super::history::panel(s, t, 300.))
    } else {
        panel(s, cx, t)
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

fn panel(s: &State, cx: &Cx<State>, t: Theme) -> El {
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
    let title_key = format!("peek-title-{page_id}");
    let (_, title_at_last) = super::editor_layout::line_edges(cx, &title_key, &peek.title);
    let title = super::editor::wire_title_arrows(
        super::inputs::text(
            &peek.title,
            crate::fonts::page_style(settings, 32., 700.).lh(40. * scale),
            t,
        )
        .key(title_key)
        .w_full()
        .min_h(40. * scale)
        .align(Align::Start)
        .placeholder("Untitled", t.title_hint)
        .label("Record title")
        .on_edit(|s: &mut State| &mut s.record_peek.as_mut().unwrap().title)
        .on_input_change(|s: &mut State, _| s.save_peek_title()),
        true,
        title_at_last,
    )
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
                                super::page_cover::header_action(key, "Add icon", t).group_opacity(
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
    let selection_page = page_id.clone();
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
            super::editor::render_text_block(
                s,
                cx,
                page_id,
                settings,
                block,
                i,
                number,
                marker_width,
                t,
                None,
                true,
            )
        })();
        let background = super::editor_colors::block_paint(&block.block, Channel::Background, t);
        let row = row()
            .key(format!("peek-drag-block-{}", block.block.id))
            .drawn_rect_into(&peek.block_rects.slot(&block.block.id))
            .w_full()
            .min_w(0.)
            .hover_group_shared(format!("peek-block-hover-{}", block.block.id))
            .child(peek_gutter(s, &block.block.id, i, block.edit.focused, t))
            .child(view.grow(1.).min_w(0.));
        let row = if let Some(background) = background {
            row.radius(6.).fill(background)
        } else {
            row
        };
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
            row()
                .key("peek-append-block")
                .drawn_rect_into(&peek.block_rects.slot("@end"))
                .w_full()
                .h(30.)
                .center_y()
                .role("button")
                .label("Start a new block")
                .focusable(true)
                .on_press_tx(|s: &mut State, tx| {
                    tx.instant();
                    s.append_editor_block_in(true);
                })
                .children(super::editor_layout::end_drop_guide(s, page_id, true, t)),
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
            super::editor::wire_file_drop(
                col()
                    .key("peek-selection-viewport")
                    .w_full()
                    .grow(1.)
                    .min_h(0.),
                page_id,
                true,
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
                    .on_drag(move |s: &mut State, drag, tx| {
                        s.select_editor_blocks(&selection_page, true, drag, tx);
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

fn peek_gutter(s: &State, id: &str, index: usize, focused: bool, t: Theme) -> El {
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
                    s.drag_editor_block(&drag_id, drag, true);
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
