use super::theme::Theme;
use crate::{
    editor_drag::{self, Side},
    state::{EditableBlock, RecordPeek, State, Tab},
};
use rune::*;
use std::collections::BTreeMap;

pub fn line_edges(cx: &Cx<State>, key: &str, edit: &TextEdit) -> (bool, bool) {
    if !edit.focused {
        return (false, false);
    }
    if let (Some(caret), Some(rect)) = (cx.caret(key), cx.window_layout(key))
        && caret[3] > 0.
        && rect[3] > 0.
    {
        let at_first = caret[1] <= rect[1] + caret[3] * 0.5;
        let at_last = caret[1] + caret[3] >= rect[1] + rect[3] - caret[3] * 0.5;
        return (at_first, at_last);
    }
    let pos = edit.focus.min(edit.text.len());
    let at_first = !edit.text[..pos].contains('\n');
    let at_last = !edit.text[pos..].contains('\n');
    (at_first, at_last)
}

pub fn end_drop_guide(s: &State, page_id: &str, peek: bool, t: Theme) -> Option<El> {
    let block_target = s
        .editor_drag
        .as_ref()
        .filter(|d| d.page_id == page_id && d.peek == peek)
        .and_then(|d| d.target.as_ref())
        .is_some_and(|target| target.id == "@end");
    let file_target = s
        .editor_file_drop
        .as_ref()
        .filter(|d| d.page_id == page_id && d.peek == peek)
        .and_then(|d| d.target.as_ref())
        .is_some_and(|target| target.id == "@end");
    (block_target || file_target).then(|| {
        el().key("editor-block-drop-guide")
            .absolute(0., 0.)
            .w_full()
            .h(3.)
            .fill(t.blue)
            .a11y_hidden()
    })
}

pub fn with_list_numbers(
    blocks: &[EditableBlock],
) -> impl Iterator<Item = (&EditableBlock, usize)> {
    blocks.iter().scan(0, |number, block| {
        if block.block.kind == moss_core::BlockKind::NumberedList {
            *number += 1;
        } else {
            *number = 0;
        }
        Some((block, *number))
    })
}

pub fn list_marker_width(blocks: &[EditableBlock], scale: f32) -> f32 {
    let last = with_list_numbers(blocks)
        .map(|(_, number)| number)
        .max()
        .unwrap_or(0);
    let digits_and_period = last.max(1).ilog10() + 2;
    30_f32.max(digits_and_period as f32 * 10. * scale)
}

pub fn list_label(
    id: &str,
    label: impl Into<String>,
    style: TextStyle,
    width: f32,
    color: Color,
) -> El {
    text(label)
        .key(format!("list-marker-{id}"))
        .font(style)
        .color(color)
        .size(width, style.line_px())
}

pub fn selection_box(s: &State, page_id: &str, peek: bool, viewport: &str, t: Theme) -> Option<El> {
    let selection = s.editor_selection.as_ref().filter(|selection| {
        selection.page_id == page_id && selection.peek == peek && selection.dragging
    })?;
    let rect = rune::memo::window_layout_with(viewport, |rect| rect)?;
    let [x, y] = selection.start;
    let [end_x, end_y] = selection.end;
    Some(
        el().key("editor-selection-box")
            .absolute(x.min(end_x) - rect[0], y.min(end_y) - rect[1])
            .size((end_x - x).abs().max(1.), (end_y - y).abs().max(1.))
            .fill(t.blue.alpha(0.12))
            .border(1., t.blue)
            .inert()
            .a11y_hidden()
            .raise(),
    )
}
pub fn decorate(s: &State, id: &str, item: El, t: Theme) -> El {
    let page_id = &s.tab().unwrap().page.id;
    decorate_for(s, page_id, id, item, t, false)
}
pub fn decorate_for(s: &State, page_id: &str, id: &str, item: El, t: Theme, peek: bool) -> El {
    let active = s
        .editor_drag
        .as_ref()
        .filter(|d| d.page_id == page_id && d.peek == peek);
    let moving = active.filter(|d| d.block_id == id);
    let target = active
        .and_then(|d| d.target.as_ref())
        .filter(|d| d.id == id)
        .or_else(|| {
            s.editor_file_drop
                .as_ref()
                .filter(|d| d.page_id == page_id && d.peek == peek)
                .and_then(|d| d.target.as_ref())
                .filter(|d| d.id == id)
        });
    let marker = target.map(|target| {
        let line = el()
            .key("editor-block-drop-guide")
            .fill(t.blue)
            .a11y_hidden();
        match target.side {
            Side::Before => line.absolute(0., -2.).w_full().h(3.),
            Side::After => line.pin(Some(0.), None, Some(0.), Some(-2.)).h(3.),
            Side::Left => line.pin(Some(-2.), Some(0.), None, Some(0.)).w(3.),
            Side::Right => line.pin(None, Some(0.), Some(-2.), Some(0.)).w(3.),
        }
    });
    let prefix = if peek { "peek-editor" } else { "editor" };
    let mut preview = col()
        .key(format!("{prefix}-drag-preview-{id}"))
        .w_full()
        .child(item);
    let selected = s.editor_selection.as_ref().is_some_and(|selection| {
        selection.page_id == page_id
            && selection.peek == peek
            && selection.ids.iter().any(|selected| selected == id)
            && !(if peek {
                s.record_peek.as_ref().is_some_and(|page| {
                    page.title.focused || page.blocks.iter().any(|block| block.edit.focused)
                })
            } else {
                s.tab().is_some_and(|page| {
                    page.title.focused || page.blocks.iter().any(|block| block.edit.focused)
                })
            })
    });
    let mut slot = col()
        .key(format!("{prefix}-block-slot-{id}"))
        .w_full()
        .min_w(0.);
    if let Some(drag) = moving {
        preview = preview
            .absolute(drag.delta[0], drag.delta[1])
            .opacity(0.7)
            .inert()
            .raise();
        slot = slot.h(0.);
    }
    slot.children(selected.then(|| {
        el().key(format!("{prefix}-block-selection-{id}"))
            .pin(Some(-8.), Some(0.), Some(-8.), Some(0.))
            .radius(6.)
            .fill(t.blue.alpha(0.16))
            .inert()
            .a11y_hidden()
    }))
    .child(preview)
    .children(marker)
    .children(super::comments::markers(s, page_id, id, peek, t))
}
pub fn group(s: &State, tab: &Tab, blocks: Vec<El>, t: Theme) -> Vec<El> {
    group_for(s, &tab.page.id, &tab.blocks, blocks, t, false)
}
pub fn group_peek(s: &State, peek: &RecordPeek, blocks: Vec<El>, t: Theme) -> Vec<El> {
    group_for(s, &peek.page.id, &peek.blocks, blocks, t, true)
}
fn group_for(
    s: &State,
    page_id: &str,
    source: &[EditableBlock],
    blocks: Vec<El>,
    t: Theme,
    peek: bool,
) -> Vec<El> {
    let mut out = Vec::new();
    let mut iter = blocks.into_iter().enumerate().peekable();
    while let Some((i, element)) = iter.next() {
        let Some((group, index)) = editor_drag::column(&source[i].block) else {
            out.push(element);
            continue;
        };
        let mut columns: BTreeMap<usize, (f32, Vec<El>)> = BTreeMap::new();
        columns
            .entry(index)
            .or_insert_with(|| (editor_drag::column_weight(&source[i].block), Vec::new()))
            .1
            .push(element);
        while let Some((j, _)) = iter.peek() {
            let Some((next_group, next_index)) = editor_drag::column(&source[*j].block) else {
                break;
            };
            if next_group != group {
                break;
            }
            let weight = editor_drag::column_weight(&source[*j].block);
            let (_, element) = iter.next().unwrap();
            columns
                .entry(next_index)
                .or_insert_with(|| (weight, Vec::new()))
                .1
                .push(element);
        }
        if columns.len() < 2 {
            out.extend(columns.into_values().flat_map(|(_, elements)| elements));
            continue;
        }
        let indexes: Vec<_> = columns.keys().copied().collect();
        let prefix = if peek { "peek-editor" } else { "editor" };
        let mut layout = row()
            .key(format!("{prefix}-column-group-{group}"))
            .w_full()
            .min_w(0.)
            .items_start();
        for (position, (index, (weight, children))) in columns.into_iter().enumerate() {
            let weight = s
                .editor_column_resize
                .as_ref()
                .filter(|r| r.page_id == page_id && r.group == group)
                .map(|r| {
                    if r.left == index {
                        r.left_weight
                    } else if r.right == index {
                        r.right_weight
                    } else {
                        weight
                    }
                })
                .unwrap_or(weight);
            layout = layout.child(
                col()
                    .key(format!("{prefix}-column-{group}-{index}"))
                    .w(0.)
                    .grow(weight)
                    .min_w(0.)
                    .gap(4.)
                    .children(children),
            );
            if let Some(&next) = indexes.get(position + 1) {
                let left_width =
                    rune::memo::layout_with(&format!("{prefix}-column-{group}-{index}"), |r| r[2])
                        .unwrap_or(100.);
                let right_width =
                    rune::memo::layout_with(&format!("{prefix}-column-{group}-{next}"), |r| r[2])
                        .unwrap_or(100.);
                let id = group.clone();
                let step_id = group.clone();
                layout = layout.child(
                    row().w(60.).min_h(30.).self_stretch().items_start().child(
                        row()
                            .key(format!("{prefix}-column-resize-{group}-{index}"))
                            .w(8.)
                            .min_h(30.)
                            .self_stretch()
                            .center()
                            .hover_group()
                            .cursor(Cursor::ResizeColumn)
                            .role("separator")
                            .label("Resize columns. Use Left and Right arrows.")
                            .focusable(true)
                            .on_press_tx(|_: &mut State, tx| tx.instant())
                            .on_step(move |s: &mut State, step, tx| {
                                s.step_editor_columns(
                                    &step_id,
                                    index,
                                    next,
                                    [left_width, right_width],
                                    step,
                                    peek,
                                );
                                tx.instant();
                            })
                            .child(el().w(1.).h_full().fill(t.muted).group_opacity(0., 1.))
                            .on_drag(move |s: &mut State, drag, tx| {
                                s.resize_editor_columns(
                                    &id,
                                    index,
                                    next,
                                    [left_width, right_width],
                                    drag,
                                    peek,
                                );
                                tx.instant();
                            }),
                    ),
                );
            }
        }
        out.push(layout);
    }
    out
}
