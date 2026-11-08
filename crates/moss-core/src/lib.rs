mod advanced_properties;
mod archive;
pub mod board;
mod board_copy;
mod board_settings;
mod calculation;
pub mod calendar;
pub mod record_layout;
pub use calculation::calculation_error;
pub mod chart;
mod child_page_blocks;
pub mod dashboard;
pub mod formula;
pub mod map;
pub mod model;
mod navigation;
pub mod page_covers;
mod page_icons;
mod page_titles;
mod previews;
mod properties;
mod record_icons;
mod record_pages;
mod record_paging;
mod record_snapshots;
mod record_values;
mod record_windows;
mod relations;
pub mod rollup;
mod schema;
mod search;
mod seeds;
pub mod simple_table;
mod store;
mod summaries;
mod view_config;
mod view_management;
mod view_query;

pub use archive::ArchiveError;
pub use model::*;
pub use navigation::{PageChild, PageChildren, PageCursor, PagePosition};
pub use record_icons::{RecordPageAppearance, blocks_have_content};
pub use record_paging::{PageDirection, RecordCursor};
pub use record_windows::{RecordWindow, RecordWindowState};
pub use rusqlite::InterruptHandle;
pub use store::Store;

#[cfg(test)]
mod store_tests;

#[cfg(test)]
mod navigation_tests;

#[cfg(test)]
mod atomic_edits_tests;

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("{0}")]
pub struct EngineError(pub String);

pub(crate) fn error(message: impl Into<String>) -> EngineError {
    EngineError(message.into())
}

#[cfg(test)]
mod calculation_tests;

pub use map::{MapBounds, MapPoint, MapPoints, PlaceValue};
