use crate::state::State;
use moss_core::Page;
use rune::TextEdit;
use std::collections::BTreeSet;

#[derive(Clone)]
pub enum SectionMenuMode {
    Actions(String),
    Create,
    Rename(String),
    AddPage(String),
    ChooseSection(String),
}

pub struct SidebarSectionMenu {
    pub anchor: String,
    pub mode: SectionMenuMode,
    pub pages: Vec<Page>,
    pub error: Option<String>,
}

impl State {
    pub fn close_missing_sidebar_section_menu(&mut self) {
        let missing = self.sidebar_section_menu.as_ref().is_some_and(|menu| {
            let id = match &menu.mode {
                SectionMenuMode::Actions(id)
                | SectionMenuMode::Rename(id)
                | SectionMenuMode::AddPage(id) => id,
                _ => return false,
            };
            !self
                .sidebar_sections
                .sections
                .iter()
                .any(|section| &section.id == id)
        });
        if missing {
            self.sidebar_section_menu = None;
            self.sidebar_section_name.focused = false;
            self.sidebar_section_search.focused = false;
        }
    }
    pub fn open_sidebar_section_menu(&mut self, mode: SectionMenuMode, anchor: &str) {
        let title = match &mode {
            SectionMenuMode::Rename(id) => self
                .sidebar_sections
                .sections
                .iter()
                .find(|section| &section.id == id)
                .map(|section| section.name.as_str())
                .unwrap_or(""),
            _ => "",
        };
        self.sidebar_section_name = TextEdit::new(title);
        self.sidebar_section_name.focused =
            matches!(mode, SectionMenuMode::Create | SectionMenuMode::Rename(_));
        self.sidebar_section_name
            .set_selection(0, self.sidebar_section_name.text.len());
        self.sidebar_section_search = TextEdit::new("");
        self.sidebar_section_search.focused = matches!(mode, SectionMenuMode::AddPage(_));
        self.sidebar_section_menu = Some(SidebarSectionMenu {
            anchor: anchor.into(),
            mode,
            pages: Vec::new(),
            error: None,
        });
        self.sidebar_page_menu = None;
        self.run_sidebar_section_search();
    }

    pub fn save_sidebar_section_name(&mut self) {
        let Some(menu) = &self.sidebar_section_menu else {
            return;
        };
        let name = self.sidebar_section_name.text.clone();
        let saved = match menu.mode.clone() {
            SectionMenuMode::Create => self.create_sidebar_section(&name).is_some(),
            SectionMenuMode::Rename(id) => self.rename_sidebar_section(&id, &name),
            _ => false,
        };
        if saved {
            self.sidebar_section_menu = None;
        } else {
            let error = self
                .error
                .take()
                .unwrap_or_else(|| "Section name could not be saved.".into());
            if let Some(menu) = &mut self.sidebar_section_menu {
                menu.error = Some(error);
            }
        }
    }

    pub fn run_sidebar_section_search(&mut self) {
        let Some(menu) = &self.sidebar_section_menu else {
            return;
        };
        if !matches!(menu.mode, SectionMenuMode::AddPage(_)) {
            return;
        }
        let query = self.sidebar_section_search.text.trim();
        let ids = if query.is_empty() {
            self.sidebar_sections
                .pages
                .get("recents")
                .into_iter()
                .flatten()
                .chain(self.pages.iter())
                .map(|page| page.id.clone())
                .take(30)
                .collect::<Vec<_>>()
        } else {
            match self.store.search(query, Some(&self.workspace.id), 30) {
                Ok(hits) => hits.into_iter().map(|hit| hit.page_id).collect(),
                Err(error) => {
                    self.sidebar_section_menu.as_mut().unwrap().error =
                        Some(format!("Pages could not be loaded: {error}"));
                    return;
                }
            }
        };
        let mut pages = Vec::new();
        let mut seen = BTreeSet::new();
        for id in ids {
            if !seen.insert(id.clone()) {
                continue;
            }
            match self.store.get_page(&id) {
                Ok(Some(page))
                    if page.workspace_id == self.workspace.id && page.deleted_at.is_none() =>
                {
                    pages.push(page)
                }
                Ok(_) => {}
                Err(error) => {
                    self.sidebar_section_menu.as_mut().unwrap().error =
                        Some(format!("Page could not be loaded: {error}"));
                    return;
                }
            }
        }
        let menu = self.sidebar_section_menu.as_mut().unwrap();
        menu.pages = pages;
        menu.error = None;
    }

    pub fn toggle_sidebar_section_shortcut(&mut self, section: &str, page: &str) {
        let contains = self
            .sidebar_sections
            .sections
            .iter()
            .find(|item| item.id == section)
            .is_some_and(|item| item.page_ids.iter().any(|id| id == page));
        let saved = if contains {
            self.remove_page_from_sidebar_section(section, page)
        } else {
            self.add_page_to_sidebar_section(section, page)
        };
        if !saved {
            let error = self
                .error
                .take()
                .unwrap_or_else(|| "Section could not be saved.".into());
            if let Some(menu) = &mut self.sidebar_section_menu {
                menu.error = Some(error);
            }
        }
    }
}
