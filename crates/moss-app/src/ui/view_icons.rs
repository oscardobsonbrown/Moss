use moss_core::ViewKind;
use rune::*;

pub use super::phosphor::SLIDERS_HORIZONTAL as SETTINGS;
use super::phosphor::{
    CALENDAR_BLANK as CALENDAR, CARDS as FEED, CHART_BAR as CHART, IMAGES_SQUARE as GALLERY,
    KANBAN as BOARD, LIST_BULLETS as LIST, MAP_PIN as MAP, ROWS as TIMELINE,
    SQUARES_FOUR as DASHBOARD, TABLE,
};

pub fn icon(kind: ViewKind) -> &'static Icon {
    match kind {
        ViewKind::Table => &TABLE,
        ViewKind::Board => &BOARD,
        ViewKind::Gallery => &GALLERY,
        ViewKind::Calendar => &CALENDAR,
        ViewKind::Timeline => &TIMELINE,
        ViewKind::List => &LIST,
        ViewKind::Chart => &CHART,
        ViewKind::Feed => &FEED,
        ViewKind::Map => &MAP,
        ViewKind::Dashboard => &DASHBOARD,
    }
}
pub fn element(kind: ViewKind, size: f32, color: Color) -> El {
    rune::icon(icon(kind), size).color(color)
}
