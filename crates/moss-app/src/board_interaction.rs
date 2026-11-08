use crate::state::State;
use moss_core::{Property, Record, ViewKind};
use rune::{Drag, Phase, ScrollPos, TextEdit, Tx};
use serde_json::Value;
use std::{cell::Cell, collections::BTreeMap, rc::Rc, time::Duration};

#[derive(Clone)]
pub struct DropZone {
    pub key: String,
    pub group: Value,
    pub subgroup: Value,
    pub lane: String,
    pub extend_bottom: bool,
    pub cards: Vec<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Destination {
    pub group: Value,
    pub subgroup: Value,
    pub before: Option<String>,
    pub key: String,
}
pub struct CardDrag {
    pub record: Record,
    pub property: Property,
    pub subgroup: Option<Property>,
    pub context: String,
    pub moved: bool,
    pub origin: [f32; 2],
    pub delta: [f32; 2],
    pub point: [f32; 2],
    pub destination: Option<Destination>,
}
#[derive(Clone)]
pub enum MenuTarget {
    Card(Record),
    Group {
        property: Property,
        value: Value,
        subgroup: bool,
    },
}
#[derive(Clone, Copy, PartialEq)]
pub enum MenuPane {
    Main,
    Move,
    Subgroup,
    Name,
}
#[derive(Clone)]
pub struct BoardMenu {
    pub target: MenuTarget,
    pub pane: MenuPane,
    pub anchor: String,
    pub point: Option<[f32; 2]>,
}
pub struct BoardUi {
    pub drag: Option<CardDrag>,
    pub menu: Option<BoardMenu>,
    pub name: TextEdit,
    pub error: Option<String>,
    pub viewport: [f32; 4],
    pub scroll: Rc<Cell<ScrollPos>>,
    pub covers: BTreeMap<String, (String, [f32; 4])>,
    pub card_heights: BTreeMap<String, f32>,
    pub drop_zones: BTreeMap<String, [f32; 4]>,
    pub lane_rects: BTreeMap<String, [f32; 4]>,
    pub card_rects: BTreeMap<String, [f32; 4]>,
    pub height_version: u64,
    pub group_drag: Option<(String, bool)>,
    pub group_drop: Option<String>,
    pub group_context: Option<String>,
    pub group_expected: Option<Value>,
    pub group_point: Option<[f32; 2]>,
    pub suppress_press: bool,
    context: Option<String>,
    scroll_at: f64,
    last_scroll: [f32; 2],
    last_height_version: u64,
    layout_config: Option<Value>,
    geometry_groups: Vec<Value>,
    geometry_subgroups: Vec<Value>,
}
impl Default for BoardUi {
    fn default() -> Self {
        Self {
            drag: None,
            menu: None,
            name: TextEdit::new(""),
            error: None,
            viewport: [0.; 4],
            scroll: Rc::new(Cell::new(ScrollPos::default())),
            covers: BTreeMap::new(),
            card_heights: BTreeMap::new(),
            drop_zones: BTreeMap::new(),
            lane_rects: BTreeMap::new(),
            card_rects: BTreeMap::new(),
            height_version: 0,
            group_drag: None,
            group_drop: None,
            group_context: None,
            group_expected: None,
            group_point: None,
            suppress_press: false,
            context: None,
            scroll_at: 0.,
            last_scroll: [0., 0.],
            last_height_version: 0,
            layout_config: None,
            geometry_groups: Vec::new(),
            geometry_subgroups: Vec::new(),
        }
    }
}
pub fn context(s: &State) -> Option<String> {
    let tab = s.tab()?;
    let db = tab.database.as_ref()?;
    let view = db.views.get(tab.view)?;
    (view.kind == ViewKind::Board).then(|| format!("{}:{}", db.id, view.id))
}
pub fn contains(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0]
        && point[1] >= rect[1]
        && point[0] < rect[0] + rect[2]
        && point[1] < rect[1] + rect[3]
}
fn target(ui: &BoardUi, zones: &[DropZone], point: [f32; 2], source: &str) -> Option<Destination> {
    let zone = zones.iter().find(|zone| {
        let Some(column) = ui.drop_zones.get(&zone.key) else {
            return false;
        };
        let Some(lane) = ui.lane_rects.get(&zone.lane) else {
            return false;
        };
        let bottom = if zone.extend_bottom {
            (lane[1] + lane[3]).max(ui.viewport[1] + ui.viewport[3])
        } else {
            lane[1] + lane[3]
        };
        contains([column[0], lane[1], column[2], bottom - lane[1]], point)
    })?;
    let before = zone
        .cards
        .iter()
        .find(|id| {
            id.as_str() != source
                && ui
                    .card_rects
                    .get(*id)
                    .is_some_and(|r| point[1] < r[1] + r[3] / 2.)
        })
        .cloned();
    Some(Destination {
        group: zone.group.clone(),
        subgroup: zone.subgroup.clone(),
        before,
        key: zone.key.clone(),
    })
}

impl State {
    pub fn cancel_board_group_drag(&mut self) {
        let ui = &mut self.database_views.board_ui;
        ui.group_drag = None;
        ui.group_drop = None;
        ui.group_context = None;
        ui.group_expected = None;
        ui.group_point = None;
        self.database_views.board.frozen = false;
    }
    pub fn open_board_menu(&mut self, target: MenuTarget, anchor: String, point: Option<[f32; 2]>) {
        if !self.finish_cell(false) {
            return;
        }
        self.view_management = None;
        self.database_views.board_ui.error = None;
        self.database_views.board_ui.menu = Some(BoardMenu {
            target,
            anchor,
            point,
            pane: MenuPane::Main,
        });
    }
    pub fn board_card_drag(
        &mut self,
        record: &Record,
        property: &Property,
        subgroup: Option<&Property>,
        zones: &[DropZone],
        drag: &Drag,
        tx: &mut Tx,
    ) {
        if drag.cancelled() {
            self.database_views.board_ui.drag = None;
            self.database_views.board.frozen = false;
            self.database_views.board_ui.suppress_press = true;
            tx.instant();
            return;
        }
        if drag.phase == Phase::Down {
            let Some(context) = context(self) else {
                return;
            };
            self.database_views.board.frozen = true;
            self.database_views.board_ui.suppress_press = false;
            let point = drag.window_point();
            let origin = self
                .database_views
                .board_ui
                .card_rects
                .get(&record.id)
                .map(|r| [r[0], r[1]])
                .unwrap_or([point[0] - drag.x, point[1] - drag.y]);
            self.database_views.board_ui.drag = Some(CardDrag {
                record: record.clone(),
                property: property.clone(),
                subgroup: subgroup.cloned(),
                context,
                moved: false,
                origin,
                delta: [0., 0.],
                point: drag.window_point(),
                destination: None,
            });
            tx.live_only();
            return;
        }
        let ui = &self.database_views.board_ui;
        let destination = contains(ui.viewport, drag.window_point())
            .then(|| target(ui, zones, drag.window_point(), &record.id))
            .flatten();
        let Some(active) = &mut self.database_views.board_ui.drag else {
            return;
        };
        active.moved |= drag.dx.hypot(drag.dy) >= 4.;
        active.delta = [drag.dx, drag.dy];
        active.point = drag.window_point();
        active.destination = destination;
        if drag.phase == Phase::Up {
            let active = self.database_views.board_ui.drag.take().unwrap();
            self.database_views.board.frozen = false;
            self.database_views.board_ui.suppress_press = active.moved;
            if context(self).as_deref() != Some(active.context.as_str()) {
                tx.instant();
                return;
            }
            if active.moved {
                if let Some(at) = active.destination {
                    if !self.commit_board_move(
                        &active.record,
                        &active.property,
                        active.subgroup.as_ref(),
                        at.group,
                        at.subgroup,
                        at.before,
                    ) {
                        self.database_views.board_ui.error = self.error.clone();
                    }
                }
            }
        }
        tx.instant();
    }
    pub fn duplicate_board_card_action(&mut self, id: &str) {
        if !self.finish_cell(false) {
            return;
        }
        let Some(db) = self
            .tab()
            .and_then(|tab| tab.database.as_ref())
            .map(|db| db.id.clone())
        else {
            return;
        };
        match self.store.duplicate_board_record(&db, id) {
            Ok(_) => {
                self.database_views.board.invalidate();
                self.database_views.board_ui.menu = None;
                self.query_records();
            }
            Err(e) => {
                self.database_views.board_ui.error = Some(format!("Could not duplicate page: {e}"))
            }
        }
    }
}
pub fn poll(s: &mut State, time: f64, tx: &mut Tx) -> bool {
    let current = context(s);
    if s.database_views.board_ui.context != current {
        let ui = &mut s.database_views.board_ui;
        let viewport = ui.viewport;
        let drop_zones = std::mem::take(&mut ui.drop_zones);
        let lane_rects = std::mem::take(&mut ui.lane_rects);
        let card_rects = std::mem::take(&mut ui.card_rects);
        s.database_views.board_ui = BoardUi {
            context: current,
            viewport,
            drop_zones,
            lane_rects,
            card_rects,
            ..Default::default()
        };
        s.database_views.board.frozen = false;
        return false;
    }
    let active = s.editing_cell.as_ref().map(|edit| edit.record_id.clone());
    let config = s
        .tab()
        .and_then(|t| t.database.as_ref().and_then(|db| db.views.get(t.view)))
        .map(|v| v.config.clone());
    let ui = &mut s.database_views.board_ui;
    if ui.layout_config != config {
        ui.card_heights.clear();
        ui.layout_config = config;
    }
    let scroll_changed =
        ui.last_scroll != ui.scroll.get().offset || ui.last_height_version != ui.height_version;
    ui.last_height_version = ui.height_version;
    ui.last_scroll = ui.scroll.get().offset;
    if let Some(projection) = &s.database_views.board.projection {
        ui.covers.retain(|id, _| projection.covers.contains_key(id));
        ui.card_heights
            .retain(|id, _| projection.order.contains_key(id) || active.as_ref() == Some(id));
        ui.card_rects
            .retain(|id, _| projection.order.contains_key(id) || active.as_ref() == Some(id));
        if ui.geometry_groups != projection.groups || ui.geometry_subgroups != projection.subgroups
        {
            ui.geometry_groups.clone_from(&projection.groups);
            ui.geometry_subgroups.clone_from(&projection.subgroups);
            let lanes: std::collections::BTreeSet<_> = projection
                .subgroups
                .iter()
                .map(|sub| format!("board-lane-{}", moss_core::board::group_key(sub)))
                .collect();
            ui.lane_rects.retain(|key, _| lanes.contains(key));
            let zones: std::collections::BTreeSet<_> = projection
                .groups
                .iter()
                .flat_map(|group| {
                    projection.subgroups.iter().map(move |sub| {
                        format!("board-bucket-{}", moss_core::board::bucket_key(group, sub))
                    })
                })
                .collect();
            ui.drop_zones.retain(|key, _| zones.contains(key));
        }
    }
    if ui.drag.is_none() {
        ui.suppress_press = false;
    }
    let point = ui
        .drag
        .as_ref()
        .filter(|d| d.moved)
        .map(|drag| drag.point)
        .or(ui.group_point);
    let Some(p) = point else {
        return scroll_changed;
    };
    let r = ui.viewport;
    let edge = |value: f32, start: f32, length: f32| {
        if value < start + 28. {
            -14.
        } else if value > start + length - 28. {
            14.
        } else {
            0.
        }
    };
    let delta = [edge(p[0], r[0], r[2]), edge(p[1], r[1], r[3])];
    if delta == [0., 0.] || !contains(r, p) {
        return false;
    }
    tx.wake_after(Duration::from_millis(16));
    if time - ui.scroll_at < 1. / 60. {
        return false;
    }
    ui.scroll_at = time;
    let position = ui.scroll.get();
    let next = [
        (position.offset[0] + delta[0]).clamp(0., position.max[0]),
        (position.offset[1] + delta[1]).clamp(0., position.max[1]),
    ];
    if next == position.offset {
        return false;
    }
    tx.scroll_to_instant("db-body", next[0], next[1]);
    true
}
