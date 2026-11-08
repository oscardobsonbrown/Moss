use crate::{emoji_catalog, state::State};
use rune::TextEdit;

pub struct PageIconPicker {
    pub page_id: String,
    pub expected_icon: String,
    pub anchor: String,
    pub selected: usize,
    pub category: Option<&'static str>,
    pub page: usize,
    pub error: Option<String>,
}
impl State {
    pub fn open_page_icon_picker(&mut self, page_id: &str, anchor: &str) {
        if self
            .page_icon_picker
            .as_ref()
            .is_some_and(|picker| picker.error.is_some())
        {
            return;
        }
        let page = match self.store.get_page(page_id) {
            Ok(Some(page))
                if page.workspace_id == self.workspace.id && page.deleted_at.is_none() =>
            {
                page
            }
            Ok(_) => {
                self.error = Some("Page was not found.".into());
                return;
            }
            Err(error) => {
                self.error = Some(format!("Page icon is not loaded: {error}"));
                return;
            }
        };
        self.page_icon_search = TextEdit::new("");
        self.page_icon_search.focused = true;
        self.sidebar_page_menu = None;
        self.page_icon_picker = Some(PageIconPicker {
            page_id: page.id,
            expected_icon: page.icon,
            anchor: anchor.into(),
            selected: 0,
            category: None,
            page: 0,
            error: None,
        });
    }
    pub fn dismiss_page_icon_picker(&mut self) {
        if self
            .page_icon_picker
            .as_ref()
            .is_some_and(|picker| picker.error.is_none())
        {
            self.page_icon_picker = None;
        }
    }
    pub fn reload_page_icon_picker(&mut self) {
        let Some(picker) = self.page_icon_picker.as_ref() else {
            return;
        };
        match self.store.get_page(&picker.page_id) {
            Ok(Some(page))
                if page.workspace_id == self.workspace.id && page.deleted_at.is_none() =>
            {
                let picker = self.page_icon_picker.as_mut().unwrap();
                picker.expected_icon = page.icon;
                picker.error = None;
            }
            Ok(_) => {
                self.page_icon_picker.as_mut().unwrap().error = Some("Page was not found.".into())
            }
            Err(error) => {
                self.page_icon_picker.as_mut().unwrap().error =
                    Some(format!("Page icon is not loaded: {error}"))
            }
        }
    }
    pub fn choose_page_icon(&mut self, icon: &str) {
        let Some(picker) = self.page_icon_picker.as_ref() else {
            return;
        };
        let result = self.store.set_page_icon_if_unchanged(
            &self.workspace.id,
            &picker.page_id,
            &picker.expected_icon,
            icon,
        );
        match result {
            Ok(page) => {
                if !icon.is_empty() {
                    self.recent_page_icons.retain(|item| item != icon);
                    self.recent_page_icons.insert(0, icon.into());
                    self.recent_page_icons.truncate(12);
                }
                self.page_icon_picker = None;
                self.propagate_page_icon(page);
            }
            Err(error) => {
                if let Some(picker) = &mut self.page_icon_picker {
                    picker.error = Some(format!("Page icon is not saved: {error}"));
                }
            }
        }
    }
    pub fn choose_random_page_icon(&mut self) {
        let all = emoji_catalog::matches("", None);
        if all.is_empty() {
            return;
        }
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;
        self.choose_page_icon(all[seed % all.len()].glyph);
    }
    pub fn submit_page_icon_search(&mut self) {
        let Some(picker) = self.page_icon_picker.as_ref() else {
            return;
        };
        let results = emoji_catalog::matches(&self.page_icon_search.text, picker.category);
        if let Some(item) = self.page_icon_picker.as_ref().and_then(|picker| {
            results.get(picker.page * emoji_catalog::PAGE_SIZE + picker.selected)
        }) {
            self.choose_page_icon(item.glyph);
        }
    }
}

#[cfg(test)]
#[path = "page_personalization_tests.rs"]
mod tests;
