use super::theme::{Theme, button};
use crate::{
    simple_table::{DeleteTablePart, TableCell},
    state::State,
};
use moss_core::{Block, simple_table::SimpleTable};
use rune::*;

fn control(key: String, label: String, icon: &'static Icon, t: Theme, keyboard: bool) -> El {
    el().key(key)
        .size(24., 24.)
        .center()
        .radius(4.)
        .hover(t.hover)
        .role("button")
        .help(label.clone())
        .label(label)
        .focusable(true)
        .focus_ring(1., -1., t.blue)
        .child(
            rune::icon(icon, 16.)
                .color(t.muted)
                .group_opacity(if keyboard { 1. } else { 0. }, 1.),
        )
}

pub fn view(s: &State, block: &Block, peek: bool, t: Theme) -> El {
    let surface = if peek { "peek-" } else { "" };
    let table = match SimpleTable::read(block) {
        Ok(table) => table,
        Err(error) => return t.label(error, t.error),
    };
    let target = TableCell {
        page_id: block.page_id.clone(),
        block_id: block.id.clone(),
        row: 0,
        column: 0,
        peek,
    };
    let mut body = col()
        .w_full()
        .key(format!("{surface}simple-table-{}", block.id));
    for (r, cells) in table.rows.iter().enumerate() {
        let mut line = row().w_full().hover_group();
        for (c, value) in cells.iter().enumerate() {
            let editing = s.simple_table.editing.as_ref().is_some_and(|e| {
                e.block_id == block.id && e.peek == peek && e.row == r && e.column == c
            });
            let cell = if editing {
                super::inputs::line(&s.simple_table.input, TextStyle::new(14., 400.).lh(20.), t)
                    .key(format!("{surface}simple-table-input"))
                    .w_full()
                    .px(5.)
                    .on_edit(|s: &mut State| &mut s.simple_table.input)
                    .on_input_change(|s: &mut State, _| {
                        s.save_simple_table_draft();
                    })
                    .on_input_key(|s: &mut State, event, tx| {
                        if event.key == Key::Tab {
                            s.simple_table_step(event.mods.shift);
                            tx.instant();
                            return true;
                        }
                        false
                    })
                    .on_input_paste(|s: &mut State, content, tx| {
                        let handled = s.paste_simple_table(content.text());
                        if handled {
                            tx.instant();
                        }
                        handled
                    })
                    .on_submit(|s: &mut State, tx| {
                        s.finish_simple_table();
                        tx.instant();
                    })
            } else {
                let page = block.page_id.clone();
                let id = block.id.clone();
                row()
                    .key(format!("{surface}simple-cell-{id}-{r}-{c}"))
                    .role("button")
                    .label(format!("Row {}, column {}", r + 1, c + 1))
                    .focusable(true)
                    .w_full()
                    .h(30.)
                    .px(5.)
                    .center_y()
                    .child(t.label(value.clone(), t.text))
                    .on_press(move |s: &mut State| {
                        s.start_simple_table_cell(&page, &id, r, c, peek)
                    })
            };
            line = line.child(
                row()
                    .w(160.)
                    .h(32.)
                    .center_y()
                    .border(1., t.line)
                    .fill(
                        if (table.header_row && r == 0) || (table.header_column && c == 0) {
                            t.sidebar
                        } else {
                            t.bg
                        },
                    )
                    .child(cell),
            );
        }
        let delete = target.clone();
        line = line.child(
            control(
                format!("{surface}simple-delete-row-{}-{r}", block.id),
                format!("Delete row {}", r + 1),
                &super::phosphor::TRASH,
                t,
                s.sidebar_keyboard_controls,
            )
            .on_press(move |s: &mut State| {
                s.simple_table.deleting = Some((delete.clone(), DeleteTablePart::Row(r)))
            }),
        );
        let insert = target.clone();
        line = line.child(
            control(
                format!("{surface}simple-insert-row-{}-{r}", block.id),
                format!("Insert row after {}", r + 1),
                &super::phosphor::PLUS,
                t,
                s.sidebar_keyboard_controls,
            )
            .on_press(move |s: &mut State| {
                s.change_simple_table(insert.clone(), |t| t.insert_row(r))
            }),
        );
        body = body.child(line);
    }
    let mut columns = row().hover_group();
    for c in 0..table.rows[0].len() {
        let delete = target.clone();
        let insert = target.clone();
        columns = columns.child(
            row()
                .w(160.)
                .child(
                    control(
                        format!("{surface}simple-insert-column-{}-{c}", block.id),
                        format!("Insert column after {}", c + 1),
                        &super::phosphor::PLUS,
                        t,
                        s.sidebar_keyboard_controls,
                    )
                    .on_press(move |s: &mut State| {
                        s.change_simple_table(insert.clone(), |t| t.insert_column(c))
                    }),
                )
                .child(
                    control(
                        format!("{surface}simple-delete-column-{}-{c}", block.id),
                        format!("Delete column {}", c + 1),
                        &super::phosphor::TRASH,
                        t,
                        s.sidebar_keyboard_controls,
                    )
                    .on_press(move |s: &mut State| {
                        s.simple_table.deleting = Some((delete.clone(), DeleteTablePart::Column(c)))
                    }),
                ),
        );
    }
    let add_row = target.clone();
    let last_row = table.rows.len() - 1;
    let add_column = target.clone();
    let last_column = table.rows[0].len() - 1;
    let header_row = target.clone();
    let header_column = target.clone();
    let markdown = table.to_markdown().unwrap_or_default();
    body = body.child(columns).child(
        row()
            .gap(4.)
            .child(
                button(
                    format!("{surface}simple-add-row-{}", block.id),
                    "Add row",
                    t,
                )
                .on_press(move |s: &mut State| {
                    s.change_simple_table(add_row.clone(), |t| t.insert_row(last_row))
                }),
            )
            .child(
                button(
                    format!("{surface}simple-add-column-{}", block.id),
                    "Add column",
                    t,
                )
                .on_press(move |s: &mut State| {
                    s.change_simple_table(add_column.clone(), |t| t.insert_column(last_column))
                }),
            )
            .child(
                button(
                    format!("{surface}simple-header-row-{}", block.id),
                    "Header row",
                    t,
                )
                .on_press(move |s: &mut State| {
                    s.change_simple_table(header_row.clone(), |t| {
                        t.header_row = !t.header_row;
                        Ok(())
                    })
                }),
            )
            .child(
                button(
                    format!("{surface}simple-header-column-{}", block.id),
                    "Header column",
                    t,
                )
                .on_press(move |s: &mut State| {
                    s.change_simple_table(header_column.clone(), |t| {
                        t.header_column = !t.header_column;
                        Ok(())
                    })
                }),
            )
            .child(
                button(
                    format!("{surface}simple-copy-{}", block.id),
                    "Copy Markdown",
                    t,
                )
                .on_press_tx(move |_: &mut State, tx| tx.copy(markdown.clone())),
            ),
    );
    if s.simple_table
        .deleting
        .as_ref()
        .is_some_and(|(part, _)| part.block_id == block.id && part.peek == peek)
    {
        body = body.child(
            row()
                .gap(6.)
                .center_y()
                .child(t.label("Delete this row or column?", t.error))
                .child(
                    button(format!("{surface}simple-delete-confirm"), "Delete", t).on_press(
                        |s: &mut State| {
                            if let Some((target, part)) = s.simple_table.deleting.take() {
                                s.change_simple_table(target, |t| match part {
                                    DeleteTablePart::Row(i) => t.delete_row(i),
                                    DeleteTablePart::Column(i) => t.delete_column(i),
                                });
                            }
                        },
                    ),
                )
                .child(
                    button(format!("{surface}simple-delete-cancel"), "Cancel", t)
                        .on_press(|s: &mut State| s.simple_table.deleting = None),
                ),
        );
    }
    if let Some(error) = &s.simple_table.error {
        body = body.child(t.label(error, t.error));
    }
    row().w_full().scroll_x().child(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::{BlockKind, new_id};
    use rune::{Cx, Env, Runtime};
    use std::collections::HashSet;

    fn two_surfaces(s: &State, _: &mut Cx<State>) -> El {
        let block = &s.tab().unwrap().blocks[0].block;
        row()
            .child(view(s, block, false, Theme::new(false)))
            .child(view(s, block, true, Theme::new(false)))
    }
    #[test]
    fn page_and_peek_have_distinct_control_keys() {
        let path = std::env::temp_dir().join(format!("moss-table-keys-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(false, None);
        s.tabs[s.active].blocks[0].block.kind = BlockKind::SimpleTable;
        let id = s.tab().unwrap().blocks[0].block.id.clone();
        let mut rt = Runtime::new(
            s,
            two_surfaces,
            (1800., 900.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.frame(0.);
        let keys: Vec<String> = rt
            .elements()
            .into_iter()
            .filter_map(|e| e.key)
            .filter(|k| k.contains("simple-"))
            .collect();
        assert!(keys.contains(&format!("simple-cell-{id}-0-0")));
        assert!(keys.contains(&format!("peek-simple-cell-{id}-0-0")));
        assert_eq!(keys.len(), keys.iter().collect::<HashSet<_>>().len());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
