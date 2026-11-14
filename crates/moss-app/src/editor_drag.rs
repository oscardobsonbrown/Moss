use crate::state::{EditableBlock, State};
use moss_core::Block;
use rune::{Drag, Phase};
use serde_json::json;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Before,
    After,
    Left,
    Right,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DropZone {
    pub id: String,
    pub rect: [f32; 4],
}
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub id: String,
    pub side: Side,
}
pub struct BlockDrag {
    pub page_id: String,
    pub block_id: String,
    pub target: Option<Target>,
    pub delta: [f32; 2],
    pub rect: [f32; 4],
    pub peek: bool,
}
pub struct ColumnResize {
    pub page_id: String,
    pub group: String,
    pub left: usize,
    pub right: usize,
    pub left_weight: f32,
    pub right_weight: f32,
    start_left: f32,
    start_right: f32,
    total_width: f32,
    start_width: f32,
    peek: bool,
}

pub fn column(block: &Block) -> Option<(String, usize)> {
    let layout = block.config.get("moss_columns")?;
    Some((
        layout.get("group")?.as_str()?.to_owned(),
        usize::try_from(layout.get("column")?.as_u64()?).ok()?,
    ))
}
pub fn column_weight(block: &Block) -> f32 {
    block
        .config
        .get("moss_columns")
        .and_then(|v| v.get("weight"))
        .and_then(serde_json::Value::as_f64)
        .filter(|v| v.is_finite() && *v > 0.001 && *v < 1000.)
        .unwrap_or(1.) as f32
}
fn set_column(block: &mut Block, layout: Option<(&str, usize)>) -> bool {
    if !block.config.is_object() && !block.config.is_null() {
        return false;
    }
    if block.config.is_null() {
        block.config = json!({});
    }
    if let Some((group, index)) = layout {
        let weight = column(block)
            .filter(|(old, _)| old == group)
            .map(|_| column_weight(block))
            .unwrap_or(1.);
        block.config["moss_columns"] = json!({"group":group,"column":index,"weight":weight});
    } else if let Some(config) = block.config.as_object_mut() {
        config.remove("moss_columns");
    }
    true
}
pub fn copy_column(from: &Block, to: &mut Block) {
    if let Some((group, index)) = column(from) {
        set_column(to, Some((&group, index)));
        to.config["moss_columns"]["weight"] = json!(column_weight(from));
    }
}
pub fn normalize(blocks: &mut [EditableBlock]) {
    let groups: BTreeSet<_> = blocks
        .iter()
        .filter_map(|b| column(&b.block).map(|c| c.0))
        .collect();
    for group in groups {
        let indexes: Vec<_> = blocks
            .iter()
            .filter_map(|b| column(&b.block))
            .filter(|(g, _)| *g == group)
            .map(|(_, i)| i)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        for b in blocks.iter_mut() {
            if let Some((g, i)) = column(&b.block).filter(|(g, _)| *g == group) {
                let mapped =
                    (indexes.len() > 1).then(|| (g.as_str(), indexes.binary_search(&i).unwrap()));
                set_column(&mut b.block, mapped);
            }
        }
    }
}
pub fn drop_target(point: [f32; 2], zones: &[DropZone], source: &str) -> Option<Target> {
    let zone = zones
        .iter()
        .filter(|z| z.id != source && z.rect[2] > 0. && z.rect[3] > 0.)
        .filter(|z| {
            point[0] >= z.rect[0] - 25.
                && point[0] <= z.rect[0] + z.rect[2] + 25.
                && point[1] >= z.rect[1] - 3.
                && point[1] <= z.rect[1] + z.rect[3] + 3.
        })
        .min_by(|a, b| a.rect[3].total_cmp(&b.rect[3]))?;
    let [x, y, w, h] = zone.rect;
    let side_width = (w * 0.14).min(48.);
    let inside_side_band = point[1] > y + h * 0.25 && point[1] < y + h * 0.75;
    let side = if zone.id == "@end" {
        Side::After
    } else if inside_side_band && point[0] < x + side_width {
        Side::Left
    } else if inside_side_band && point[0] > x + w - side_width {
        Side::Right
    } else if point[1] < y + h / 2. {
        Side::Before
    } else {
        Side::After
    };
    Some(Target {
        id: zone.id.clone(),
        side,
    })
}
impl State {
    pub fn resize_editor_columns(
        &mut self,
        group: &str,
        left: usize,
        right: usize,
        widths: [f32; 2],
        drag: &Drag,
        peek: bool,
    ) {
        if drag.cancelled() {
            self.editor_column_resize = None;
            return;
        }
        if drag.phase == Phase::Down {
            let (page_id, blocks) = if peek {
                let Some(p) = self.record_peek.as_ref() else {
                    return;
                };
                (&p.page.id, &p.blocks)
            } else {
                let Some(t) = self.tab() else { return };
                (&t.page.id, &t.blocks)
            };
            let get = |index| {
                blocks
                    .iter()
                    .find(|b| column(&b.block).is_some_and(|(g, i)| g == group && i == index))
                    .map(|b| column_weight(&b.block))
            };
            let (Some(l), Some(r)) = (get(left), get(right)) else {
                return;
            };
            if widths.iter().any(|w| !w.is_finite() || *w <= 0.) {
                return;
            }
            self.editor_column_resize = Some(ColumnResize {
                page_id: page_id.clone(),
                group: group.into(),
                left,
                right,
                left_weight: l,
                right_weight: r,
                start_left: l,
                start_right: r,
                total_width: widths[0] + widths[1],
                start_width: widths[0],
                peek,
            });
            return;
        }
        let Some(resize) = self.editor_column_resize.as_mut() else {
            return;
        };
        if resize.group != group
            || resize.left != left
            || resize.right != right
            || resize.peek != peek
        {
            return;
        }
        let min = (resize.total_width * 0.2).min(80.);
        let next = (resize.start_width + drag.dx).clamp(min, resize.total_width - min);
        let total = resize.start_left + resize.start_right;
        resize.left_weight = total * next / resize.total_width;
        resize.right_weight = total - resize.left_weight;
        if drag.phase == Phase::Up {
            let resize = self.editor_column_resize.take().unwrap();
            if drag.dx.abs() < 1. {
                return;
            }
            self.save_column_widths(&resize);
        }
    }
    pub fn step_editor_columns(
        &mut self,
        group: &str,
        left: usize,
        right: usize,
        widths: [f32; 2],
        step: i32,
        peek: bool,
    ) {
        let (page_id, blocks) = if peek {
            let Some(p) = self.record_peek.as_ref() else {
                return;
            };
            (&p.page.id, &p.blocks)
        } else {
            let Some(t) = self.tab() else { return };
            (&t.page.id, &t.blocks)
        };
        let get = |index| {
            blocks
                .iter()
                .find(|b| column(&b.block).is_some_and(|(g, i)| g == group && i == index))
                .map(|b| column_weight(&b.block))
        };
        let (Some(l), Some(r)) = (get(left), get(right)) else {
            return;
        };
        if widths.iter().any(|w| !w.is_finite() || *w <= 0.) {
            return;
        }
        let total_width = widths[0] + widths[1];
        let min = (total_width * 0.2).min(80.);
        let next = (widths[0] + step as f32 * 10.).clamp(min, total_width - min);
        if (next - widths[0]).abs() < 0.5 {
            return;
        }
        let left_weight = (l + r) * next / total_width;
        let resize = ColumnResize {
            page_id: page_id.clone(),
            group: group.into(),
            left,
            right,
            left_weight,
            right_weight: l + r - left_weight,
            start_left: l,
            start_right: r,
            total_width,
            start_width: widths[0],
            peek,
        };
        self.save_column_widths(&resize);
    }
    fn save_column_widths(&mut self, resize: &ColumnResize) {
        let blocks = if resize.peek {
            let Some(p) = self
                .record_peek
                .as_mut()
                .filter(|p| p.page.id == resize.page_id)
            else {
                return;
            };
            &mut p.blocks
        } else {
            let Some(t) = self
                .tabs
                .get_mut(self.active)
                .filter(|t| t.page.id == resize.page_id)
            else {
                return;
            };
            &mut t.blocks
        };
        let mut changed = false;
        for block in blocks {
            if let Some((_, i)) = column(&block.block).filter(|(g, _)| *g == resize.group) {
                let value = if i == resize.left {
                    Some(resize.left_weight)
                } else if i == resize.right {
                    Some(resize.right_weight)
                } else {
                    None
                };
                if let Some(value) = value {
                    block.block.config["moss_columns"]["weight"] = json!(value);
                    changed = true;
                }
            }
        }
        if changed {
            if resize.peek {
                self.save_peek_blocks()
            } else {
                self.save_blocks()
            }
        }
    }
    pub fn drag_editor_block(&mut self, id: &str, drag: &Drag, zones: &[DropZone], peek: bool) {
        if drag.phase == Phase::Down {
            self.editor_drag_cancelled = false;
            self.editor_drag_suppress_press = false;
            return;
        }
        if drag.cancelled() || self.editor_drag_cancelled {
            self.editor_drag = None;
            if drag.phase == Phase::Up {
                self.editor_drag_cancelled = false;
            }
            return;
        }
        if drag.dx.abs() < 4. && drag.dy.abs() < 4. && self.editor_drag.is_none() {
            return;
        }
        let (page_id, blocks) = if peek {
            let Some(p) = self.record_peek.as_ref() else {
                return;
            };
            (&p.page.id, &p.blocks)
        } else {
            let Some(t) = self.tab() else { return };
            (&t.page.id, &t.blocks)
        };
        if !blocks.iter().any(|b| b.block.id == id) {
            return;
        }
        let page_id = page_id.clone();
        let rect = self
            .editor_drag
            .as_ref()
            .filter(|d| d.block_id == id && d.page_id == page_id)
            .map(|d| d.rect)
            .or_else(|| zones.iter().find(|z| z.id == id).map(|z| z.rect))
            .unwrap_or([drag.window_point()[0], drag.window_point()[1], 260., 30.]);
        self.editor_drag_suppress_press = true;
        let point = drag.window_point();
        let target = Some(drop_target(point, zones, id).unwrap_or_else(|| Target {
            id: id.into(),
            side: Side::Before,
        }));
        self.editor_drag = Some(BlockDrag {
            page_id,
            block_id: id.into(),
            target,
            delta: [drag.dx, drag.dy],
            rect,
            peek,
        });
        if drag.phase == Phase::Up {
            let active = self.editor_drag.take().unwrap();
            if let Some(target) = active.target {
                if active.peek {
                    self.move_peek_block(&active.block_id, &target);
                } else {
                    self.move_editor_block(&active.block_id, &target);
                }
            }
        }
    }
    pub fn move_editor_block(&mut self, source: &str, target: &Target) -> bool {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return false;
        };
        if !rearrange(&mut tab.blocks, source, target) {
            return false;
        }
        self.save_blocks();
        true
    }
    pub fn move_peek_block(&mut self, source: &str, target: &Target) -> bool {
        let Some(peek) = self.record_peek.as_mut() else {
            return false;
        };
        if !rearrange(&mut peek.blocks, source, target) {
            return false;
        }
        self.save_peek_blocks();
        true
    }
}
fn rearrange(blocks: &mut Vec<EditableBlock>, source: &str, target: &Target) -> bool {
    let Some(from) = blocks.iter().position(|b| b.block.id == source) else {
        return false;
    };
    if target.id == "@end" {
        let mut item = blocks.remove(from);
        if !set_column(&mut item.block, None) {
            blocks.insert(from, item);
            return false;
        }
        blocks.push(item);
        normalize(blocks);
        return true;
    }
    let Some(to) = blocks.iter().position(|b| b.block.id == target.id) else {
        return false;
    };
    if from == to {
        return false;
    }
    if [from, to]
        .iter()
        .any(|&i| !blocks[i].block.config.is_null() && !blocks[i].block.config.is_object())
    {
        return false;
    }
    let mut item = blocks.remove(from);
    set_column(&mut item.block, None);
    let pos = blocks.iter().position(|b| b.block.id == target.id).unwrap();
    let target_column = column(&blocks[pos].block);
    match target.side {
        Side::Before | Side::After => {
            if target_column.is_some() {
                copy_column(&blocks[pos].block, &mut item.block);
            }
            blocks.insert(pos + usize::from(target.side == Side::After), item);
        }
        Side::Left | Side::Right => {
            let (group, index) = target_column.unwrap_or_else(|| {
                let group = moss_core::new_id();
                set_column(&mut blocks[pos].block, Some((&group, 0)));
                (group, 0)
            });
            let at = index + usize::from(target.side == Side::Right);
            for block in blocks.iter_mut() {
                if let Some((g, i)) = column(&block.block).filter(|(g, i)| *g == group && *i >= at)
                {
                    set_column(&mut block.block, Some((&g, i + 1)));
                }
            }
            set_column(&mut item.block, Some((&group, at)));
            let insert = blocks
                .iter()
                .position(|b| column(&b.block).is_some_and(|(g, i)| g == group && i > at))
                .unwrap_or_else(|| {
                    blocks
                        .iter()
                        .rposition(|b| column(&b.block).is_some_and(|(g, _)| g == group))
                        .unwrap()
                        + 1
                });
            blocks.insert(insert, item);
        }
    }
    normalize(blocks);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::new_id;
    fn fixture() -> (State, std::path::PathBuf, Vec<String>) {
        let path = std::env::temp_dir().join(format!("moss-block-drag-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        state.tabs[state.active].blocks[0]
            .edit
            .replace_text("first");
        state.append_editor_block();
        state.tabs[state.active].blocks[1]
            .edit
            .replace_text("second");
        state.append_editor_block();
        state.tabs[state.active].blocks[2]
            .edit
            .replace_text("third");
        state.save_blocks();
        let ids = state
            .tab()
            .unwrap()
            .blocks
            .iter()
            .map(|b| b.block.id.clone())
            .collect();
        (state, path, ids)
    }
    #[test]
    fn reorder_keeps_block_identity_and_persists() {
        let (mut s, path, ids) = fixture();
        let page = s.tab().unwrap().page.id.clone();
        assert!(s.move_editor_block(
            &ids[0],
            &Target {
                id: ids[2].clone(),
                side: Side::After
            }
        ));
        assert_eq!(
            s.store
                .get_blocks(&page)
                .unwrap()
                .iter()
                .map(|b| b.id.clone())
                .collect::<Vec<_>>(),
            vec![ids[1].clone(), ids[2].clone(), ids[0].clone()]
        );
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn columns_persist_extend_and_collapse_when_a_column_is_moved_out() {
        let (mut s, path, ids) = fixture();
        let page = s.tab().unwrap().page.id.clone();
        assert!(s.move_editor_block(
            &ids[1],
            &Target {
                id: ids[0].clone(),
                side: Side::Right
            }
        ));
        let saved = s.store.get_blocks(&page).unwrap();
        let (group, left) = column(&saved[0]).unwrap();
        assert_eq!(left, 0);
        assert_eq!(column(&saved[1]), Some((group, 1)));
        s.add_block(0);
        assert_eq!(
            column(&s.tab().unwrap().blocks[1].block),
            column(&s.tab().unwrap().blocks[0].block)
        );
        assert!(s.move_editor_block(
            &ids[1],
            &Target {
                id: ids[2].clone(),
                side: Side::After
            }
        ));
        assert!(
            s.store
                .get_blocks(&page)
                .unwrap()
                .iter()
                .all(|b| column(b).is_none())
        );
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn moving_the_last_column_to_the_end_removes_column_layout() {
        let (mut s, path, ids) = fixture();
        assert!(s.move_editor_block(
            &ids[1],
            &Target {
                id: ids[0].clone(),
                side: Side::Right
            }
        ));
        assert!(s.move_editor_block(
            &ids[1],
            &Target {
                id: "@end".into(),
                side: Side::After
            }
        ));
        assert!(
            s.tab()
                .unwrap()
                .blocks
                .iter()
                .all(|block| column(&block.block).is_none())
        );
        assert_eq!(s.tab().unwrap().blocks.last().unwrap().block.id, ids[1]);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn can_insert_a_third_column_and_move_a_block_between_columns() {
        let (mut s, path, ids) = fixture();
        assert!(s.move_editor_block(
            &ids[1],
            &Target {
                id: ids[0].clone(),
                side: Side::Right
            }
        ));
        assert!(s.move_editor_block(
            &ids[2],
            &Target {
                id: ids[1].clone(),
                side: Side::Left
            }
        ));
        let blocks = &s.tab().unwrap().blocks;
        assert_eq!(
            blocks
                .iter()
                .map(|b| b.block.id.as_str())
                .collect::<Vec<_>>(),
            vec![ids[0].as_str(), ids[2].as_str(), ids[1].as_str()]
        );
        assert_eq!(
            blocks
                .iter()
                .map(|b| column(&b.block).unwrap().1)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(s.move_editor_block(
            &ids[2],
            &Target {
                id: ids[0].clone(),
                side: Side::After
            }
        ));
        let blocks = &s.tab().unwrap().blocks;
        assert_eq!(column(&blocks[0].block), column(&blocks[1].block));
        assert_eq!(column(&blocks[2].block).unwrap().1, 1);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn placing_a_block_in_a_resized_column_preserves_its_width() {
        let (mut s, path, ids) = fixture();
        assert!(s.move_editor_block(
            &ids[1],
            &Target {
                id: ids[0].clone(),
                side: Side::Right
            }
        ));
        s.tabs[s.active].blocks[0].block.config["moss_columns"]["weight"] = json!(1.5);
        assert!(s.move_editor_block(
            &ids[2],
            &Target {
                id: ids[0].clone(),
                side: Side::Before
            }
        ));
        assert_eq!(column_weight(&s.tab().unwrap().blocks[0].block), 1.5);
        assert_eq!(column_weight(&s.tab().unwrap().blocks[1].block), 1.5);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn guides_use_block_edges_for_columns_and_middle_for_order() {
        let zones = vec![DropZone {
            id: "b".into(),
            rect: [100., 100., 200., 40.],
        }];
        assert_eq!(
            drop_target([108., 120.], &zones, "a").unwrap().side,
            Side::Left
        );
        assert_eq!(
            drop_target([292., 120.], &zones, "a").unwrap().side,
            Side::Right
        );
        assert_eq!(
            drop_target([200., 110.], &zones, "a").unwrap().side,
            Side::Before
        );
        assert_eq!(
            drop_target([200., 130.], &zones, "a").unwrap().side,
            Side::After
        );
        for (point, side) in [
            ([90., 120.], Side::Left),
            ([310., 120.], Side::Right),
            ([90., 110.], Side::Before),
            ([90., 130.], Side::After),
            ([310., 110.], Side::Before),
            ([310., 130.], Side::After),
            ([103., 102.], Side::Before),
            ([103., 138.], Side::After),
            ([130., 110.], Side::Before),
            ([130., 130.], Side::After),
        ] {
            assert_eq!(
                drop_target(point, &zones, "a").unwrap().side,
                side,
                "{point:?}"
            );
        }
        assert!(drop_target([900., 900.], &zones, "a").is_none());
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use rune::{Env, Runtime};

    #[test]
    fn dragging_in_the_handle_gutter_reorders_without_creating_columns() {
        for peek in [false, true] {
            let path =
                std::env::temp_dir().join(format!("moss-drag-gutter-{}", moss_core::new_id()));
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
                s.save_peek_blocks();
            } else {
                s.tabs[s.active].blocks[0].edit.replace_text("First");
                s.append_editor_block();
                s.tabs[s.active].blocks[1].edit.replace_text("Second");
                s.save_blocks();
            }
            let (page, blocks) = if peek {
                let p = s.record_peek.as_ref().unwrap();
                (p.page.id.clone(), &p.blocks)
            } else {
                let p = s.tab().unwrap();
                (p.page.id.clone(), &p.blocks)
            };
            let ids: Vec<_> = blocks.iter().map(|b| b.block.id.clone()).collect();
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
            rt.frame(0.);
            rt.frame(0.02);
            let handle = rt
                .presented(if peek {
                    "peek-block-menu-1"
                } else {
                    "block-menu-1"
                })
                .unwrap()
                .0;
            let target = rt
                .presented(&format!(
                    "{}-{}",
                    if peek { "peek-drag-block" } else { "block" },
                    ids[0]
                ))
                .unwrap()
                .0;
            let from = [handle[0] + handle[2] / 2., handle[1] + handle[3] / 2.];
            let to = [from[0], target[1] + 2.];
            rt.pointer_move(from[0], from[1]);
            rt.frame(0.02);
            rt.pointer_down(from[0], from[1]);
            rt.pointer_move(to[0], to[1]);
            rt.frame(0.02);
            let destination = rt
                .state
                .editor_drag
                .as_ref()
                .unwrap()
                .target
                .as_ref()
                .unwrap();
            assert_eq!((&destination.id, destination.side), (&ids[0], Side::Before));
            assert!(rt.presented("editor-block-drop-guide").is_some());
            rt.pointer_up(to[0], to[1]);
            rt.frame(0.02);
            let saved = rt.state.store.get_blocks(&page).unwrap();
            assert_eq!(
                saved.iter().map(|b| &b.id).collect::<Vec<_>>(),
                vec![&ids[1], &ids[0]]
            );
            assert!(saved.iter().all(|b| column(b).is_none()));
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn picked_up_block_leaves_a_return_guide_and_can_be_put_back() {
        for peek in [false, true] {
            let path =
                std::env::temp_dir().join(format!("moss-drag-return-{}", moss_core::new_id()));
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
                s.save_peek_blocks();
            } else {
                s.tabs[s.active].blocks[0].edit.replace_text("First");
                s.append_editor_block();
                s.tabs[s.active].blocks[1].edit.replace_text("Second");
                s.save_blocks();
            }
            let blocks = if peek {
                &s.record_peek.as_ref().unwrap().blocks
            } else {
                &s.tab().unwrap().blocks
            };
            let ids: Vec<_> = blocks.iter().map(|b| b.block.id.clone()).collect();
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
            rt.frame(0.);
            rt.frame(0.02);
            let handle = rt
                .presented(if peek {
                    "peek-block-menu-0"
                } else {
                    "block-menu-0"
                })
                .unwrap()
                .0;
            let prefix = if peek { "peek-editor" } else { "editor" };
            let next_key = format!("{prefix}-block-slot-{}", ids[1]);
            let old_next = rt.presented(&next_key).unwrap().0;
            let start = [handle[0] + handle[2] / 2., handle[1] + handle[3] / 2.];
            let outside = [start[0] - 100., start[1] - 100.];
            let writes = rt.state.store.local_change_count();
            rt.pointer_move(start[0], start[1]);
            rt.frame(0.02);
            rt.pointer_down(start[0], start[1]);
            rt.pointer_move(outside[0], outside[1]);
            rt.frame(0.02);
            let target = rt
                .state
                .editor_drag
                .as_ref()
                .unwrap()
                .target
                .as_ref()
                .unwrap();
            assert_eq!(target.id, ids[0]);
            let slot = rt
                .presented(&format!("{prefix}-block-slot-{}", ids[0]))
                .unwrap()
                .0;
            let guide = rt
                .presented("editor-block-drop-guide")
                .expect("return guide")
                .0;
            assert!(slot[3] < 1., "picked-up block still takes space: {slot:?}");
            assert!((guide[1] - slot[1]).abs() < 4.);
            assert!(rt.presented(&next_key).unwrap().0[1] < old_next[1] - 10.);
            rt.pointer_up(outside[0], outside[1]);
            rt.frame(0.02);
            assert!(rt.state.editor_drag.is_none());
            assert_eq!(rt.state.store.local_change_count(), writes);
            let blocks = if peek {
                &rt.state.record_peek.as_ref().unwrap().blocks
            } else {
                &rt.state.tab().unwrap().blocks
            };
            assert_eq!(
                blocks.iter().map(|b| &b.block.id).collect::<Vec<_>>(),
                ids.iter().collect::<Vec<_>>()
            );
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }

    fn assert_drop_guides(rt: &mut Runtime<State>, id: &str, rect: [f32; 4], peek: bool) {
        let [x, y, w, h] = rect;
        let slot_key = format!(
            "{}-block-slot-{id}",
            if peek { "peek-editor" } else { "editor" }
        );
        for (side, point) in [
            (Side::Left, [x - 10., y + h / 2.]),
            (Side::Left, [x + 3., y + h / 2.]),
            (Side::Before, [x + w / 2., y + 1.]),
            (Side::After, [x + w / 2., y + h - 1.]),
            (Side::Right, [x + w - 3., y + h / 2.]),
            (Side::Right, [x + w + 10., y + h / 2.]),
        ] {
            rt.pointer_move(point[0], point[1]);
            rt.frame(0.02);
            let target = rt
                .state
                .editor_drag
                .as_ref()
                .unwrap()
                .target
                .as_ref()
                .unwrap();
            assert_eq!((&target.id, target.side), (&id.to_string(), side));
            let guide = rt.presented("editor-block-drop-guide").expect("guide").0;
            let slot = rt.presented(&slot_key).expect("target slot").0;
            match side {
                Side::Left | Side::Right => {
                    assert!(
                        guide[2] >= 2. && (guide[3] - slot[3]).abs() < 1.,
                        "{side:?}: {guide:?} {slot:?}"
                    );
                    let edge = if side == Side::Left {
                        slot[0]
                    } else {
                        slot[0] + slot[2]
                    };
                    assert!(
                        (guide[0] + guide[2] / 2. - edge).abs() < 3.,
                        "{side:?}: {guide:?} {slot:?}"
                    );
                }
                Side::Before | Side::After => {
                    assert!(
                        guide[3] >= 2. && (guide[2] - slot[2]).abs() < 1.,
                        "{side:?}: {guide:?} {slot:?}"
                    );
                    let edge = if side == Side::Before {
                        slot[1]
                    } else {
                        slot[1] + slot[3]
                    };
                    assert!(
                        (guide[1] + guide[3] / 2. - edge).abs() < 3.,
                        "{side:?}: {guide:?} {slot:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn focused_separator_resizes_with_keyboard_and_persists_in_page_and_peek() {
        for peek in [false, true] {
            let path =
                std::env::temp_dir().join(format!("moss-keyboard-columns-{}", moss_core::new_id()));
            let mut s = State::open(path.clone(), None).unwrap();
            s.new_page(peek, None);
            let (page, ids) = if peek {
                s.create_peek(Default::default());
                s.record_peek.as_mut().unwrap().blocks[0]
                    .edit
                    .replace_text("First");
                s.add_peek_block(0);
                s.record_peek.as_mut().unwrap().blocks[1]
                    .edit
                    .replace_text("Second");
                s.save_peek_blocks();
                let p = s.record_peek.as_ref().unwrap();
                (
                    p.page.id.clone(),
                    p.blocks
                        .iter()
                        .map(|b| b.block.id.clone())
                        .collect::<Vec<_>>(),
                )
            } else {
                s.tabs[s.active].blocks[0].edit.replace_text("First");
                s.append_editor_block();
                s.tabs[s.active].blocks[1].edit.replace_text("Second");
                s.save_blocks();
                let t = s.tab().unwrap();
                (
                    t.page.id.clone(),
                    t.blocks
                        .iter()
                        .map(|b| b.block.id.clone())
                        .collect::<Vec<_>>(),
                )
            };
            let target = Target {
                id: ids[0].clone(),
                side: Side::Right,
            };
            assert!(if peek {
                s.move_peek_block(&ids[1], &target)
            } else {
                s.move_editor_block(&ids[1], &target)
            });
            let group = column(&s.store.get_blocks(&page).unwrap()[0]).unwrap().0;
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
            rt.frame(0.);
            rt.frame(0.02);
            let prefix = if peek { "peek-editor" } else { "editor" };
            let key = format!("{prefix}-column-resize-{group}-0");
            let info = rt
                .elements()
                .into_iter()
                .find(|e| e.key.as_deref() == Some(&key))
                .unwrap();
            assert_eq!(info.role, "separator");
            assert!(info.focusable);
            assert_eq!(
                info.label.as_deref(),
                Some("Resize columns. Use Left and Right arrows.")
            );
            let col_key = format!("{prefix}-column-{group}-0");
            let before = rt.presented(&col_key).unwrap().0[2];
            let point = rt.find(&key).unwrap();
            rt.pointer_move(point.0, point.1);
            rt.frame(0.02);
            rt.pointer_down(point.0, point.1);
            rt.pointer_up(point.0, point.1);
            rt.frame(0.02);
            assert!(
                rt.elements()
                    .into_iter()
                    .find(|e| e.key.as_deref() == Some(&key))
                    .unwrap()
                    .focused
            );
            rt.key(rune::KeyEvent {
                key: rune::Key::Right,
                mods: rune::Mods::default(),
            });
            rt.frame(0.02);
            let after = rt.presented(&col_key).unwrap().0[2];
            assert!(after > before + 8., "{before} {after}");
            let saved = rt.state.store.get_blocks(&page).unwrap();
            assert!(column_weight(&saved[0]) > column_weight(&saved[1]));
            rt.key(rune::KeyEvent {
                key: rune::Key::Left,
                mods: rune::Mods::default(),
            });
            rt.frame(0.02);
            let restored = rt.presented(&col_key).unwrap().0[2];
            assert!((restored - before).abs() < 1., "{before} {restored}");
            let saved = rt.state.store.get_blocks(&page).unwrap();
            assert!((column_weight(&saved[0]) - column_weight(&saved[1])).abs() < 0.01);
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
    #[test]
    fn dragging_the_handle_shows_a_guide_and_creates_columns() {
        let path = std::env::temp_dir().join(format!("moss-drag-ui-{}", moss_core::new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(false, None);
        s.tabs[s.active].blocks[0].edit.replace_text("First");
        s.append_editor_block();
        s.tabs[s.active].blocks[1].edit.replace_text("Second");
        s.save_blocks();
        let ids: Vec<_> = s
            .tab()
            .unwrap()
            .blocks
            .iter()
            .map(|b| b.block.id.clone())
            .collect();
        let mut rt = Runtime::with_text(
            s,
            crate::ui::view,
            (1000., 800.),
            Env {
                system_dark: false,
                scale: 1.,
            },
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.on_key = Some(crate::key);
        rt.frame(0.);
        rt.frame(0.02);
        let handle = rt.presented("block-menu-1").expect("handle").0;
        let target = rt
            .presented(&format!("block-{}", ids[0]))
            .expect("target")
            .0;
        let from = [handle[0] + handle[2] / 2., handle[1] + handle[3] / 2.];
        let dest = [target[0] + target[2] - 3., target[1] + target[3] / 2.];
        rt.pointer_move(from[0], from[1]);
        rt.frame(0.02);
        rt.pointer_down(from[0], from[1]);
        rt.pointer_move(from[0], from[1] + 8.);
        rt.frame(0.02);
        rt.pointer_move(dest[0], dest[1]);
        rt.frame(0.02);
        assert!(rt.state.editor_drag.is_some(), "drag did not start");
        assert_drop_guides(&mut rt, &ids[0], target, false);
        rt.pointer_up(dest[0], dest[1]);
        rt.frame(0.02);
        let blocks = &rt.state.tab().unwrap().blocks;
        assert!(column(&blocks[0].block).is_some());
        assert!(column(&blocks[1].block).is_some());
        let left = rt.presented(&format!("block-{}", ids[0])).unwrap().0;
        let right = rt.presented(&format!("block-{}", ids[1])).unwrap().0;
        assert!(right[0] > left[0] + left[2] * 0.8, "{left:?} {right:?}");
        assert!(
            (left[2] - right[2]).abs() < 1.,
            "column widths differ: {left:?} {right:?}"
        );
        assert!(rt.state.block_commands.is_none());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn escape_cancels_a_drag_without_saving_and_the_preview_follows_the_pointer() {
        let path = std::env::temp_dir().join(format!("moss-drag-cancel-{}", moss_core::new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(false, None);
        s.tabs[s.active].blocks[0].edit.replace_text("First");
        s.append_editor_block();
        s.tabs[s.active].blocks[1].edit.replace_text("Second");
        s.save_blocks();
        let id = s.tab().unwrap().blocks[0].block.id.clone();
        let mut rt = Runtime::with_text(
            s,
            crate::ui::view,
            (1000., 800.),
            Env {
                system_dark: false,
                scale: 1.,
            },
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.on_key = Some(crate::key);
        rt.frame(0.);
        rt.frame(0.02);
        let handle = rt.presented("block-menu-0").unwrap().0;
        let source = rt.presented(&format!("block-{id}")).unwrap().0;
        let start = [handle[0] + handle[2] / 2., handle[1] + handle[3] / 2.];
        let point = [start[0] + 45., start[1] + 50.];
        let writes = rt.state.store.local_change_count();
        rt.pointer_move(start[0], start[1]);
        rt.frame(0.02);
        rt.pointer_down(start[0], start[1]);
        rt.pointer_move(point[0], point[1]);
        rt.frame(0.02);
        let preview = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(&format!("editor-drag-preview-{id}")))
            .expect("drag preview");
        assert!((preview.opacity - 0.7).abs() < 0.01);
        assert!(preview.inert);
        assert!(rt.presented(&format!("block-input-{id}")).is_some());
        assert!(
            (preview.rect[0] - (source[0] + 45.)).abs() < 1.,
            "{:?} {source:?}",
            preview.rect
        );
        assert!(
            (preview.rect[1] - (source[1] + 50.)).abs() < 1.,
            "{:?} {source:?}",
            preview.rect
        );
        rt.key(rune::KeyEvent {
            key: rune::Key::Escape,
            mods: rune::Mods::default(),
        });
        rt.frame(0.02);
        rt.pointer_up(point[0], point[1]);
        rt.frame(0.02);
        assert!(rt.state.editor_drag.is_none());
        assert_eq!(rt.state.store.local_change_count(), writes);
        assert_eq!(rt.state.tab().unwrap().blocks[0].block.id, id);
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn column_divider_tracks_pointer_saves_on_release_and_reopens() {
        for scale in [1., 2.] {
            let path =
                std::env::temp_dir().join(format!("moss-column-resize-{}", moss_core::new_id()));
            let mut s = State::open(path.clone(), None).unwrap();
            s.new_page(false, None);
            s.tabs[s.active].blocks[0].edit.replace_text("Left");
            s.append_editor_block();
            s.tabs[s.active].blocks[1].edit.replace_text("Right");
            s.save_blocks();
            let ids: Vec<_> = s
                .tab()
                .unwrap()
                .blocks
                .iter()
                .map(|b| b.block.id.clone())
                .collect();
            let page = s.tab().unwrap().page.id.clone();
            let workspace = s.workspace.id.clone();
            assert!(s.move_editor_block(
                &ids[1],
                &Target {
                    id: ids[0].clone(),
                    side: Side::Right
                }
            ));
            let group = column(&s.tab().unwrap().blocks[0].block).unwrap().0;
            let mut rt = Runtime::with_text(
                s,
                crate::ui::view,
                (1100., 800.),
                Env {
                    system_dark: false,
                    scale,
                },
                rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
            );
            rt.on_key = Some(crate::key);
            rt.frame(0.);
            rt.frame(0.02);
            let key = format!("editor-column-resize-{group}-0");
            let handle = rt.presented(&key).expect("column divider").0;
            let left = rt.presented(&format!("editor-column-{group}-0")).unwrap().0;
            let right = rt.presented(&format!("editor-column-{group}-1")).unwrap().0;
            let p = [handle[0] + handle[2] / 2., handle[1] + handle[3] / 2.];
            let writes = rt.state.store.local_change_count();
            rt.pointer_move(p[0], p[1]);
            rt.frame(0.02);
            rt.pointer_down(p[0], p[1]);
            rt.pointer_move(p[0] + 50., p[1]);
            rt.frame(0.02);
            assert!(
                rt.state.editor_column_resize.is_some(),
                "divider did not start at {p:?}, handle {handle:?}, left {left:?}, right {right:?}"
            );
            assert_eq!(
                rt.state.store.local_change_count(),
                writes,
                "do not save while dragging"
            );
            let wider = rt.presented(&format!("editor-column-{group}-0")).unwrap().0;
            let narrower = rt.presented(&format!("editor-column-{group}-1")).unwrap().0;
            assert!(wider[2] > left[2] + 30., "{left:?} {wider:?}");
            assert!(narrower[2] < right[2] - 30., "{right:?} {narrower:?}");
            rt.pointer_up(p[0] + 50., p[1]);
            rt.frame(0.02);
            assert!(rt.state.store.local_change_count() > writes);
            let saved = rt.state.store.get_blocks(&page).unwrap();
            let a = column_weight(&saved[0]);
            let b = column_weight(&saved[1]);
            assert!(a > b);
            drop(rt);
            let mut reopened = State::open(path.clone(), Some(&workspace)).unwrap();
            reopened.open_page(&page, false);
            assert_eq!(column_weight(&reopened.tab().unwrap().blocks[0].block), a);
            assert_eq!(column_weight(&reopened.tab().unwrap().blocks[1].block), b);
            drop(reopened);
            std::fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn record_peek_drag_creates_persistent_columns_and_split_stays_in_column() {
        let path = std::env::temp_dir().join(format!("moss-peek-drag-{}", moss_core::new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        s.create_peek(Default::default());
        s.record_peek.as_mut().unwrap().blocks[0]
            .edit
            .replace_text("First");
        s.add_peek_block(0);
        s.record_peek.as_mut().unwrap().blocks[1]
            .edit
            .replace_text("Second");
        s.save_peek_blocks();
        let ids: Vec<_> = s
            .record_peek
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .map(|b| b.block.id.clone())
            .collect();
        let page = s.record_peek.as_ref().unwrap().page.id.clone();
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
        rt.frame(0.);
        rt.frame(0.02);
        let handle = rt.presented("peek-block-menu-1").unwrap().0;
        let target = rt
            .presented(&format!("peek-drag-block-{}", ids[0]))
            .unwrap()
            .0;
        let from = [handle[0] + handle[2] / 2., handle[1] + handle[3] / 2.];
        let to = [target[0] + target[2] - 3., target[1] + target[3] / 2.];
        rt.pointer_move(from[0], from[1]);
        rt.frame(0.02);
        rt.pointer_down(from[0], from[1]);
        rt.pointer_move(from[0], from[1] + 8.);
        rt.frame(0.02);
        rt.pointer_move(to[0], to[1]);
        rt.frame(0.02);
        assert_drop_guides(&mut rt, &ids[0], target, true);
        rt.pointer_up(to[0], to[1]);
        rt.frame(0.02);
        let saved = rt.state.store.get_blocks(&page).unwrap();
        assert_eq!(column(&saved[0]).unwrap().1, 0);
        assert_eq!(column(&saved[1]).unwrap().1, 1);
        let group = column(&saved[0]).unwrap().0;
        assert!(
            rt.presented(&format!("peek-editor-column-group-{group}"))
                .is_some()
        );
        rt.state.add_peek_block(0);
        let peek = rt.state.record_peek.as_ref().unwrap();
        assert_eq!(column(&peek.blocks[0].block), column(&peek.blocks[1].block));
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
