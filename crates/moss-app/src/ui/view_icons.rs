use moss_core::ViewKind;
use rune::*;

use super::phosphor::{
    CALENDAR_BLANK as CALENDAR, CARDS as FEED, CHART_BAR as CHART, IMAGES_SQUARE as GALLERY,
    KANBAN as BOARD, LIST_BULLETS as LIST, MAP_PIN as MAP, ROWS as TIMELINE,
    SQUARES_FOUR as DASHBOARD, TABLE,
};

macro_rules! toolbar_icons {
    ($($id:ident => $name:literal),+ $(,)?) => {
        $(pub const $id: Icon = Icon {
            name: concat!("toolbar-", $name),
            view: 256.,
            body: include_str!(concat!("../../assets/phosphor/toolbar/", $name, ".svg")),
        };)+
    };
}
toolbar_icons! {
    TOOLBAR_TABLE => "table",
    TOOLBAR_BOARD => "kanban",
    TOOLBAR_CALENDAR => "calendar-blank",
    TOOLBAR_TIMELINE => "rows",
    TOOLBAR_GALLERY => "images-square",
    TOOLBAR_LIST => "list-bullets",
    TOOLBAR_CHART => "chart-bar",
    TOOLBAR_FEED => "cards",
    TOOLBAR_MAP => "map-pin",
    TOOLBAR_DASHBOARD => "squares-four",
    TOOLBAR_PLUS => "plus",
    TOOLBAR_SETTINGS => "sliders-horizontal",
    TOOLBAR_CARET => "caret-down",
}

pub fn toolbar_icon(kind: ViewKind) -> &'static Icon {
    match kind {
        ViewKind::Table => &TOOLBAR_TABLE,
        ViewKind::Board => &TOOLBAR_BOARD,
        ViewKind::Calendar => &TOOLBAR_CALENDAR,
        ViewKind::Timeline => &TOOLBAR_TIMELINE,
        ViewKind::Gallery => &TOOLBAR_GALLERY,
        ViewKind::List => &TOOLBAR_LIST,
        ViewKind::Chart => &TOOLBAR_CHART,
        ViewKind::Feed => &TOOLBAR_FEED,
        ViewKind::Map => &TOOLBAR_MAP,
        ViewKind::Dashboard => &TOOLBAR_DASHBOARD,
    }
}

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
