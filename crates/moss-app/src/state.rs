use moss_core::*;
use rune::TextEdit;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};

pub struct EditableBlock {
    pub block: Block,
    pub edit: TextEdit,
}
pub struct Tab {
    pub page: Page,
    pub title: TextEdit,
    pub blocks: Vec<EditableBlock>,
    pub choosing_type: bool,
    pub child_pages: Option<PageChildren>,
    pub page_links: BTreeMap<String, Page>,
    pub child_page_cursor: Option<(PageCursor, bool)>,
    pub database: Option<Database>,
    pub record_context: Option<(Database, Record)>,
    pub records: Vec<Record>,
    pub table_window: Option<crate::table_window::TableWindow>,
    pub created_record_ids: Vec<String>,
    pub has_more_records: bool,
    pub gallery_previews: BTreeMap<String, String>,
    pub gallery_covers: BTreeMap<String, PageCover>,
    pub record_icons: BTreeMap<String, String>,
    pub record_has_content: BTreeMap<String, bool>,
    pub relation_titles: BTreeMap<String, String>,
    pub summary_values: BTreeMap<String, crate::table_summaries::SummaryValue>,
    pub summary_key: String,
    pub summary_epoch: u64,
    pub title_search: String,
    pub view: usize,
    pub offset: u64,
    pub(crate) paging_next: Option<RecordCursor>,
    pub(crate) paging_previous: Option<RecordCursor>,
    pub(crate) paging_request: Option<(RecordCursor, PageDirection)>,
    pub calendar_month: chrono::NaiveDate,
    pub timeline_start: chrono::NaiveDate,
    pub(crate) dirty_title: bool,
    pub(crate) dirty_blocks: bool,
}
pub struct CellTarget {
    pub record_id: String,
    pub property_id: Option<String>,
    pub expected_value: Value,
    pub save_failed: bool,
    pub repeat_values: Option<BTreeMap<String, Value>>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PeekMode {
    Side,
    Center,
}
pub struct RecordPeek {
    pub database: Database,
    pub record: Record,
    pub page: Page,
    pub title: TextEdit,
    pub blocks: Vec<EditableBlock>,
    pub page_links: BTreeMap<String, Page>,
    pub mode: PeekMode,
    pub dirty_title: bool,
    pub dirty_blocks: bool,
}
pub struct NewItemDraft {
    pub record: Option<Record>,
    pub page: Option<Page>,
    pub blocks: Vec<Block>,
    pub created_link: Option<Block>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    Search,
    Settings,
    PageActions,
    Appearance,
    Trash,
    Workspace,
    Views,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Preferences {
    pub dark: Option<bool>,
    #[serde(default)]
    pub start_week_on_monday: bool,
    #[serde(default)]
    pub week_start: Option<u8>,
    pub workspace: String,
    pub tabs: BTreeMap<String, Vec<String>>,
    pub active: BTreeMap<String, usize>,
    #[serde(default)]
    pub expanded_pages: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub sidebar_widths: BTreeMap<String, f32>,
    #[serde(default)]
    pub collapsed_sidebars: BTreeMap<String, bool>,
    #[serde(default)]
    pub selected_views: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub calendar_positions: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub favorite_pages: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub sidebar_sections: BTreeMap<String, Vec<crate::sidebar_sections::SidebarSection>>,
    #[serde(default)]
    pub recent_pages: BTreeMap<String, Vec<String>>,
}
impl Preferences {
    pub fn week_start(&self) -> u8 {
        self.week_start
            .filter(|day| *day < 7)
            .unwrap_or(u8::from(self.start_week_on_monday))
    }
}
pub struct State {
    pub editor_selection: Option<crate::editor_selection::Selection>,
    pub editor_drag: Option<crate::editor_drag::BlockDrag>,
    pub editor_column_resize: Option<crate::editor_drag::ColumnResize>,
    pub editor_drag_cancelled: bool,
    pub editor_drag_suppress_press: bool,
    pub page_appearance: crate::page_appearance::PageAppearance,
    pub page_cover: crate::page_covers::CoverState,
    pub archive_transfer: crate::archive_transfer::ArchiveTransfer,
    pub simple_table: crate::simple_table::SimpleTableState,
    pub database_views: crate::database_views::DatabaseViews,
    pub summary_jobs: crate::table_summaries::SummaryJobs,
    pub table_window_jobs: crate::table_window::TableWindowJobs,
    pub store: Store,
    pub path: PathBuf,
    pub workspaces: Vec<Workspace>,
    pub workspace: Workspace,
    pub pages: Vec<Page>,
    pub sidebar_tree: crate::sidebar_tree::SidebarTree,
    pub sidebar_reveal: crate::sidebar_reveal::SidebarReveal,
    pub sidebar_keyboard_controls: bool,
    pub sidebar_sections: crate::sidebar_sections::SidebarSections,
    pub sidebar_section_menu: Option<crate::sidebar_section_menu::SidebarSectionMenu>,
    pub sidebar_section_name: TextEdit,
    pub sidebar_section_search: TextEdit,
    pub caret: crate::caret::CaretBlink,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub navigation_history: Vec<String>,
    pub navigation_position: usize,
    navigating_history: bool,
    pub prefs: Preferences,
    pub shared_preferences: std::sync::Arc<std::sync::Mutex<Preferences>>,
    pub modal: Option<Modal>,
    pub search: TextEdit,
    pub all_workspaces: bool,
    pub search_new_tab: bool,
    pub search_selected: usize,
    pub hits: Vec<SearchHit>,
    pub trash: Vec<Page>,
    pub filter: TextEdit,
    pub view_search: TextEdit,
    pub view_management: Option<crate::view_management::ViewEditor>,
    pub view_name_input: TextEdit,
    pub page_icon_picker: Option<crate::page_personalization::PageIconPicker>,
    pub page_icon_search: TextEdit,
    pub block_commands: Option<crate::block_commands::BlockCommands>,
    pub block_command_search: TextEdit,
    pub recent_page_icons: Vec<String>,
    #[cfg(test)]
    pub new_name: TextEdit,
    pub error: Option<String>,
    pub(crate) loading: bool,
    pub cell: TextEdit,
    pub editing_cell: Option<CellTarget>,
    pub selected_cell: Option<(String, Option<String>)>,
    pub record_peek: Option<RecordPeek>,
    pub new_item_drafts: BTreeMap<String, NewItemDraft>,
    pub pending_empty_items: Vec<String>,
    pub gallery_settings_open: bool,
    pub database_new_open: bool,
    pub database_property_editor: Option<crate::database_properties::PropertyEditor>,
    pub metadata_menu: Option<crate::database_properties::MetadataMenu>,
    pub metadata_drag: Option<String>,
    pub metadata_drop_before: Option<String>,
    pub metadata_suppress_press: bool,
    pub database_property_input: TextEdit,
    pub database_property_options_input: TextEdit,
    pub table_filter_input: TextEdit,
    pub advanced_property: crate::advanced_properties::AdvancedProperty,
    pub computed_change_count: u64,
    pub computed_refresh_deferred: bool,
    pub sidebar_page_menu: Option<crate::sidebar_page_menu::SidebarPageMenu>,
    pub sidebar_favorites: crate::sidebar_favorites::SidebarFavorites,
    pub sidebar_page_name: TextEdit,
    pub property_visibility_open: bool,
    pub property_visibility_from_view_settings: bool,
    pub property_visibility_search: TextEdit,
    pub undated_open: bool,
    pub undated_search: TextEdit,
    pub property_picker: Option<crate::property_controls::PropertyPicker>,
    pub place_picker: Option<crate::place_property::PlacePicker>,
    pub property_picker_input: TextEdit,
    pub date_end_input: TextEdit,
    pub date_start_time_input: TextEdit,
    pub date_end_time_input: TextEdit,
    pub timeline_drag: Option<(chrono::NaiveDate, chrono::NaiveDate)>,
    pub handles: std::sync::Arc<std::sync::Mutex<BTreeMap<String, rune::AppHandle<State>>>>,
}
impl State {
    pub fn data_path() -> PathBuf {
        std::env::var_os("MOSS_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let support = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                    .join("Library/Application Support");
                support.join("Moss")
            })
    }
    pub fn open(
        path: PathBuf,
        workspace_id: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(&path)?;
        let mut store = Store::open(path.join("workspace.sqlite3"))?;
        if store.list_workspaces()?.is_empty() {
            if std::env::var_os("MOSS_DEMO").is_some_and(|value| value != "0") {
                store.seed_demo_workspaces()?;
            } else {
                let w = store.create_workspace("Personal", "◈")?;
                let page =
                    store.create_page(&w.id, None, "My workspace", "📖", PageKind::Document)?;
                let mut blocks = vec![
                    Block::paragraph(&page.id, "A place for your notes, plans, and ideas.", 0),
                    Block::paragraph(&page.id, "Make room for what matters", 1),
                    Block::paragraph(
                        &page.id,
                        "Start writing here. Add a page from the sidebar, or press / in an empty block to choose a block type.",
                        2,
                    ),
                    Block::paragraph(&page.id, "Everything you write stays on this Mac.", 3),
                ];
                blocks[1].kind = BlockKind::Heading2;
                store.save_blocks(&page.id, &blocks)?;
            }
        }
        let workspaces = store.list_workspaces()?;
        let prefs: Preferences = std::fs::read(path.join("preferences.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let id = workspace_id.unwrap_or(&prefs.workspace);
        let workspace = workspaces
            .iter()
            .find(|w| w.id == id)
            .unwrap_or(&workspaces[0])
            .clone();
        let pages = store.page_suggestions(&workspace.id, 30)?;
        let restore = prefs.tabs.get(&workspace.id).cloned().unwrap_or_default();
        let active = *prefs.active.get(&workspace.id).unwrap_or(&0);
        let mut s = Self {
            editor_selection: None,
            editor_drag: None,
            editor_column_resize: None,
            editor_drag_cancelled: false,
            editor_drag_suppress_press: false,
            page_appearance: Default::default(),
            page_cover: Default::default(),
            archive_transfer: Default::default(),
            simple_table: Default::default(),
            database_views: Default::default(),
            summary_jobs: Default::default(),
            table_window_jobs: Default::default(),
            store,
            path,
            workspaces,
            workspace,
            pages,
            sidebar_tree: Default::default(),
            sidebar_reveal: Default::default(),
            sidebar_keyboard_controls: false,
            sidebar_sections: Default::default(),
            sidebar_section_menu: None,
            sidebar_section_name: TextEdit::new(""),
            sidebar_section_search: TextEdit::new(""),
            caret: Default::default(),
            tabs: vec![],
            active: 0,
            navigation_history: vec![],
            navigation_position: 0,
            navigating_history: false,
            shared_preferences: std::sync::Arc::new(std::sync::Mutex::new(prefs.clone())),
            prefs,
            modal: None,
            search: TextEdit::new(""),
            all_workspaces: false,
            search_new_tab: false,
            search_selected: 0,
            hits: vec![],
            trash: vec![],
            filter: TextEdit::new(""),
            view_search: TextEdit::new(""),
            view_management: None,
            view_name_input: TextEdit::new(""),
            page_icon_picker: None,
            page_icon_search: TextEdit::new(""),
            block_commands: None,
            block_command_search: TextEdit::new(""),
            recent_page_icons: Vec::new(),
            #[cfg(test)]
            new_name: TextEdit::new(""),
            error: None,
            loading: true,
            cell: TextEdit::new(""),
            editing_cell: None,
            selected_cell: None,
            record_peek: None,
            new_item_drafts: BTreeMap::new(),
            pending_empty_items: Vec::new(),
            gallery_settings_open: false,
            database_new_open: false,
            database_property_editor: None,
            metadata_menu: None,
            metadata_drag: None,
            metadata_drop_before: None,
            metadata_suppress_press: false,
            database_property_input: TextEdit::new(""),
            database_property_options_input: TextEdit::new(""),
            table_filter_input: TextEdit::new(""),
            advanced_property: Default::default(),
            computed_change_count: 0,
            computed_refresh_deferred: false,
            sidebar_page_menu: None,
            sidebar_favorites: Default::default(),
            sidebar_page_name: TextEdit::new(""),
            property_visibility_open: false,
            property_visibility_from_view_settings: false,
            property_visibility_search: TextEdit::new(""),
            undated_open: false,
            undated_search: TextEdit::new(""),
            property_picker: None,
            place_picker: None,
            property_picker_input: TextEdit::new(""),
            date_end_input: TextEdit::new(""),
            date_start_time_input: TextEdit::new(""),
            date_end_time_input: TextEdit::new(""),
            timeline_drag: None,
            handles: Default::default(),
        };
        s.refresh_sidebar_tree();
        s.refresh_sidebar_favorites();
        s.refresh_sidebar_sections();
        for id in restore {
            if s.store
                .get_page(&id)?
                .is_some_and(|p| p.workspace_id == s.workspace.id && p.deleted_at.is_none())
            {
                s.open_page(&id, true);
            }
        }
        if s.tabs.is_empty() {
            if let Some(id) = s.sidebar_tree.first_root().map(str::to_owned) {
                s.open_page(&id, true);
            }
        }
        if !s.tabs.is_empty() {
            s.active = active.min(s.tabs.len() - 1);
        }
        s.loading = false;
        if let Some(tab) = s.tab() {
            s.navigation_history.push(tab.page.id.clone());
        }
        s.computed_change_count = s.store.local_change_count();
        Ok(s)
    }
    pub fn tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }
    pub fn sidebar_width(&self, window_width: f32) -> f32 {
        let requested = self
            .prefs
            .sidebar_widths
            .get(&self.workspace.id)
            .copied()
            .filter(|w| w.is_finite())
            .unwrap_or(270.);
        requested.clamp(200., (window_width - 400.).clamp(200., 480.))
    }
    pub fn sidebar_collapsed(&self) -> bool {
        self.prefs
            .collapsed_sidebars
            .get(&self.workspace.id)
            .copied()
            .unwrap_or(false)
    }
    pub fn toggle_sidebar(&mut self) {
        self.close_sidebar_reveal();
        let collapsed = !self.sidebar_collapsed();
        self.prefs
            .collapsed_sidebars
            .insert(self.workspace.id.clone(), collapsed);
        self.save_preferences();
    }
    pub fn set_sidebar_width(&mut self, width: f32, window_width: f32) {
        if width.is_finite() {
            self.prefs
                .sidebar_widths
                .insert(self.workspace.id.clone(), width);
            let width = self.sidebar_width(window_width).round();
            self.prefs
                .sidebar_widths
                .insert(self.workspace.id.clone(), width);
        }
    }
    pub fn save_preferences(&mut self) {
        if self.loading {
            return;
        }
        let owner = self.shared_preferences.clone();
        let mut shared = owner.lock().unwrap();
        shared.workspace = self.workspace.id.clone();
        if let Some(collapsed) = self.prefs.collapsed_sidebars.get(&self.workspace.id) {
            shared
                .collapsed_sidebars
                .insert(self.workspace.id.clone(), *collapsed);
        }
        if let Some(width) = self.prefs.sidebar_widths.get(&self.workspace.id) {
            shared
                .sidebar_widths
                .insert(self.workspace.id.clone(), *width);
        }
        shared.tabs.insert(
            self.workspace.id.clone(),
            self.tabs.iter().map(|t| t.page.id.clone()).collect(),
        );
        shared.active.insert(self.workspace.id.clone(), self.active);
        let selected = shared
            .selected_views
            .entry(self.workspace.id.clone())
            .or_default();
        for tab in &self.tabs {
            if let Some(view) = tab.database.as_ref().and_then(|db| db.views.get(tab.view)) {
                selected.insert(tab.page.id.clone(), view.id.clone());
            }
        }
        let calendar = shared
            .calendar_positions
            .entry(self.workspace.id.clone())
            .or_default();
        for tab in &self.tabs {
            if let Some(view) = tab
                .database
                .as_ref()
                .and_then(|db| db.views.get(tab.view))
                .filter(|view| view.kind == ViewKind::Calendar)
            {
                calendar.insert(view.id.clone(), tab.calendar_month.to_string());
            }
        }
        self.prefs = shared.clone();
        let result = serde_json::to_vec(&self.prefs)
            .map_err(|e| e.to_string())
            .and_then(|b| {
                std::fs::write(self.path.join("preferences.json.tmp"), b).map_err(|e| e.to_string())
            })
            .and_then(|_| {
                std::fs::rename(
                    self.path.join("preferences.json.tmp"),
                    self.path.join("preferences.json"),
                )
                .map_err(|e| e.to_string())
            });
        if let Err(e) = result {
            self.error = Some(format!("Could not save window settings: {e}"));
        }
    }
    pub fn set_theme(&mut self, dark: Option<bool>) {
        self.shared_preferences.lock().unwrap().dark = dark;
        self.prefs.dark = dark;
        if let Ok(handles) = self.handles.lock() {
            for h in handles.values() {
                h.update(move |s, _| s.prefs.dark = dark);
            }
        }
        self.save_preferences();
    }
    pub fn set_week_start(&mut self, day: u8) {
        if day >= 7 {
            return;
        }
        self.shared_preferences.lock().unwrap().week_start = Some(day);
        self.prefs.week_start = Some(day);
        if let Ok(handles) = self.handles.lock() {
            for handle in handles.values() {
                handle.update(move |s, _| s.prefs.week_start = Some(day));
            }
        }
        self.save_preferences();
    }
    pub fn set_page_settings(&mut self, settings: PageSettings) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        match self.store.set_page_settings_if_unchanged(
            &self.workspace.id,
            &tab.page.id,
            tab.page.settings,
            settings,
        ) {
            Ok(page) => {
                self.tabs[self.active].page = page.clone();
                if let Some(p) = self.pages.iter_mut().find(|p| p.id == page.id) {
                    *p = page;
                }
            }
            Err(e) => self.error = Some(format!("Page settings are not saved: {e}")),
        }
    }
    pub(crate) fn record_navigation(&mut self, id: &str) {
        if self.loading {
            return;
        }
        self.record_sidebar_recent(id);
        if self.navigating_history {
            return;
        }
        if self
            .navigation_history
            .get(self.navigation_position)
            .is_some_and(|page| page == id)
        {
            return;
        }
        self.navigation_history
            .truncate(self.navigation_position.saturating_add(1));
        self.navigation_history.push(id.to_owned());
        self.navigation_position = self.navigation_history.len() - 1;
    }
    pub fn navigate_history(&mut self, forward: bool) {
        let target = if forward {
            self.navigation_position.checked_add(1)
        } else {
            self.navigation_position.checked_sub(1)
        };
        let Some(target) = target.filter(|i| *i < self.navigation_history.len()) else {
            return;
        };
        let id = self.navigation_history[target].clone();
        self.navigating_history = true;
        self.open_page(&id, false);
        self.navigating_history = false;
        if self.tab().is_some_and(|tab| tab.page.id == id) {
            if let Some((position, _)) = self
                .navigation_history
                .iter()
                .enumerate()
                .filter(|(_, page)| **page == id)
                .min_by_key(|(position, _)| position.abs_diff(target))
            {
                self.navigation_position = position;
            }
        }
    }
    pub fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() || !self.flush_current() {
            return;
        }
        if !self.discard_draft_for_page(&self.tabs[i].page.id.clone()) {
            return;
        }
        self.modal = None;
        self.tabs.remove(i);
        if i < self.active {
            self.active -= 1;
        } else {
            self.active = self.active.min(self.tabs.len().saturating_sub(1));
        }
        self.filter = TextEdit::new(
            self.tab()
                .map(|tab| tab.title_search.as_str())
                .unwrap_or(""),
        );
        self.save_preferences();
    }
    pub fn flush_current(&mut self) -> bool {
        self.metadata_menu = None;
        if !self.finish_page_cover() {
            return false;
        }
        if !self.finish_simple_table() {
            return false;
        }
        self.dismiss_block_commands();
        if self.block_commands.is_some() {
            return false;
        }
        self.dismiss_page_icon_picker();
        if self.page_icon_picker.is_some() {
            return false;
        }
        self.close_relation_picker();
        if self.advanced_property.relation_picker.is_some() {
            return false;
        }
        self.database_new_open = false;
        self.gallery_settings_open = false;
        self.property_visibility_open = false;
        self.property_visibility_search = TextEdit::new("");
        self.undated_open = false;
        self.undated_search = TextEdit::new("");
        self.selected_cell = None;
        self.dismiss_property_picker();
        if self.property_picker.is_some() {
            return false;
        }
        if !self.finish_cell(false) || !self.close_peek() {
            return false;
        }
        let Some(tab) = self.tab() else {
            self.database_property_editor = None;
            self.sidebar_page_menu = None;
            return true;
        };
        let (title, blocks) = (tab.dirty_title, tab.dirty_blocks);
        if title {
            self.save_title();
        }
        if blocks {
            self.save_blocks();
        }
        let saved = self.tab().is_none_or(|t| !t.dirty_title && !t.dirty_blocks);
        if saved {
            self.database_property_editor = None;
            self.sidebar_page_menu = None;
        }
        saved
    }
    pub fn refresh_pages(&mut self) {
        match self.store.page_suggestions(&self.workspace.id, 30) {
            Ok(p) => self.pages = p,
            Err(e) => self.error = Some(e.to_string()),
        }
        self.refresh_sidebar_tree();
        self.refresh_sidebar_favorites();
        self.refresh_sidebar_sections();
    }
    pub fn save_title(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        tab.dirty_title = true;
        let result = if tab.page.kind == PageKind::Record {
            match self.store.get_record_by_page(&tab.page.id) {
                Ok(Some(record)) => self
                    .store
                    .rename_record(&record.id, &tab.title.text)
                    .map(Some),
                Ok(None) => {
                    self.error = Some("The linked record is missing.".into());
                    return;
                }
                Err(e) => Err(e),
            }
        } else {
            self.store
                .rename_page_if_unchanged(&tab.page.id, &tab.page.title, &tab.title.text)
                .map(|page| {
                    tab.page = page;
                    None
                })
        };
        let saved = match result {
            Ok(record) => {
                tab.dirty_title = false;
                if let Some(record) = &record {
                    tab.page.title = record.title.clone();
                    tab.page.updated_at = record.updated_at.clone();
                }
                record
            }
            Err(e) => {
                self.error = Some(format!("Title is not saved: {e}"));
                None
            }
        };
        if let Some(page) = self.pages.iter_mut().find(|p| p.id == tab.page.id) {
            *page = tab.page.clone();
        }
        self.sidebar_tree.update_page(&tab.page);
        let updated_page = (!tab.dirty_title).then(|| tab.page.clone());
        if let Some(page) = updated_page {
            self.sidebar_favorites.update_page(&page);
            self.sidebar_sections.update_page(&page);
            self.update_child_page(&page);
        }
        if let Some(record) = saved {
            self.sync_record(&record);
        }
    }
    pub fn save_blocks(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        tab.dirty_blocks = true;
        let blocks: Vec<Block> = tab
            .blocks
            .iter_mut()
            .enumerate()
            .map(|(i, b)| {
                crate::rich_text::save(&mut b.block, &b.edit);
                b.block.sort_order = i as i64;
                b.block.clone()
            })
            .collect();
        let page_id = tab.page.id.clone();
        match self.store.save_editor_blocks(&page_id, &blocks) {
            Ok(blocks) => {
                crate::page_covers::sync_saved_image_assets(&mut tab.blocks, &blocks);
                tab.dirty_blocks = false;
                self.sync_gallery_preview(&page_id, &blocks);
                self.sync_record_content(&page_id, &blocks);
                self.keep_draft_for_page(&page_id);
            }
            Err(e) => {
                self.error = Some(format!(
                    "Changes are not saved. Keep this page open and try again: {e}"
                ))
            }
        }
    }
    pub fn add_block(&mut self, after: usize) {
        let tab = &mut self.tabs[self.active];
        if tab.blocks.is_empty() {
            tab.blocks.push(EditableBlock {
                block: Block::paragraph(&tab.page.id, "", 0),
                edit: TextEdit::new("").rich(),
            });
        }
        let after = after.min(tab.blocks.len() - 1);
        let edit = &mut tab.blocks[after].edit;
        let (start, end) = edit.range();
        let mut suffix = edit.clone();
        suffix.set_selection(0, end);
        suffix.insert("");
        suffix.clear_history();
        edit.set_selection(start, edit.text.len());
        edit.insert("");
        edit.focused = false;
        let mut edit = suffix;
        edit.set_caret(0);
        edit.focused = true;
        let mut block = Block::paragraph(&tab.page.id, "", after as i64 + 1);
        crate::editor_drag::copy_column(&tab.blocks[after].block, &mut block);
        let oldkind = tab.blocks[after].block.kind;
        if matches!(
            oldkind,
            BlockKind::BulletedList | BlockKind::NumberedList | BlockKind::Todo
        ) {
            block.kind = oldkind;
        }
        tab.blocks.insert(after + 1, EditableBlock { block, edit });
        self.save_blocks();
    }
    pub fn open_view_chooser(&mut self) {
        self.dismiss_property_picker();
        if self.property_picker.is_some() {
            return;
        }
        self.gallery_settings_open = false;
        self.property_visibility_open = false;
        self.property_visibility_search.focused = false;
        self.undated_open = false;
        self.undated_search.focused = false;
        self.view_search = TextEdit::new("");
        self.view_search.focused = true;
        self.modal = Some(Modal::Views);
    }
    pub fn open_search(&mut self) {
        self.start_search(false);
    }
    pub fn open_new_tab_search(&mut self) {
        self.start_search(true);
    }
    fn start_search(&mut self, new_tab: bool) {
        self.dismiss_property_picker();
        if self.property_picker.is_some() {
            return;
        }
        self.gallery_settings_open = false;
        self.property_visibility_open = false;
        self.property_visibility_search.focused = false;
        self.undated_open = false;
        self.undated_search.focused = false;
        self.search = TextEdit::new("");
        self.search.focused = true;
        self.all_workspaces = false;
        self.search_new_tab = new_tab;
        self.modal = Some(Modal::Search);
        self.run_search();
    }
    pub fn run_search(&mut self) {
        self.search_selected =
            usize::from(self.search_new_tab && !self.search.text.trim().is_empty());
        if self.search.text.trim().is_empty() {
            if self.all_workspaces {
                self.hits.clear();
                return;
            }
            self.hits = self
                .pages
                .iter()
                .take(30)
                .map(|p| SearchHit {
                    page_id: p.id.clone(),
                    workspace_id: p.workspace_id.clone(),
                    title: p.title.clone(),
                    snippet: String::new(),
                    kind: p.kind,
                })
                .collect();
            return;
        }
        match self.store.search(
            &self.search.text,
            if self.all_workspaces {
                None
            } else {
                Some(&self.workspace.id)
            },
            30,
        ) {
            Ok(h) => self.hits = h,
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn open_search_hit(&mut self, index: usize) {
        let Some(hit) = self.hits.get(index).cloned() else {
            return;
        };
        let new_tab = self.search_new_tab;
        if hit.workspace_id == self.workspace.id {
            self.open_page(&hit.page_id, new_tab);
            if self.modal == Some(Modal::Search) && self.error.is_some() {
                self.modal = None;
            }
        } else {
            let handle = self
                .handles
                .lock()
                .ok()
                .and_then(|h| h.get(&hit.workspace_id).cloned());
            if let Some(handle) = handle {
                handle.update(move |s, tx| {
                    tx.instant();
                    s.open_page(&hit.page_id, new_tab);
                });
                handle.open();
                self.modal = None;
            } else {
                self.error = Some("The workspace window is not available.".into());
            }
        }
    }
    pub fn open_trash(&mut self) {
        match self.store.list_trash(&self.workspace.id) {
            Ok(p) => {
                self.trash = p;
                self.modal = Some(Modal::Trash)
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn trash_page(&mut self) {
        let Some(t) = self.tab() else { return };
        let id = t.page.id.clone();
        self.trash_page_by_id(&id);
    }
    pub fn restore_page(&mut self, id: &str) {
        let parent_id = self
            .store
            .get_page(id)
            .ok()
            .flatten()
            .and_then(|p| p.parent_id);
        match self.store.restore_page(id) {
            Ok(()) => {
                if let Some(parent_id) = parent_id.as_deref() {
                    self.refresh_child_pages(parent_id);
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.refresh_pages();
        self.open_trash();
    }
    pub fn switch_workspace(&mut self, id: &str) {
        if id == self.workspace.id {
            self.modal = None;
            return;
        }
        if let Ok(handles) = self.handles.lock() {
            if let Some(h) = handles.get(id) {
                h.open();
                self.modal = None;
                return;
            }
        }
        self.error =
            Some("This workspace window is not available. Restart Moss to open it.".into());
    }
}
pub fn display_value(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(display_value).collect::<Vec<_>>().join(", "),
        Value::Object(o)
            if o.get("latitude").is_some_and(Value::is_number)
                && o.get("longitude").is_some_and(Value::is_number) =>
        {
            o.get("name")
                .and_then(Value::as_str)
                .unwrap_or("Place")
                .to_owned()
        }
        _ => v.to_string(),
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod search_mode_tests;
