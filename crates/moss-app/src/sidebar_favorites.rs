use crate::state::State;
use moss_core::{Page, PageChild, PagePosition, Store};
use std::collections::{BTreeMap, BTreeSet};

const MAX_FAVORITES: usize = 100;
const CHILD_PAGE_SIZE: u32 = 50;

#[derive(Clone, Debug)]
pub struct SidebarShortcutRow {
    pub page: Page,
    pub has_children: bool,
    pub expanded: bool,
    pub depth: usize,
    pub unique_suffix: Option<String>,
}

#[derive(Default)]
pub struct SidebarFavorites {
    pub pages: Vec<Page>,
    pub rows: Vec<SidebarShortcutRow>,
    has_children: BTreeSet<String>,
    expanded: BTreeMap<String, Vec<PageChild>>,
}

pub(crate) fn build_shortcut_rows(
    roots: &[Page],
    has_children: &BTreeSet<String>,
    expanded: &BTreeMap<String, Vec<PageChild>>,
) -> Vec<SidebarShortcutRow> {
    let mut rows = Vec::new();
    let mut seen_keys = BTreeSet::new();
    for root in roots {
        let mut ancestors = BTreeSet::new();
        push_shortcut_row(
            root,
            has_children.contains(&root.id),
            0,
            None,
            expanded,
            &mut ancestors,
            &mut seen_keys,
            &mut rows,
        );
    }
    rows
}

#[allow(clippy::too_many_arguments)]
fn push_shortcut_row(
    page: &Page,
    has_children: bool,
    depth: usize,
    parent_id: Option<&str>,
    expanded: &BTreeMap<String, Vec<PageChild>>,
    ancestors: &mut BTreeSet<String>,
    seen_keys: &mut BTreeSet<String>,
    rows: &mut Vec<SidebarShortcutRow>,
) {
    let is_expanded = has_children && expanded.contains_key(&page.id);
    let unique_suffix = if seen_keys.insert(page.id.clone()) {
        None
    } else {
        Some(format!(
            "sub-{}-{}",
            parent_id.unwrap_or("root"),
            rows.len()
        ))
    };
    rows.push(SidebarShortcutRow {
        page: page.clone(),
        has_children,
        expanded: is_expanded,
        depth,
        unique_suffix,
    });
    if is_expanded
        && ancestors.insert(page.id.clone())
        && let Some(children) = expanded.get(&page.id)
    {
        for child in children {
            push_shortcut_row(
                &child.page,
                child.has_children,
                depth + 1,
                Some(&page.id),
                expanded,
                ancestors,
                seen_keys,
                rows,
            );
        }
        ancestors.remove(&page.id);
    }
}

pub(crate) fn refresh_shortcut_tree(
    roots: &[Page],
    has_children: &mut BTreeSet<String>,
    expanded: &mut BTreeMap<String, Vec<PageChild>>,
    store: &Store,
    workspace_id: &str,
) -> Vec<SidebarShortcutRow> {
    let root_ids: Vec<String> = roots.iter().map(|page| page.id.clone()).collect();
    *has_children = store
        .parents_with_children(workspace_id, &root_ids)
        .unwrap_or_default();
    let mut next_expanded = BTreeMap::new();
    let mut pending: Vec<String> = roots
        .iter()
        .filter(|page| has_children.contains(&page.id) && expanded.contains_key(&page.id))
        .map(|page| page.id.clone())
        .collect();
    let mut visited = BTreeSet::new();
    while let Some(parent_id) = pending.pop() {
        if !visited.insert(parent_id.clone()) {
            continue;
        }
        if let Ok(group) = store.page_children(
            workspace_id,
            Some(&parent_id),
            PagePosition::Start,
            CHILD_PAGE_SIZE,
        ) {
            for child in &group.items {
                if child.has_children && expanded.contains_key(&child.page.id) {
                    pending.push(child.page.id.clone());
                }
            }
            next_expanded.insert(parent_id, group.items);
        }
    }
    *expanded = next_expanded;
    build_shortcut_rows(roots, has_children, expanded)
}

impl SidebarFavorites {
    pub fn set_pages(&mut self, pages: Vec<Page>, store: &Store, workspace_id: &str) {
        self.pages = pages;
        self.rows = refresh_shortcut_tree(
            &self.pages,
            &mut self.has_children,
            &mut self.expanded,
            store,
            workspace_id,
        );
    }

    pub fn update_page(&mut self, page: &Page) {
        let mut changed = false;
        for cached in &mut self.pages {
            if cached.id == page.id {
                *cached = page.clone();
                changed = true;
            }
        }
        for children in self.expanded.values_mut() {
            for child in children {
                if child.page.id == page.id {
                    child.page = page.clone();
                    changed = true;
                }
            }
        }
        if changed {
            self.rows = build_shortcut_rows(&self.pages, &self.has_children, &self.expanded);
        }
    }
}

impl State {
    pub fn toggle_sidebar_favorite_page(&mut self, id: &str) {
        if self.sidebar_favorites.expanded.remove(id).is_some() {
            self.sidebar_favorites.rows = build_shortcut_rows(
                &self.sidebar_favorites.pages,
                &self.sidebar_favorites.has_children,
                &self.sidebar_favorites.expanded,
            );
            return;
        }
        match self.store.page_children(
            &self.workspace.id,
            Some(id),
            PagePosition::Start,
            CHILD_PAGE_SIZE,
        ) {
            Ok(group) => {
                if !group.items.is_empty() {
                    self.sidebar_favorites.has_children.insert(id.to_owned());
                    self.sidebar_favorites
                        .expanded
                        .insert(id.to_owned(), group.items);
                    self.sidebar_favorites.rows = build_shortcut_rows(
                        &self.sidebar_favorites.pages,
                        &self.sidebar_favorites.has_children,
                        &self.sidebar_favorites.expanded,
                    );
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn is_sidebar_favorite(&self, id: &str) -> bool {
        self.shared_preferences
            .lock()
            .unwrap()
            .favorite_pages
            .get(&self.workspace.id)
            .is_some_and(|ids| ids.iter().any(|saved| saved == id))
    }

    pub fn refresh_sidebar_favorites(&mut self) -> bool {
        let ids = self
            .shared_preferences
            .lock()
            .unwrap()
            .favorite_pages
            .get(&self.workspace.id)
            .cloned()
            .unwrap_or_default();
        let mut pages = Vec::new();
        let mut seen = BTreeSet::new();
        for id in ids.iter().take(MAX_FAVORITES) {
            if !seen.insert(id) {
                continue;
            }
            match self.store.get_page(id) {
                Ok(Some(page))
                    if page.workspace_id == self.workspace.id && page.deleted_at.is_none() =>
                {
                    pages.push(page)
                }
                Ok(_) => {}
                Err(error) => {
                    self.error = Some(format!("Favorites could not be loaded: {error}"));
                    return false;
                }
            }
        }
        self.sidebar_favorites
            .set_pages(pages, &self.store, &self.workspace.id);
        true
    }

    pub fn toggle_sidebar_favorite(&mut self, id: &str) -> bool {
        let owner = self.shared_preferences.clone();
        let mut shared = owner.lock().unwrap();
        let mut next = shared.clone();
        let ids = next
            .favorite_pages
            .entry(self.workspace.id.clone())
            .or_default();
        if ids.iter().any(|saved| saved == id) {
            ids.retain(|saved| saved != id);
        } else {
            match self.store.get_page(id) {
                Ok(Some(page))
                    if page.workspace_id == self.workspace.id && page.deleted_at.is_none() => {}
                Ok(_) => {
                    self.error = Some("This page is not available in this workspace.".into());
                    return false;
                }
                Err(error) => {
                    self.error = Some(format!("Favorite could not be saved: {error}"));
                    return false;
                }
            }
            if ids.len() >= MAX_FAVORITES {
                self.error = Some("Favorites is full. Remove a page before adding another.".into());
                return false;
            }
            ids.push(id.to_owned());
        }
        let saved = serde_json::to_vec(&next)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                std::fs::write(self.path.join("preferences.json.tmp"), bytes)
                    .map_err(|error| error.to_string())
            })
            .and_then(|_| {
                std::fs::rename(
                    self.path.join("preferences.json.tmp"),
                    self.path.join("preferences.json"),
                )
                .map_err(|error| error.to_string())
            });
        if let Err(error) = saved {
            self.error = Some(format!("Favorite could not be saved: {error}"));
            return false;
        }
        *shared = next.clone();
        self.prefs = next;
        drop(shared);
        self.error = None;
        self.refresh_sidebar_favorites()
    }
}

#[cfg(test)]
#[path = "sidebar_favorites_tests.rs"]
mod tests;
