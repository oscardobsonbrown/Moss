use crate::state::{Preferences, State};
use moss_core::{Page, new_id};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::{cell::Cell, rc::Rc};

const MAX_SECTIONS: usize = 32;
const MAX_CUSTOM_PAGES: usize = 100;
const MAX_RECENTS: usize = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarSectionKind {
    Recents,
    Favorites,
    Private,
    #[default]
    Custom,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SidebarSection {
    pub id: String,
    pub name: String,
    pub kind: SidebarSectionKind,
    pub page_ids: Vec<String>,
    pub collapsed: bool,
    pub hidden: bool,
}

pub struct SidebarSections {
    pub sections: Vec<SidebarSection>,
    pub pages: BTreeMap<String, Vec<Page>>,
    pub dragging: Option<String>,
    pub drag_target: Option<String>,
    pub page_drag: Option<SidebarPageDrag>,
    pub suppress_page_press: bool,
    pub page_drag_cancelled: bool,
    pub scroll: Rc<Cell<rune::ScrollPos>>,
    pub page_scroll_at: f64,
    pub viewport_rect: rune::Live<[f32; 4]>,
    pub row_rects: crate::state::RectSlots,
    pub visible_rows: std::ops::Range<usize>,
}

impl Default for SidebarSections {
    fn default() -> Self {
        Self {
            sections: Vec::new(),
            pages: BTreeMap::new(),
            dragging: None,
            drag_target: None,
            page_drag: None,
            suppress_page_press: false,
            page_drag_cancelled: false,
            scroll: Rc::new(Cell::new(rune::ScrollPos::default())),
            page_scroll_at: 0.,
            viewport_rect: rune::Live::new([0.; 4]),
            row_rects: crate::state::RectSlots::default(),
            visible_rows: 0..0,
        }
    }
}

pub struct SidebarPageDrag {
    pub page_id: String,
    pub source: String,
    pub target: Option<String>,
    pub before: Option<String>,
    pub parent: Option<String>,
    pub over: Option<String>,
    pub inside: bool,
    pub after: bool,
    pub title: String,
    pub icon: String,
    pub point: [f32; 2],
    pub viewport: [f32; 4],
}

impl SidebarSections {
    pub fn update_page(&mut self, page: &Page) {
        for pages in self.pages.values_mut() {
            if let Some(old) = pages.iter_mut().find(|old| old.id == page.id) {
                *old = page.clone();
            }
        }
    }
    pub fn update_record_title(&mut self, record: &moss_core::Record) {
        if let Some(page_id) = &record.page_id {
            for pages in self.pages.values_mut() {
                if let Some(page) = pages.iter_mut().find(|page| page.id == *page_id) {
                    page.title = record.title.clone();
                }
            }
        }
    }
}

fn defaults() -> Vec<SidebarSection> {
    [
        ("recents", "Recents", SidebarSectionKind::Recents),
        ("favorites", "Favorites", SidebarSectionKind::Favorites),
        ("private", "Private", SidebarSectionKind::Private),
    ]
    .into_iter()
    .map(|(id, name, kind)| SidebarSection {
        id: id.into(),
        name: name.into(),
        kind,
        ..Default::default()
    })
    .collect()
}

fn sections_for(prefs: &Preferences, workspace: &str) -> Vec<SidebarSection> {
    let builtins = defaults();
    let Some(saved) = prefs.sidebar_sections.get(workspace) else {
        return builtins;
    };
    let mut sections = Vec::new();
    let mut seen = BTreeSet::new();
    let mut custom_count = 0;
    for saved in saved.iter().take(MAX_SECTIONS * 4) {
        if saved.id.is_empty() || saved.id.len() > 128 || !seen.insert(saved.id.clone()) {
            continue;
        }
        if let Some(builtin) = builtins.iter().find(|builtin| builtin.id == saved.id) {
            let mut section = builtin.clone();
            section.collapsed = saved.collapsed;
            section.hidden = saved.hidden;
            sections.push(section);
        } else if saved.kind == SidebarSectionKind::Custom && custom_count < MAX_SECTIONS - 3 {
            let mut page_ids = BTreeSet::new();
            let page_ids = saved
                .page_ids
                .iter()
                .take(MAX_CUSTOM_PAGES)
                .filter(|id| !id.is_empty() && id.len() <= 128 && page_ids.insert((*id).clone()))
                .cloned()
                .collect();
            sections.push(SidebarSection {
                id: saved.id.clone(),
                name: valid_name(&saved.name).unwrap_or_else(|| "Untitled section".into()),
                kind: SidebarSectionKind::Custom,
                page_ids,
                collapsed: saved.collapsed,
                hidden: saved.hidden,
            });
            custom_count += 1;
        }
    }
    for required in builtins {
        if !sections.iter().any(|section| section.id == required.id) {
            sections.push(required);
        }
    }
    sections
}

fn write_preferences(path: &std::path::Path, prefs: &Preferences) -> Result<(), String> {
    let bytes = serde_json::to_vec(prefs).map_err(|error| error.to_string())?;
    std::fs::write(path.join("preferences.json.tmp"), bytes).map_err(|error| error.to_string())?;
    std::fs::rename(
        path.join("preferences.json.tmp"),
        path.join("preferences.json"),
    )
    .map_err(|error| error.to_string())
}

impl State {
    fn section_pages(&self, ids: &[String], limit: usize) -> Result<Vec<Page>, String> {
        let mut pages = Vec::new();
        let mut seen = BTreeSet::new();
        for id in ids.iter().take(MAX_CUSTOM_PAGES) {
            if !seen.insert(id) {
                continue;
            }
            match self.store.get_page(id).map_err(|e| e.to_string())? {
                Some(page)
                    if page.workspace_id == self.workspace.id && page.deleted_at.is_none() =>
                {
                    pages.push(page)
                }
                _ => {}
            }
            if pages.len() == limit {
                break;
            }
        }
        Ok(pages)
    }

    fn load_sidebar_sections(
        &self,
        sections: Vec<SidebarSection>,
        recent: &[String],
    ) -> Result<SidebarSections, String> {
        let mut pages = BTreeMap::new();
        for section in &sections {
            let list = match section.kind {
                SidebarSectionKind::Recents => Some(self.section_pages(recent, MAX_RECENTS)?),
                SidebarSectionKind::Custom => {
                    Some(self.section_pages(&section.page_ids, MAX_CUSTOM_PAGES)?)
                }
                _ => None,
            };
            if let Some(list) = list {
                pages.insert(section.id.clone(), list);
            }
        }
        Ok(SidebarSections {
            sections,
            pages,
            ..Default::default()
        })
    }

    pub fn refresh_sidebar_sections(&mut self) -> bool {
        let prefs = self.shared_preferences.lock().unwrap().clone();
        let sections = sections_for(&prefs, &self.workspace.id);
        let recent = prefs
            .recent_pages
            .get(&self.workspace.id)
            .cloned()
            .unwrap_or_default();
        let valid = match self.section_pages(&recent, MAX_RECENTS) {
            Ok(pages) => pages.into_iter().map(|page| page.id).collect::<Vec<_>>(),
            Err(error) => {
                self.error = Some(format!("Sidebar sections could not be loaded: {error}"));
                return false;
            }
        };
        let invalid_sections = prefs
            .sidebar_sections
            .get(&self.workspace.id)
            .is_some_and(|saved| saved != &sections);
        if recent != valid || invalid_sections {
            return self.change_sidebar_sections(|_, _, _| Ok(()));
        }
        match self.load_sidebar_sections(sections, &valid) {
            Ok(mut cache) => {
                cache.dragging = self.sidebar_sections.dragging.take();
                cache.drag_target = self.sidebar_sections.drag_target.take();
                cache.page_drag = self.sidebar_sections.page_drag.take();
                cache.suppress_page_press = self.sidebar_sections.suppress_page_press;
                cache.page_drag_cancelled = self.sidebar_sections.page_drag_cancelled;
                cache.scroll = self.sidebar_sections.scroll.clone();
                cache.page_scroll_at = self.sidebar_sections.page_scroll_at;
                cache.viewport_rect = self.sidebar_sections.viewport_rect.clone();
                cache.row_rects = self.sidebar_sections.row_rects.clone();
                cache.visible_rows = self.sidebar_sections.visible_rows.clone();
                self.sidebar_sections = cache;
                self.close_missing_sidebar_section_menu();
                true
            }
            Err(error) => {
                self.error = Some(format!("Sidebar sections could not be loaded: {error}"));
                false
            }
        }
    }

    fn change_sidebar_sections(
        &mut self,
        change: impl FnOnce(
            &mut Vec<SidebarSection>,
            &mut Vec<String>,
            &mut Vec<String>,
        ) -> Result<(), String>,
    ) -> bool {
        let owner = self.shared_preferences.clone();
        let mut shared = owner.lock().unwrap();
        let mut next = shared.clone();
        let mut sections = sections_for(&next, &self.workspace.id);
        let mut recent = next
            .recent_pages
            .get(&self.workspace.id)
            .cloned()
            .unwrap_or_default();
        let mut favorites = next
            .favorite_pages
            .get(&self.workspace.id)
            .cloned()
            .unwrap_or_default();
        let old_favorites = favorites.clone();
        if let Err(error) = change(&mut sections, &mut recent, &mut favorites) {
            self.error = Some(error);
            return false;
        }
        let valid_recent = match self.section_pages(&recent, MAX_RECENTS) {
            Ok(pages) => pages.into_iter().map(|p| p.id).collect::<Vec<_>>(),
            Err(error) => {
                self.error = Some(format!("Sidebar sections could not be saved: {error}"));
                return false;
            }
        };
        let mut cache = match self.load_sidebar_sections(sections.clone(), &valid_recent) {
            Ok(cache) => cache,
            Err(error) => {
                self.error = Some(format!("Sidebar sections could not be saved: {error}"));
                return false;
            }
        };
        let favorite_pages = if favorites != old_favorites {
            match self.section_pages(&favorites, MAX_CUSTOM_PAGES) {
                Ok(pages) => Some(pages),
                Err(error) => {
                    self.error = Some(format!("Favorites could not be loaded: {error}"));
                    return false;
                }
            }
        } else {
            None
        };
        next.favorite_pages
            .insert(self.workspace.id.clone(), favorites);
        next.sidebar_sections
            .insert(self.workspace.id.clone(), sections);
        next.recent_pages
            .insert(self.workspace.id.clone(), valid_recent);
        let result = write_preferences(&self.path, &next);
        if let Err(error) = result {
            self.error = Some(format!("Sidebar sections could not be saved: {error}"));
            return false;
        }
        *shared = next.clone();
        self.prefs = next;
        cache.dragging = self.sidebar_sections.dragging.take();
        cache.drag_target = self.sidebar_sections.drag_target.take();
        cache.page_drag = self.sidebar_sections.page_drag.take();
        cache.suppress_page_press = self.sidebar_sections.suppress_page_press;
        cache.page_drag_cancelled = self.sidebar_sections.page_drag_cancelled;
        cache.scroll = self.sidebar_sections.scroll.clone();
        cache.page_scroll_at = self.sidebar_sections.page_scroll_at;
        cache.viewport_rect = self.sidebar_sections.viewport_rect.clone();
        cache.row_rects = self.sidebar_sections.row_rects.clone();
        cache.visible_rows = self.sidebar_sections.visible_rows.clone();
        self.sidebar_sections = cache;
        if let Some(pages) = favorite_pages {
            self.sidebar_favorites.pages = pages;
        }
        self.close_missing_sidebar_section_menu();
        self.error = None;
        true
    }

    pub fn create_sidebar_section(&mut self, name: &str) -> Option<String> {
        let Some(name) = valid_name(name) else {
            self.error = Some("Enter a section name.".into());
            return None;
        };
        let id = format!("custom-{}", new_id());
        self.change_sidebar_sections(|sections, _, _| {
            if sections.len() >= MAX_SECTIONS {
                return Err("Sidebar sections are full.".into());
            }
            sections.push(SidebarSection {
                id: id.clone(),
                name,
                kind: SidebarSectionKind::Custom,
                ..Default::default()
            });
            Ok(())
        })
        .then_some(id)
    }
    pub fn rename_sidebar_section(&mut self, id: &str, name: &str) -> bool {
        let Some(name) = valid_name(name) else {
            self.error = Some("Enter a section name.".into());
            return false;
        };
        self.change_sidebar_sections(|sections, _, _| {
            let section = custom(sections, id)?;
            section.name = name;
            Ok(())
        })
    }
    pub fn remove_sidebar_section(&mut self, id: &str) -> bool {
        self.change_sidebar_sections(|sections, _, _| {
            custom(sections, id)?;
            sections.retain(|section| section.id != id);
            Ok(())
        })
    }
    pub fn move_sidebar_section(&mut self, id: &str, offset: isize) -> bool {
        self.change_sidebar_sections(|sections, _, _| {
            let visible: Vec<_> = sections
                .iter()
                .enumerate()
                .filter_map(|(i, section)| (!section.hidden).then_some(i))
                .collect();
            let from = visible
                .iter()
                .position(|&i| sections[i].id == id)
                .ok_or("Section not found.")?;
            let to = from.saturating_add_signed(offset).min(visible.len() - 1);
            sections.swap(visible[from], visible[to]);
            Ok(())
        })
    }
    pub fn reorder_sidebar_section(&mut self, id: &str, before_id: &str) -> bool {
        self.change_sidebar_sections(|sections, _, _| {
            if id == before_id {
                return if sections.iter().any(|s| s.id == id) {
                    Ok(())
                } else {
                    Err("Section not found.".into())
                };
            }
            let from = sections
                .iter()
                .position(|s| s.id == id)
                .ok_or("Section not found.")?;
            if before_id.is_empty() {
                let section = sections.remove(from);
                sections.push(section);
                return Ok(());
            }
            if !sections.iter().any(|s| s.id == before_id) {
                return Err("Section not found.".into());
            }
            let section = sections.remove(from);
            let to = sections.iter().position(|s| s.id == before_id).unwrap();
            sections.insert(to, section);
            Ok(())
        })
    }
    pub fn toggle_sidebar_section(&mut self, id: &str) -> bool {
        self.change_sidebar_sections(|sections, _, _| {
            let section = sections
                .iter_mut()
                .find(|s| s.id == id)
                .ok_or("Section not found.")?;
            section.collapsed = !section.collapsed;
            Ok(())
        })
    }
    pub fn set_sidebar_section_hidden(&mut self, id: &str, hidden: bool) -> bool {
        self.change_sidebar_sections(|sections, _, _| {
            let section = sections
                .iter_mut()
                .find(|section| section.id == id)
                .ok_or("Section not found.")?;
            section.hidden = hidden;
            Ok(())
        })
    }
    pub fn add_page_to_sidebar_section(&mut self, section: &str, id: &str) -> bool {
        match self.section_pages(&[id.to_owned()], 1) {
            Ok(pages) if !pages.is_empty() => {}
            Ok(_) => {
                self.error = Some("Page is not available in this workspace.".into());
                return false;
            }
            Err(error) => {
                self.error = Some(error);
                return false;
            }
        }
        self.change_sidebar_sections(|sections, _, _| {
            let section = custom(sections, section)?;
            if section.page_ids.iter().any(|existing| existing == id) {
                return Ok(());
            }
            if section.page_ids.len() >= MAX_CUSTOM_PAGES {
                return Err("Section is full.".into());
            }
            section.page_ids.push(id.into());
            Ok(())
        })
    }
    pub fn remove_page_from_sidebar_section(&mut self, section: &str, id: &str) -> bool {
        self.change_sidebar_sections(|sections, _, _| {
            custom(sections, section)?
                .page_ids
                .retain(|page| page != id);
            Ok(())
        })
    }

    pub fn move_sidebar_page(
        &mut self,
        id: &str,
        source: &str,
        target: &str,
        before: Option<&str>,
    ) -> bool {
        if source == "private" && target == "private" {
            let visible = {
                let prefs = self.shared_preferences.lock().unwrap();
                sections_for(&prefs, &self.workspace.id)
                    .iter()
                    .any(|section| section.id == "private" && !section.hidden)
            };
            if !visible {
                self.error = Some("The section is hidden.".into());
                return false;
            }
            return match self
                .store
                .reorder_sidebar_page(&self.workspace.id, id, before)
            {
                Ok(()) => {
                    self.refresh_sidebar_tree();
                    true
                }
                Err(error) => {
                    self.error = Some(format!("Could not reorder page: {error}"));
                    false
                }
            };
        }
        if !self
            .section_pages(&[id.to_owned()], 1)
            .is_ok_and(|pages| !pages.is_empty())
        {
            self.error = Some("Page is not available in this workspace.".into());
            return false;
        }
        self.change_sidebar_sections(|sections, _, favorites| {
            let from = sections
                .iter()
                .position(|s| s.id == source)
                .ok_or("Source section not found.")?;
            let to = sections
                .iter()
                .position(|s| s.id == target)
                .ok_or("Target section not found.")?;
            let from_kind = sections[from].kind;
            let to_kind = sections[to].kind;
            if sections[from].hidden || sections[to].hidden {
                return Err("The source or target section is hidden.".into());
            }
            if from_kind == SidebarSectionKind::Recents || to_kind == SidebarSectionKind::Recents {
                return Err("Recent pages cannot be moved.".into());
            }
            if before == Some(id) && source == target {
                return Ok(());
            }
            match from_kind {
                SidebarSectionKind::Custom => {
                    if !sections[from].page_ids.iter().any(|page| page == id) {
                        return Err("Page is no longer in the source section.".into());
                    }
                    sections[from].page_ids.retain(|page| page != id);
                }
                SidebarSectionKind::Favorites => {
                    if !favorites.iter().any(|page| page == id) {
                        return Err("Page is no longer in Favorites.".into());
                    }
                    favorites.retain(|page| page != id);
                }
                SidebarSectionKind::Private => {}
                SidebarSectionKind::Recents => unreachable!(),
            }
            let pages = match to_kind {
                SidebarSectionKind::Custom => Some(&mut sections[to].page_ids),
                SidebarSectionKind::Favorites => Some(favorites),
                SidebarSectionKind::Private => None,
                SidebarSectionKind::Recents => unreachable!(),
            };
            if let Some(pages) = pages {
                let existing = pages.iter().position(|page| page == id);
                if existing.is_none() && pages.len() >= MAX_CUSTOM_PAGES {
                    return Err("Target section is full.".into());
                }
                if let Some(index) = existing {
                    pages.remove(index);
                }
                let index = before
                    .and_then(|page| pages.iter().position(|item| item == page))
                    .unwrap_or(pages.len());
                pages.insert(index, id.into());
            }
            sections[to].collapsed = false;
            Ok(())
        })
    }
    pub fn record_sidebar_recent(&mut self, id: &str) {
        self.record_sidebar_recent_inner(None, id, false);
    }
    pub fn record_sidebar_recent_deferred(&mut self, page: Option<&Page>, id: &str) {
        self.record_sidebar_recent_inner(page, id, true);
    }
    fn record_sidebar_recent_inner(&mut self, provided: Option<&Page>, id: &str, defer: bool) {
        let page = if let Some(page) =
            provided.filter(|p| p.workspace_id == self.workspace.id && p.deleted_at.is_none())
        {
            page.clone()
        } else {
            match self.store.get_page(id) {
                Ok(Some(page))
                    if page.workspace_id == self.workspace.id && page.deleted_at.is_none() =>
                {
                    page
                }
                Ok(_) => return,
                Err(error) => {
                    self.error = Some(format!("Recent pages could not be loaded: {error}"));
                    return;
                }
            }
        };
        let owner = self.shared_preferences.clone();
        let mut shared = owner.lock().unwrap();
        let previous = shared
            .recent_pages
            .get(&self.workspace.id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let cached_recents = self.sidebar_sections.pages.get("recents");
        let mut pages = vec![page];
        let mut seen = BTreeSet::from([id.to_owned()]);
        for old in previous.iter().take(MAX_RECENTS) {
            if !seen.insert(old.clone()) {
                continue;
            }
            if defer
                && let Some(cached) = cached_recents
                    .and_then(|list| list.iter().find(|p| p.id == *old))
                    .filter(|p| p.workspace_id == self.workspace.id && p.deleted_at.is_none())
            {
                pages.push(cached.clone());
            } else {
                match self.store.get_page(old) {
                    Ok(Some(page))
                        if page.workspace_id == self.workspace.id && page.deleted_at.is_none() =>
                    {
                        pages.push(page)
                    }
                    Ok(_) => {}
                    Err(error) => {
                        self.error = Some(format!("Recent pages could not be loaded: {error}"));
                        return;
                    }
                }
            }
            if pages.len() == MAX_RECENTS {
                break;
            }
        }
        let ids: Vec<_> = pages.iter().map(|page| page.id.clone()).collect();
        if previous != ids.as_slice() {
            if defer {
                shared
                    .recent_pages
                    .insert(self.workspace.id.clone(), ids.clone());
                self.prefs
                    .recent_pages
                    .insert(self.workspace.id.clone(), ids);
            } else {
                let mut next = shared.clone();
                next.recent_pages.insert(self.workspace.id.clone(), ids);
                if let Err(error) = write_preferences(&self.path, &next) {
                    self.error = Some(format!("Recent pages could not be saved: {error}"));
                    return;
                }
                *shared = next.clone();
                self.prefs = next;
            }
        }
        self.sidebar_sections.pages.insert("recents".into(), pages);
    }
}

fn valid_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control))
        .then(|| name.to_owned())
}

pub fn poll(s: &mut State, time: f64, tx: &mut rune::Tx) -> bool {
    let prefs = s.poll_preferences(time, tx);
    let Some(drag) = &s.sidebar_sections.page_drag else {
        return prefs;
    };
    let [x, y] = drag.point;
    let [left, top, width, height] = drag.viewport;
    if x < left || x > left + width || y < top || y > top + height {
        return prefs;
    }
    let delta = if y < top + 28. {
        -12.
    } else if y > top + height - 28. {
        12.
    } else {
        0.
    };
    if delta == 0. {
        return prefs;
    }
    tx.wake_after(std::time::Duration::from_millis(16));
    if time - s.sidebar_sections.page_scroll_at < 1. / 60. {
        return prefs;
    }
    s.sidebar_sections.page_scroll_at = time;
    let scroll = s.sidebar_sections.scroll.get();
    let next = (scroll.offset[1] + delta).clamp(0., scroll.max[1]);
    if next == scroll.offset[1] {
        return prefs;
    }
    tx.scroll_to_instant("sidebar-scroll", 0., next);
    true
}
fn custom<'a>(
    sections: &'a mut [SidebarSection],
    id: &str,
) -> Result<&'a mut SidebarSection, String> {
    sections
        .iter_mut()
        .find(|section| section.id == id && section.kind == SidebarSectionKind::Custom)
        .ok_or_else(|| "Custom section not found.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::PageKind;
    use std::path::PathBuf;

    fn fixture() -> (State, PathBuf) {
        let path = std::env::temp_dir().join(format!("moss-sections-{}", new_id()));
        (State::open(path.clone(), None).unwrap(), path)
    }
    fn page(s: &mut State, parent: Option<&str>, name: &str) -> Page {
        s.store
            .create_page(&s.workspace.id, parent, name, "", PageKind::Document)
            .unwrap()
    }

    #[test]
    fn old_preferences_default_sections_and_restoring_tabs_adds_no_recent() {
        let old: Preferences =
            serde_json::from_str(r#"{"workspace":"","tabs":{},"active":{}}"#).unwrap();
        assert!(old.sidebar_sections.is_empty() && old.recent_pages.is_empty());
        let (mut s, path) = fixture();
        assert_eq!(
            s.sidebar_sections
                .sections
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["recents", "favorites", "private"]
        );
        let second = page(&mut s, None, "Second");
        s.open_page(&second.id, true);
        let recent = s.prefs.recent_pages[&s.workspace.id].clone();
        s.save_preferences();
        drop(s);
        let reopened = State::open(path.clone(), None).unwrap();
        assert_eq!(reopened.prefs.recent_pages[&reopened.workspace.id], recent);
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn recents_are_valid_unique_bounded_and_history_updates_them() {
        let (mut s, path) = fixture();
        let pages: Vec<_> = (0..5)
            .map(|i| page(&mut s, None, &format!("Page {i}")))
            .collect();
        for page in &pages {
            s.open_page(&page.id, false);
        }
        let recent = &s.prefs.recent_pages[&s.workspace.id];
        assert_eq!(
            recent,
            &pages[1..]
                .iter()
                .rev()
                .map(|p| p.id.clone())
                .collect::<Vec<_>>()
        );
        s.open_page(&pages[3].id, false);
        assert_eq!(s.prefs.recent_pages[&s.workspace.id][0], pages[3].id);
        s.navigate_history(false);
        assert_eq!(s.prefs.recent_pages[&s.workspace.id][0], pages[4].id);
        assert_eq!(s.prefs.recent_pages[&s.workspace.id].len(), 4);
        s.store.delete_page(&pages[3].id).unwrap();
        s.record_sidebar_recent(&pages[2].id);
        assert!(!s.prefs.recent_pages[&s.workspace.id].contains(&pages[3].id));
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn custom_sections_keep_shortcuts_and_workspace_isolation() {
        let (mut s, path) = fixture();
        let parent = page(&mut s, None, "Parent");
        let child = page(&mut s, Some(&parent.id), "Child");
        assert!(s.toggle_sidebar_favorite(&child.id));
        let id = s.create_sidebar_section("Projects").unwrap();
        assert!(s.add_page_to_sidebar_section(&id, &child.id));
        assert!(s.add_page_to_sidebar_section(&id, &child.id));
        assert_eq!(s.sidebar_sections.pages[&id].len(), 1);
        assert!(s.is_sidebar_favorite(&child.id));
        assert_eq!(
            s.store.get_page(&child.id).unwrap().unwrap().parent_id,
            Some(parent.id)
        );
        assert!(s.rename_sidebar_section(&id, "Work"));
        assert!(s.reorder_sidebar_section(&id, "recents"));
        assert_eq!(s.sidebar_sections.sections[0].id, id);
        assert!(s.move_sidebar_section(&id, 1));
        assert!(s.reorder_sidebar_section(&id, ""));
        assert_eq!(s.sidebar_sections.sections.last().unwrap().id, id);
        assert!(s.toggle_sidebar_section(&id));
        assert!(s.sidebar_sections.sections.last().unwrap().collapsed);
        assert!(!s.remove_sidebar_section("private"));
        let other = s.store.create_workspace("Other", "").unwrap();
        let mut other_state = State::open(path.clone(), Some(&other.id)).unwrap();
        other_state.shared_preferences = s.shared_preferences.clone();
        other_state.refresh_sidebar_sections();
        assert!(
            other_state
                .sidebar_sections
                .sections
                .iter()
                .all(|section| section.id != id)
        );
        assert!(!other_state.add_page_to_sidebar_section("private", &child.id));
        assert!(s.remove_page_from_sidebar_section(&id, &child.id));
        assert!(s.sidebar_sections.pages[&id].is_empty());
        assert!(s.remove_sidebar_section(&id));
        drop(other_state);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn failed_write_does_not_publish_section_or_recent_changes() {
        let (mut s, path) = fixture();
        let target = page(&mut s, None, "Target");
        let before = s.shared_preferences.lock().unwrap().clone();
        let names = s.sidebar_sections.sections.clone();
        s.path = path.join("missing-directory");
        assert!(s.create_sidebar_section("Not saved").is_none());
        s.record_sidebar_recent(&target.id);
        let after = s.shared_preferences.lock().unwrap().clone();
        assert_eq!(before.sidebar_sections, after.sidebar_sections);
        assert_eq!(before.recent_pages, after.recent_pages);
        assert_eq!(s.sidebar_sections.sections, names);
        assert!(s.error.is_some());
        s.path = path.clone();
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn failed_page_move_preserves_source_and_favorites() {
        let (mut s, path) = fixture();
        let item = page(&mut s, None, "Item");
        let section = s.create_sidebar_section("Projects").unwrap();
        assert!(s.add_page_to_sidebar_section(&section, &item.id));
        let before = s.shared_preferences.lock().unwrap().clone();
        s.path = path.join("missing-directory");
        assert!(!s.move_sidebar_page(&item.id, &section, "favorites", None));
        let after = s.shared_preferences.lock().unwrap().clone();
        assert_eq!(before.sidebar_sections, after.sidebar_sections);
        assert_eq!(before.favorite_pages, after.favorite_pages);
        assert_eq!(s.sidebar_sections.pages[&section][0].id, item.id);
        assert!(!s.is_sidebar_favorite(&item.id));
        s.path = path.clone();
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn failed_visibility_save_does_not_hide_a_section() {
        let (mut s, path) = fixture();
        let before = s
            .shared_preferences
            .lock()
            .unwrap()
            .sidebar_sections
            .clone();
        s.path = path.join("missing-directory");
        assert!(!s.set_sidebar_section_hidden("private", true));
        assert!(
            !s.sidebar_sections
                .sections
                .iter()
                .find(|section| section.id == "private")
                .unwrap()
                .hidden
        );
        assert_eq!(
            s.shared_preferences.lock().unwrap().sidebar_sections,
            before
        );
        s.path = path.clone();
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn section_visibility_is_workspace_scoped_and_keeps_shortcuts() {
        let (mut s, path) = fixture();
        let item = page(&mut s, None, "Item");
        let custom = s.create_sidebar_section("Projects").unwrap();
        assert!(s.add_page_to_sidebar_section(&custom, &item.id));
        assert!(s.set_sidebar_section_hidden(&custom, true));
        assert!(s.set_sidebar_section_hidden("favorites", true));
        assert!(s.move_sidebar_section("private", -1));
        let visible: Vec<_> = s
            .sidebar_sections
            .sections
            .iter()
            .filter(|section| !section.hidden)
            .map(|section| section.id.as_str())
            .collect();
        assert_eq!(visible, ["private", "recents"]);
        assert!(!s.move_sidebar_page(&item.id, "private", &custom, None));
        assert_eq!(s.sidebar_sections.pages[&custom][0].id, item.id);
        let other = s.store.create_workspace("Other", "").unwrap();
        let mut other_state = State::open(path.clone(), Some(&other.id)).unwrap();
        other_state.shared_preferences = s.shared_preferences.clone();
        assert!(other_state.refresh_sidebar_sections());
        assert!(
            other_state
                .sidebar_sections
                .sections
                .iter()
                .all(|section| !section.hidden)
        );
        let workspace = s.workspace.id.clone();
        drop(other_state);
        drop(s);
        let reopened = State::open(path.clone(), Some(&workspace)).unwrap();
        let saved = reopened
            .sidebar_sections
            .sections
            .iter()
            .find(|section| section.id == custom)
            .unwrap();
        assert!(saved.hidden && saved.page_ids.contains(&item.id));
        assert!(reopened.store.get_page(&item.id).unwrap().is_some());
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn record_peek_adds_recent_only_on_open_and_updates_renamed_label() {
        let (mut s, path) = fixture();
        s.new_page(true, None);
        s.create_inline(Default::default(), false);
        s.finish_cell(false);
        let record = s.tab().unwrap().records[0].clone();
        let before = s.prefs.recent_pages[&s.workspace.id].clone();
        s.query_records();
        assert_eq!(s.prefs.recent_pages[&s.workspace.id], before);
        s.open_record(0);
        let page_id = s.record_peek.as_ref().unwrap().page.id.clone();
        assert_eq!(s.prefs.recent_pages[&s.workspace.id][0], page_id);
        s.record_peek
            .as_mut()
            .unwrap()
            .title
            .replace_text("Renamed record");
        s.save_peek_title();
        assert_eq!(
            s.sidebar_sections.pages["recents"][0].title,
            "Renamed record"
        );
        assert_eq!(
            s.store.get_record(&record.id).unwrap().unwrap().title,
            "Renamed record"
        );
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn section_and_shortcut_limits_reject_extra_items_without_changes() {
        let (mut s, path) = fixture();
        let custom = s.create_sidebar_section("Projects").unwrap();
        for i in 0..(MAX_SECTIONS - 4) {
            assert!(s.create_sidebar_section(&format!("Section {i}")).is_some());
        }
        assert_eq!(s.sidebar_sections.sections.len(), MAX_SECTIONS);
        assert!(s.create_sidebar_section("Overflow").is_none());
        let ids: Vec<String> = (0..=MAX_CUSTOM_PAGES)
            .map(|i| page(&mut s, None, &format!("Page {i}")).id)
            .collect();
        assert!(s.change_sidebar_sections(|sections, _, _| {
            custom_section_for_test(sections, &custom).page_ids = ids[..MAX_CUSTOM_PAGES].to_vec();
            Ok(())
        }));
        assert!(!s.add_page_to_sidebar_section(&custom, &ids[MAX_CUSTOM_PAGES]));
        assert!(s.toggle_sidebar_favorite(&ids[MAX_CUSTOM_PAGES]));
        assert!(!s.move_sidebar_page(&ids[MAX_CUSTOM_PAGES], "favorites", &custom, None));
        assert!(s.is_sidebar_favorite(&ids[MAX_CUSTOM_PAGES]));
        assert_eq!(s.sidebar_sections.pages[&custom].len(), MAX_CUSTOM_PAGES);
        assert!(
            !s.sidebar_sections.pages[&custom]
                .iter()
                .any(|p| p.id == ids[MAX_CUSTOM_PAGES])
        );
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn malformed_sections_keep_canonical_builtins_and_bound_shortcuts() {
        let (mut s, path) = fixture();
        let mut raw = vec![SidebarSection {
            id: "private".into(),
            name: "Wrong".into(),
            kind: SidebarSectionKind::Custom,
            page_ids: vec!["ignored".into()],
            collapsed: true,
            hidden: false,
        }];
        for i in 0..45 {
            raw.push(SidebarSection {
                id: format!("custom-{i}"),
                name: format!("Section {i}"),
                kind: SidebarSectionKind::Custom,
                page_ids: (0..150).map(|n| format!("page-{n}")).collect(),
                collapsed: false,
                hidden: false,
            });
        }
        s.shared_preferences
            .lock()
            .unwrap()
            .sidebar_sections
            .insert(s.workspace.id.clone(), raw);
        assert!(s.refresh_sidebar_sections());
        let sections = &s.sidebar_sections.sections;
        assert_eq!(sections.len(), MAX_SECTIONS);
        for builtin in defaults() {
            let actual = sections
                .iter()
                .find(|section| section.id == builtin.id)
                .unwrap();
            assert_eq!(actual.kind, builtin.kind);
            assert_eq!(actual.name, builtin.name);
            assert!(actual.page_ids.is_empty());
        }
        assert!(
            sections
                .iter()
                .find(|section| section.id == "private")
                .unwrap()
                .collapsed
        );
        assert!(
            sections
                .iter()
                .all(|section| section.page_ids.len() <= MAX_CUSTOM_PAGES)
        );
        assert_eq!(s.prefs.sidebar_sections[&s.workspace.id], *sections);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn recording_visit_does_not_reload_custom_section_cache() {
        let (mut s, path) = fixture();
        let shortcut = page(&mut s, None, "Shortcut");
        let visit = page(&mut s, None, "Visit");
        let section = s.create_sidebar_section("Projects").unwrap();
        assert!(s.add_page_to_sidebar_section(&section, &shortcut.id));
        s.sidebar_sections.pages.get_mut(&section).unwrap()[0].title = "Cache marker".into();
        s.record_sidebar_recent(&visit.id);
        assert_eq!(s.sidebar_sections.pages[&section][0].title, "Cache marker");
        assert_eq!(s.sidebar_sections.pages["recents"][0].id, visit.id);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    fn custom_section_for_test<'a>(
        sections: &'a mut [SidebarSection],
        id: &str,
    ) -> &'a mut SidebarSection {
        sections
            .iter_mut()
            .find(|section| section.id == id)
            .unwrap()
    }
}
