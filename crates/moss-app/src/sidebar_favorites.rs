use crate::state::State;
use moss_core::Page;
use std::collections::BTreeSet;

const MAX_FAVORITES: usize = 100;

#[derive(Default)]
pub struct SidebarFavorites {
    pub pages: Vec<Page>,
}

impl SidebarFavorites {
    pub fn update_page(&mut self, page: &Page) {
        if let Some(cached) = self.pages.iter_mut().find(|cached| cached.id == page.id) {
            *cached = page.clone();
        }
    }
}

impl State {
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
        self.sidebar_favorites.pages = pages;
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
