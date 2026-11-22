use super::theme::{Theme, icon_button};
use crate::{block_commands::BlockCommandMode, state::State};
use moss_core::BlockKind;
use crate::text_colors::{self, Channel};
use rune::*;
use std::hash::{Hash, Hasher};

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
    let zones = super::editor_layout::zones(tab);
    let mut zone_hasher = std::collections::hash_map::DefaultHasher::new();
    for zone in zones.iter() {
        zone.id.hash(&mut zone_hasher);
        for value in zone.rect {
            value.to_bits().hash(&mut zone_hasher);
        }
    }
    let zone_version = zone_hasher.finish();
    let marker_width = super::editor_layout::list_marker_width(
        &tab.blocks,
        crate::fonts::scale(tab.page.settings),
    );
    let blocks: Vec<_> = super::editor_layout::with_list_numbers(&tab.blocks)
        .enumerate()
        .map(|(i, (b, number))| {
            let render_item = || {
                let kind = b.block.kind;
                if kind == BlockKind::ChildPage {
                    return page_link(s, tab, b, i, t, &zones);
                }
                if kind == BlockKind::Bookmark {
                    return website_link(s, b, i, t, &zones);
                }
                if kind == BlockKind::Image {
                    return super::image_block::view(s, &b.block, i, t, available_width, &zones);
                }
                if kind == BlockKind::SimpleTable {
                    return row()
                        .key(format!("block-{}", b.block.id))
                        .w_full()
                        .min_w(0.)
                        .gap(6.)
                        .child(gutter(s, &b.block.id, i, false, 26., t, &zones))
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
                        .child(gutter(s, &b.block.id, i, b.edit.focused, 26., t, &zones));
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
                let links = if kind != BlockKind::Code {
                    crate::inline_links::links(&b.edit.text)
                } else {
                    Vec::new()
                };
                let hidden = crate::inline_links::hidden_markdown_syntax(&b.edit, &links);
                let comment_ranges = super::comments::keyed_ranges(s, &tab.page.id, &b.block.id, &b.edit.text);
                let comment_color = rune::color::oklch(0.68, 0.14, 80.);
                let overlay = if links.is_empty() {
                    None
                } else {
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
                        .min_h(line * crate::fonts::scale(tab.page.settings));
                    for (range, target) in links {
                        overlay = overlay.link(
                            Link::new(range, target).hover(LinkPaint::default().underline(t.text)),
                        );
                    }
                    Some(
                        overlay.on_link(|s: &mut State, target, tx| s.open_inline_link(target, tx)),
                    )
                };
                let rich_links = super::inputs::link_hit_layer(&b.edit, style, &hidden, t)
                    .map(|layer| layer.on_link(|s: &mut State, target, tx| s.open_inline_link(target, tx)));
                let background = super::editor_colors::block_paint(&b.block, Channel::Background, t);
                let foreground = super::editor_colors::block_paint(&b.block, Channel::Text, t);
                let mut spans: Vec<_> = text_colors::spans(&b.block, t.dark).collect();
                spans.extend(hidden);
                spans.extend(comment_ranges.iter().map(|(_, range)| Span::new(range.start, range.end).underline(comment_color)));
                let input = super::inputs::text(&b.edit, style, t)
                    .spans(spans)
                    .text_highlights(comment_ranges.into_iter().map(|(id, range)| rune::text::TextHighlight::new(range, comment_color.alpha(0.23)).key(super::comments::highlight_key(&id, false))))
                    .key(format!("block-input-{}", b.block.id))
                    .w_full()
                    .min_w(0.)
                    .min_h(line * crate::fonts::scale(tab.page.settings))
                    .color(foreground.unwrap_or(if kind == BlockKind::Todo && b.block.checked {
                        t.faint
                    } else {
                        t.text
                    }))
                    .label("Block text")
                    .on_edit(move |s: &mut State| &mut s.tabs[s.active].blocks[i].edit)
                    .on_input_change(|s: &mut State, _| {
                        s.editor_selection = None;
                        s.save_blocks();
                    })
                    .on_input_paste(move |s: &mut State, content, tx| {
                        let handled =
                            s.paste_simple_table_into_paragraph(&paste_id, false, content.text());
                        if handled {
                            tx.instant();
                        }
                        handled
                    })
                    .on_input_key(move |s: &mut State, event, tx| {
                        if super::text_toolbar::shortcut(s, &key_id, false, event, tx) {
                            return true;
                        }
                        if event.key == Key::Text("-".into())
                            && s.editor_markdown_divider(&key_id, false)
                        {
                            tx.instant();
                            return true;
                        }
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
                let marker = match kind {
                    BlockKind::Todo => todo_checkbox(
                        &b.block.id,
                        b.block.checked,
                        i,
                        false,
                        [marker_width, style.line_px()],
                        t,
                    ),
                    BlockKind::BulletedList => super::editor_layout::list_label(
                        &b.block.id, "•", style, marker_width, foreground.unwrap_or(t.text),
                    ),
                    BlockKind::NumberedList => super::editor_layout::list_label(
                        &b.block.id, format!("{number}."), style, marker_width, foreground.unwrap_or(t.text),
                    ),
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
                            b.config["collapsed"] = serde_json::json!(
                                !b.config["collapsed"].as_bool().unwrap_or(false)
                            );
                            s.save_blocks();
                        }),
                    _ => el().w(0.),
                };
                let hover_key = format!("block-hover-{}", b.block.id);
                let row = row()
                    .key(format!("block-{}", b.block.id))
                    .hover_group_shared(hover_key)
                    .w_full()
                    .min_h(30. * crate::fonts::scale(tab.page.settings))
                    .py(2. * crate::fonts::scale(tab.page.settings))
                    .gap(6.)
                    .child(gutter(
                        s,
                        &b.block.id,
                        i,
                        b.edit.focused,
                        line * crate::fonts::scale(tab.page.settings),
                        t,
                        &zones,
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
                    .child(
                        col()
                            .grow(1.)
                            .min_w(0.)
                            .child(input)
                            .children(overlay)
                            .children(rich_links)
                            .children(b.edit.text.is_empty().then(|| {
                                text("Press / to insert blocks")
                                    .key(format!("block-hint-{}", b.block.id))
                                    .font(style)
                                    .color(t.input_hint)
                                    .absolute(0., 0.)
                                    .w_full()
                                    .group_opacity(if b.edit.focused { 1. } else { 0. }, 1.)
                                    .a11y_hidden()
                            })),
                    );
                if matches!(kind, BlockKind::Code | BlockKind::Callout) {
                    row.padding(14.).radius(4.).fill(background.unwrap_or(t.sidebar))
                } else if let Some(background) = background {
                    row.radius(6.).fill(background)
                } else {
                    row
                }
            };
            let render = || super::editor_layout::decorate(s, &b.block.id, render_item(), t);
            if b.block.kind == BlockKind::Todo
                && s.editor_drag.is_none()
                && s.editor_selection.is_none()
                && super::comments::markers(s, &tab.page.id, &b.block.id, false, t).is_empty()
            {
                let menu_open = s
                    .block_commands
                    .as_ref()
                    .is_some_and(|menu| menu.block_id == b.block.id);
                let settings = tab.page.settings;
                rune::memo(
                    format!("editor-block-slot-{}", b.block.id),
                    (
                        i,
                        &b.edit,
                        &b.block.config,
                        b.block.checked,
                        (zone_version, marker_width.to_bits()),
                        menu_open,
                        settings.font as u8,
                        settings.small_text,
                        settings.full_width,
                        t.bg.r.to_bits(),
                        t.text_scale.to_bits(),
                        b.edit.focused && t.caret_visible,
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
    let selection_zones = zones.clone();
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
            s.select_editor_blocks(&selection_page, false, &selection_zones, drag, tx);
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
                        .children(
                            s.editor_drag
                                .as_ref()
                                .and_then(|drag| drag.target.as_ref())
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
                ),
        );
    col()
        .key(&viewport)
        .grow(1.)
        .min_h(0.)
        .min_w(0.)
        .w_full()
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

pub(super) fn gutter(
    s: &State,
    id: &str,
    index: usize,
    focused: bool,
    line: f32,
    t: Theme,
    zones: &std::rc::Rc<Vec<crate::editor_drag::DropZone>>,
) -> El {
    let visible = focused
        || s.block_commands
            .as_ref()
            .is_some_and(|menu| menu.block_id == id);
    let add_id = id.to_owned();
    let add_key = format!("newblock-add-{id}");
    let add_anchor = add_key.clone();
    let menu_id = id.to_owned();
    let drag_id = id.to_owned();
    let drag_zones = zones.clone();
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
                    s.drag_editor_block(&drag_id, drag, &drag_zones, false);
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
    zones: &std::rc::Rc<Vec<crate::editor_drag::DropZone>>,
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
        .child(gutter(s, &b.block.id, index, false, 26., t, zones))
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
    zones: &std::rc::Rc<Vec<crate::editor_drag::DropZone>>,
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
        .w_full()
        .min_h(32.)
        .center_y()
        .gap(8.)
        .child(gutter(s, &b.block.id, index, false, 26., t, zones))
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
