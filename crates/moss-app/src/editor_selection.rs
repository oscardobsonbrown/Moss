use crate::{
    editor_drag::DropZone,
    state::{EditableBlock, State},
};
use moss_core::{Block, BlockKind};
use rune::{Drag, Key, KeyEvent, Phase, TextEdit, Tx};

pub struct Selection {
    pub page_id: String,
    pub peek: bool,
    pub ids: Vec<String>,
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub dragging: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    #[test]
    fn multiline_task_checkbox_stays_aligned_with_the_first_line() {
        for (peek, small) in [(false, false), (false, true), (true, false), (true, true)] {
            let path =
                std::env::temp_dir().join(format!("moss-block-alignment-{}", moss_core::new_id()));
            let mut s = State::open(path.clone(), None).unwrap();
            s.new_page(peek, None);
            if peek {
                s.create_peek(Default::default());
                s.record_peek.as_mut().unwrap().page.settings.small_text = small;
                let b = &mut s.record_peek.as_mut().unwrap().blocks[0];
                b.block.kind = BlockKind::Todo;
                b.edit.replace_text("First line\nSecond line");
            } else {
                s.tabs[s.active].page.settings.small_text = small;
                let b = &mut s.tabs[s.active].blocks[0];
                b.block.kind = BlockKind::Todo;
                b.edit.replace_text("First line\nSecond line");
            }
            let id = if peek {
                &s.record_peek.as_ref().unwrap().blocks[0].block.id
            } else {
                &s.tab().unwrap().blocks[0].block.id
            }
            .clone();
            let mut rt = Runtime::with_text(
                s,
                crate::ui::view,
                (1400., 1000.),
                Env::default(),
                rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
            );
            rt.frame(0.);
            rt.frame(0.02);
            let input = rt
                .presented(&format!(
                    "{}-{id}",
                    if peek { "peek-block" } else { "block-input" }
                ))
                .unwrap()
                .0;
            let checkbox = rt
                .presented(if peek { "peek-todo-0" } else { "todo-0" })
                .unwrap()
                .0;
            assert!(
                input[3] > checkbox[3] + 10.,
                "peek={peek}: {input:?} {checkbox:?}"
            );
            assert!(
                (input[1] - checkbox[1]).abs() < 1.,
                "peek={peek}: {input:?} {checkbox:?}"
            );
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn dragging_blank_space_selects_blocks_and_delete_saves() {
        for peek in [false, true] {
            let path =
                std::env::temp_dir().join(format!("moss-block-selection-{}", moss_core::new_id()));
            let mut s = State::open(path.clone(), None).unwrap();
            s.new_page(peek, None);
            if peek {
                s.create_peek(Default::default());
                s.record_peek.as_mut().unwrap().blocks[0]
                    .edit
                    .replace_text("First");
                s.add_peek_block(0);
                s.record_peek.as_mut().unwrap().blocks[1]
                    .edit
                    .replace_text("Second");
                s.record_peek.as_mut().unwrap().blocks[1].block.kind = BlockKind::Todo;
                s.add_peek_block(1);
                s.record_peek.as_mut().unwrap().blocks[2]
                    .edit
                    .replace_text("Third");
                s.save_peek_blocks();
            } else {
                s.tabs[s.active].blocks[0].edit.replace_text("First");
                s.append_editor_block();
                s.tabs[s.active].blocks[1].edit.replace_text("Second");
                s.tabs[s.active].blocks[1].block.kind = BlockKind::Todo;
                s.append_editor_block();
                s.tabs[s.active].blocks[2].edit.replace_text("Third");
                s.save_blocks();
            }
            let (page, ids) = if peek {
                let p = s.record_peek.as_ref().unwrap();
                (
                    p.page.id.clone(),
                    p.blocks
                        .iter()
                        .map(|b| b.block.id.clone())
                        .collect::<Vec<_>>(),
                )
            } else {
                let p = s.tab().unwrap();
                (
                    p.page.id.clone(),
                    p.blocks
                        .iter()
                        .map(|b| b.block.id.clone())
                        .collect::<Vec<_>>(),
                )
            };
            let mut rt = Runtime::with_text(
                s,
                crate::ui::view,
                (1400., 1000.),
                Env {
                    system_dark: false,
                    scale: 1.,
                },
                rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
            );
            rt.on_key = Some(crate::key);
            let clipboard = rune::clipboard::memory();
            rt.use_clipboard(&clipboard);
            rt.frame(0.);
            rt.frame(0.02);
            let prefix = if peek { "peek-drag-block" } else { "block" };
            let first = rt.presented(&format!("{prefix}-{}", ids[0])).unwrap().0;
            let second = rt.presented(&format!("{prefix}-{}", ids[1])).unwrap().0;
            let starts = if peek {
                vec![first[0] + first[2] + 10.]
            } else {
                vec![first[0] - 60., first[0] - 100., first[0] + first[2] + 50.]
            };
            for x in starts {
                let start = [x, first[1] + first[3] / 2.];
                let end = [first[0] + first[2] / 2., second[1] + second[3] / 2.];
                rt.pointer_move(start[0], start[1]);
                rt.pointer_down(start[0], start[1]);
                rt.frame(0.02);
                rt.pointer_move(end[0], end[1]);
                rt.frame(0.02);
                let box_rect = rt
                    .presented("editor-selection-box")
                    .expect("selection box during drag")
                    .0;
                assert!((box_rect[0] - start[0].min(end[0])).abs() < 2.);
                assert!((box_rect[1] - start[1].min(end[1])).abs() < 2.);
                assert_eq!(rt.state.editor_selection.as_ref().unwrap().ids, ids[..2]);
                rt.pointer_up(end[0], end[1]);
                rt.frame(0.02);
                assert!(rt.presented("editor-selection-box").is_none());
                assert_eq!(
                    rt.state.editor_selection.as_ref().unwrap().ids,
                    ids[..2],
                    "peek={peek}, x={x}"
                );
                let prefix = if peek { "peek-editor" } else { "editor" };
                for id in &ids[..2] {
                    let slot = rt
                        .presented(&format!("{prefix}-block-slot-{id}"))
                        .unwrap()
                        .0;
                    let highlight = rt
                        .presented(&format!("{prefix}-block-selection-{id}"))
                        .unwrap()
                        .0;
                    let input = rt
                        .presented(&format!(
                            "{}-{id}",
                            if peek { "peek-block" } else { "block-input" }
                        ))
                        .unwrap()
                        .0;
                    assert!(
                        (highlight[0] - (slot[0] - 8.)).abs() < 1.,
                        "{highlight:?} {slot:?}"
                    );
                    assert!(
                        (highlight[2] - (slot[2] + 16.)).abs() < 1.,
                        "{highlight:?} {slot:?}"
                    );
                    assert!(
                        (input[1] + input[3] / 2. - (highlight[1] + highlight[3] / 2.)).abs() < 1.5,
                        "{input:?} {highlight:?}"
                    );
                }
                let checkbox = rt
                    .presented(if peek { "peek-todo-1" } else { "todo-1" })
                    .unwrap()
                    .0;
                let checkbox_box = rt
                    .presented(&format!(
                        "{}todo-box-{}",
                        if peek { "peek-" } else { "" },
                        ids[1]
                    ))
                    .unwrap()
                    .0;
                let paragraph = rt
                    .presented(&format!(
                        "{}-{}",
                        if peek { "peek-block" } else { "block-input" },
                        ids[0]
                    ))
                    .unwrap()
                    .0;
                assert!(
                    (checkbox_box[0] - paragraph[0]).abs() < 1.,
                    "{checkbox_box:?} {paragraph:?}"
                );
                let input = rt
                    .presented(&format!(
                        "{}-{}",
                        if peek { "peek-block" } else { "block-input" },
                        ids[1]
                    ))
                    .unwrap()
                    .0;
                assert!(
                    (checkbox[1] + checkbox[3] / 2. - (input[1] + input[3] / 2.)).abs() < 1.5,
                    "{checkbox:?} {input:?}"
                );
            }
            rt.key(KeyEvent {
                key: Key::Command('c'),
                mods: Default::default(),
            });
            rt.frame(0.02);
            assert_eq!(
                clipboard.text().as_deref().map(String::as_str),
                Some("First\nSecond")
            );
            rt.key(KeyEvent {
                key: Key::Backspace,
                mods: Default::default(),
            });
            rt.frame(0.02);
            let saved = rt.state.store.get_blocks(&page).unwrap();
            assert_eq!(saved.len(), 1, "peek={peek}");
            assert_eq!(saved[0].id, ids[2]);
            let input_key = if peek {
                format!("peek-block-{}", ids[2])
            } else {
                format!("block-input-{}", ids[2])
            };
            let input = rt.presented(&input_key).unwrap().0;
            let y = input[1] + input[3] / 2.;
            rt.pointer_down(input[0] + 4., y);
            rt.pointer_move(input[0] + 35., y);
            rt.pointer_up(input[0] + 35., y);
            rt.frame(0.02);
            assert!(
                rt.state.editor_selection.is_none(),
                "text selection started a block selection"
            );
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}

impl State {
    pub fn select_editor_blocks(
        &mut self,
        page_id: &str,
        peek: bool,
        zones: &[DropZone],
        drag: &Drag,
        tx: &mut Tx,
    ) {
        if drag.cancelled() {
            self.editor_selection = None;
        } else if drag.phase == Phase::Down {
            self.editor_selection = Some(Selection {
                page_id: page_id.into(),
                peek,
                ids: Vec::new(),
                start: drag.window_point(),
                end: drag.window_point(),
                dragging: false,
            });
        } else if let Some(selection) = self
            .editor_selection
            .as_mut()
            .filter(|selection| selection.page_id == page_id && selection.peek == peek)
        {
            if drag.dx.abs() >= 4. || drag.dy.abs() >= 4. || selection.dragging {
                selection.end = drag.window_point();
                selection.dragging = drag.phase != Phase::Up;
                let [x, y] = selection.start;
                let [end_x, end_y] = selection.end;
                let (left, right) = (x.min(end_x), x.max(end_x));
                let (top, bottom) = (y.min(end_y), y.max(end_y));
                selection.ids = zones
                    .iter()
                    .filter(|zone| {
                        zone.id != "@end"
                            && zone.rect[2] > 0.
                            && zone.rect[3] > 0.
                            && zone.rect[0] <= right
                            && zone.rect[0] + zone.rect[2] >= left
                            && zone.rect[1] <= bottom
                            && zone.rect[1] + zone.rect[3] >= top
                    })
                    .map(|zone| zone.id.clone())
                    .collect();
                tx.blur();
            }
            if drag.phase == Phase::Up && selection.ids.is_empty() {
                self.editor_selection = None;
            }
        }
        tx.instant();
    }

    pub fn editor_selection_key(&mut self, event: &KeyEvent, tx: &mut Tx) -> bool {
        let Some(selection) = self.editor_selection.as_ref().filter(|s| !s.ids.is_empty()) else {
            return false;
        };
        let valid = if selection.peek {
            self.record_peek
                .as_ref()
                .is_some_and(|p| p.page.id == selection.page_id)
        } else {
            self.record_peek.is_none() && self.tab().is_some_and(|t| t.page.id == selection.page_id)
        };
        if !valid || self.modal.is_some() || tx.focused().is_some() {
            return false;
        }
        if event.key == Key::Escape {
            self.editor_selection = None;
            tx.instant();
            return true;
        }
        let peek = selection.peek;
        let ids = selection.ids.clone();
        if matches!(event.key, Key::Up | Key::Down | Key::Enter)
            && !event.mods.cmd
            && !event.mods.alt
            && !event.mods.ctrl
            && !event.mods.shift
        {
            let indices: Vec<usize> = {
                let blocks = if peek {
                    &self.record_peek.as_ref().unwrap().blocks
                } else {
                    &self.tab().unwrap().blocks
                };
                blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| ids.contains(&b.block.id))
                    .map(|(i, _)| i)
                    .collect()
            };
            if let (Some(&first), Some(&last)) = (indices.first(), indices.last()) {
                let count = if peek {
                    self.record_peek.as_ref().unwrap().blocks.len()
                } else {
                    self.tab().unwrap().blocks.len()
                };
                match event.key {
                    Key::Up => {
                        if first == 0 {
                            self.focus_editor_title(peek, true);
                        } else {
                            self.focus_block_index(first - 1, None, peek, Some(tx));
                        }
                        tx.instant();
                        return true;
                    }
                    Key::Down => {
                        if last + 1 < count {
                            self.focus_block_index(last + 1, Some(0), peek, Some(tx));
                            tx.instant();
                            return true;
                        }
                    }
                    Key::Enter => {
                        let is_text = if peek {
                            crate::editor_commands::is_text_block(
                                self.record_peek.as_ref().unwrap().blocks[first].block.kind,
                            )
                        } else {
                            crate::editor_commands::is_text_block(
                                self.tab().unwrap().blocks[first].block.kind,
                            )
                        };
                        if is_text {
                            self.focus_block_index(first, None, peek, Some(tx));
                            tx.instant();
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }
        if event.key == Key::Command('c') {
            let (blocks, links) = if peek {
                let page = self.record_peek.as_ref().unwrap();
                (&page.blocks, &page.page_links)
            } else {
                let page = self.tab().unwrap();
                (&page.blocks, &page.page_links)
            };
            if blocks.iter().any(|block| {
                ids.contains(&block.block.id)
                    && matches!(block.block.kind, BlockKind::Image | BlockKind::SimpleTable)
            }) {
                self.error =
                    Some("Copy supports text and links, but not images or tables yet.".into());
                tx.instant();
                return true;
            }
            let text = blocks
                .iter()
                .filter(|block| ids.contains(&block.block.id))
                .map(|block| match block.block.kind {
                    BlockKind::ChildPage => block.block.config["page_id"]
                        .as_str()
                        .and_then(|id| links.get(id))
                        .map(|page| page.title.clone())
                        .unwrap_or_default(),
                    BlockKind::Bookmark => {
                        block.block.config["url"].as_str().unwrap_or("").to_owned()
                    }
                    _ => block.edit.text.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            tx.copy(text);
            return true;
        }
        if matches!(event.key, Key::Backspace | Key::Delete) {
            let page_id = selection.page_id.clone();
            let first_removed = {
                let blocks = if peek {
                    &mut self.record_peek.as_mut().unwrap().blocks
                } else {
                    &mut self.tabs[self.active].blocks
                };
                let first = blocks
                    .iter()
                    .position(|block| ids.contains(&block.block.id))
                    .unwrap_or(0);
                blocks.retain(|block| !ids.contains(&block.block.id));
                if blocks.is_empty() {
                    blocks.push(EditableBlock {
                        block: Block::paragraph(&page_id, "", 0),
                        edit: TextEdit::new("").rich(),
                    });
                }
                crate::editor_drag::normalize(blocks);
                first
            };
            self.editor_selection = None;
            self.focus_after_removal(first_removed, peek, Some(tx));
            self.save_editor_blocks(peek);
            tx.instant();
            return true;
        }
        false
    }
}
