use crate::state::State;
use moss_core::{Page, PageKind};
use rune::{PointerPress, TextEdit};
use std::collections::HashSet;

pub struct SidebarPageMenu {
    pub page: Page,
    pub point: PointerPress,
    pub renaming: bool,
    pub anchor: Option<String>,
    pub icon_anchor: String,
    pub error: Option<String>,
}

impl State {
    fn sidebar_page_error(&mut self, message: String) {
        if let Some(menu) = &mut self.sidebar_page_menu {
            menu.error = Some(message);
        } else {
            self.error = Some(message);
        }
    }

    fn live_workspace_page(&self, id: &str) -> Result<Page, String> {
        let page = self
            .store
            .get_page(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Page was not found.".to_owned())?;
        if page.workspace_id != self.workspace.id || page.deleted_at.is_some() {
            return Err("Page is not available in this workspace.".into());
        }
        Ok(page)
    }

    pub fn open_sidebar_page_menu(&mut self, id: &str, point: PointerPress) {
        match self.live_workspace_page(id) {
            Ok(page) => {
                self.sidebar_page_name = TextEdit::new(&page.title);
                self.sidebar_page_menu = Some(SidebarPageMenu {
                    page,
                    point,
                    renaming: false,
                    anchor: None,
                    icon_anchor: format!("page-icon-{id}"),
                    error: None,
                });
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn open_sidebar_page_menu_at_anchor(&mut self, id: &str, anchor: &str, icon_anchor: &str) {
        self.open_sidebar_page_menu(id, PointerPress { x: 0., y: 0. });
        if let Some(menu) = &mut self.sidebar_page_menu {
            menu.anchor = Some(anchor.to_owned());
            menu.icon_anchor = icon_anchor.to_owned();
        }
    }

    pub fn propagate_page_icon(&mut self, page: Page) {
        for tab in &mut self.tabs {
            if tab.page.id == page.id {
                tab.page.icon = page.icon.clone();
                tab.page.updated_at = page.updated_at.clone();
            }
            for record in &tab.records {
                if record.page_id.as_deref() == Some(page.id.as_str()) {
                    tab.record_icons
                        .insert(record.id.clone(), page.icon.clone());
                }
            }
        }
        if let Some(peek) = &mut self.record_peek {
            if peek.page.id == page.id {
                peek.page.icon = page.icon.clone();
                peek.page.updated_at = page.updated_at.clone();
            }
        }
        for cached in &mut self.pages {
            if cached.id == page.id {
                cached.icon = page.icon.clone();
                cached.updated_at = page.updated_at.clone();
            }
        }
        for cached in &mut self.sidebar_favorites.pages {
            if cached.id == page.id {
                cached.icon = page.icon.clone();
                cached.updated_at = page.updated_at.clone();
            }
        }
        self.sidebar_tree.update_page(&page);
        self.sidebar_sections.update_page(&page);
        self.update_child_page(&page);
    }

    pub fn close_sidebar_page_menu(&mut self) {
        self.sidebar_page_menu = None;
    }

    // Save only affected tabs: unrelated selections and peeks stay in place.
    fn save_affected_tabs(&mut self, indices: &[usize]) -> bool {
        let active = self.active;
        for &index in indices {
            self.active = index;
            if self.tabs[index].dirty_title {
                self.save_title();
            }
            if self.tabs[index].dirty_blocks {
                self.save_blocks();
            }
            if self.tabs[index].dirty_title || self.tabs[index].dirty_blocks {
                self.active = active;
                self.sidebar_page_error("A page draft could not be saved.".into());
                return false;
            }
        }
        self.active = active;
        true
    }

    pub fn rename_sidebar_page(&mut self) -> bool {
        let Some(menu) = &self.sidebar_page_menu else {
            return false;
        };
        let id = menu.page.id.clone();
        let expected_title = menu.page.title.clone();
        let name = self.sidebar_page_name.text.clone();
        let page = match self.live_workspace_page(&id) {
            Ok(page) => page,
            Err(error) => {
                self.sidebar_page_error(error);
                return false;
            }
        };
        let indices: Vec<_> = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(i, tab)| (tab.page.id == id).then_some(i))
            .collect();
        if indices.iter().any(|&index| self.tabs[index].dirty_title) {
            self.sidebar_page_error("Save the open page title before renaming it.".into());
            return false;
        }
        if !self.save_affected_tabs(&indices) {
            return false;
        }
        if page.kind == PageKind::Record {
            self.sidebar_page_error("Rename this record from its database.".into());
            return false;
        }
        let result = self
            .store
            .rename_page_if_unchanged(&id, &expected_title, &name)
            .map_err(|error| error.to_string());
        match result {
            Ok(page) => {
                for tab in &mut self.tabs {
                    if tab.page.id == id {
                        tab.page = page.clone();
                        if tab.title.text != page.title {
                            tab.title.replace_text(&page.title);
                        }
                    }
                }
                if let Some(item) = self.pages.iter_mut().find(|p| p.id == id) {
                    *item = page.clone();
                }
                self.sidebar_tree.update_page(&page);
                self.sidebar_favorites.update_page(&page);
                self.sidebar_sections.update_page(&page);
                self.update_child_page(&page);
                self.sidebar_page_menu = None;
                true
            }
            Err(error) => {
                self.sidebar_page_error(format!("Page name is not saved: {error}"));
                false
            }
        }
    }

    fn is_descendant(&self, id: &str, root: &str) -> Result<bool, String> {
        let mut cursor = Some(id.to_owned());
        let mut seen = HashSet::new();
        while let Some(id) = cursor {
            if id == root {
                return Ok(true);
            }
            if !seen.insert(id.clone()) {
                return Err("Page hierarchy has a cycle.".into());
            }
            let page = self
                .store
                .get_page(&id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "A page in the hierarchy was not found.".to_owned())?;
            cursor = page.parent_id;
        }
        Ok(false)
    }

    pub fn trash_page_by_id(&mut self, id: &str) -> bool {
        let root = match self.live_workspace_page(id) {
            Ok(page) => page,
            Err(error) => {
                self.sidebar_page_error(error);
                return false;
            }
        };
        let mut affected = Vec::new();
        for (index, tab) in self.tabs.iter().enumerate() {
            match self.is_descendant(&tab.page.id, id) {
                Ok(true) => affected.push(index),
                Ok(false) => {}
                Err(error) => {
                    self.sidebar_page_error(error);
                    return false;
                }
            }
        }
        let peek_affected = if let Some(peek) = &self.record_peek {
            match self.is_descendant(&peek.page.id, id) {
                Ok(value) => value,
                Err(error) => {
                    self.sidebar_page_error(error);
                    return false;
                }
            }
        } else {
            false
        };
        if affected.contains(&self.active) {
            if self.database_property_editor.is_some() {
                self.sidebar_page_error(
                    "Close the property editor before moving this page to Trash.".into(),
                );
                return false;
            }
            self.dismiss_cell_picker();
            if self.cell_picker.is_some() || !self.finish_cell(false) {
                self.sidebar_page_error("A property draft could not be saved.".into());
                return false;
            }
        }
        if peek_affected && !self.close_peek() {
            self.sidebar_page_error("A record draft could not be saved.".into());
            return false;
        }
        if !self.save_affected_tabs(&affected) {
            return false;
        }
        if let Err(error) = self.store.delete_page(id) {
            self.sidebar_page_error(format!("Page was not moved to Trash: {error}"));
            return false;
        }
        let active_page = self.tab().map(|tab| tab.page.id.clone());
        let old_active = self.active;
        for index in affected.into_iter().rev() {
            self.tabs.remove(index);
        }
        self.active = active_page
            .as_ref()
            .and_then(|id| self.tabs.iter().position(|tab| &tab.page.id == id))
            .unwrap_or_else(|| old_active.min(self.tabs.len().saturating_sub(1)));
        if active_page
            .as_deref()
            .is_some_and(|page| self.tabs.iter().all(|tab| tab.page.id != page))
        {
            self.modal = None;
        }
        self.sidebar_page_menu = None;
        if let Some(parent) = root.parent_id.as_deref() {
            self.refresh_child_pages(parent);
        }
        self.refresh_pages();
        self.save_preferences();
        true
    }
}
