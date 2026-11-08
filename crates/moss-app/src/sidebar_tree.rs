use std::collections::{BTreeMap, BTreeSet};

use moss_core::{PageChildren, PageCursor, PagePosition};

use crate::state::State;

const PAGE_SIZE: u32 = 50;
const RESTORE_GROUP_LIMIT: usize = 4;

#[derive(Default)]
pub struct SidebarTree {
    groups: BTreeMap<Option<String>, PageChildren>,
    positions: BTreeMap<Option<String>, (PageCursor, bool)>,
    pub rows: Vec<SidebarRow>,
}

pub enum SidebarRow {
    Page {
        page: moss_core::Page,
        has_children: bool,
        expanded: bool,
        depth: usize,
    },
    Paging {
        parent: Option<String>,
        previous: Option<PageCursor>,
        next: Option<PageCursor>,
        depth: usize,
    },
}

impl SidebarTree {
    pub fn first_root(&self) -> Option<&str> {
        self.groups
            .get(&None)?
            .items
            .first()
            .map(|child| child.page.id.as_str())
    }

    pub fn update_page(&mut self, page: &moss_core::Page) {
        for group in self.groups.values_mut() {
            if let Some(child) = group
                .items
                .iter_mut()
                .find(|child| child.page.id == page.id)
            {
                child.page = page.clone();
            }
        }
        self.rows = self.build_rows();
    }

    fn build_rows(&self) -> Vec<SidebarRow> {
        enum Task<'a> {
            Group(Option<String>, usize),
            Page(&'a moss_core::PageChild, usize),
            Paging(Option<String>, &'a PageChildren, usize),
        }
        let mut rows = Vec::new();
        let mut stack = vec![Task::Group(None, 0)];
        while let Some(task) = stack.pop() {
            match task {
                Task::Group(parent, depth) => {
                    let Some(group) = self.groups.get(&parent) else {
                        continue;
                    };
                    if group.previous.is_some() || group.next.is_some() {
                        stack.push(Task::Paging(parent, group, depth));
                    }
                    for child in group.items.iter().rev() {
                        stack.push(Task::Page(child, depth));
                    }
                }
                Task::Page(child, depth) => {
                    let key = Some(child.page.id.clone());
                    let expanded = self.groups.contains_key(&key);
                    rows.push(SidebarRow::Page {
                        page: child.page.clone(),
                        has_children: child.has_children,
                        expanded,
                        depth,
                    });
                    if expanded {
                        stack.push(Task::Group(key, depth + 1));
                    }
                }
                Task::Paging(parent, group, depth) => rows.push(SidebarRow::Paging {
                    parent,
                    previous: group.previous.clone(),
                    next: group.next.clone(),
                    depth,
                }),
            }
        }
        rows
    }

    fn remove_descendants(&mut self, parent: &Option<String>) {
        let mut pending = vec![parent.clone()];
        while let Some(key) = pending.pop() {
            self.positions.remove(&key);
            if let Some(group) = self.groups.remove(&key) {
                pending.extend(group.items.into_iter().map(|c| Some(c.page.id)));
            }
        }
    }
}

impl State {
    pub fn refresh_sidebar_tree(&mut self) {
        let saved: BTreeSet<String> = self
            .prefs
            .expanded_pages
            .get(&self.workspace.id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let restoring = self.sidebar_tree.groups.is_empty();
        let mut tree = SidebarTree::default();
        let mut pending = vec![None];
        while let Some(parent) = pending.pop() {
            let anchor = self.sidebar_tree.positions.get(&parent);
            let position = match anchor {
                Some((cursor, true)) => PagePosition::After(cursor),
                Some((cursor, false)) => PagePosition::Before(cursor),
                None => PagePosition::Start,
            };
            let mut group = match self.store.page_children(
                &self.workspace.id,
                parent.as_deref(),
                position,
                PAGE_SIZE,
            ) {
                Ok(group) => group,
                Err(e) => {
                    self.error = Some(e.to_string());
                    return;
                }
            };
            let reset_anchor = group.items.is_empty() && anchor.is_some();
            if reset_anchor {
                group = match self.store.page_children(
                    &self.workspace.id,
                    parent.as_deref(),
                    PagePosition::Start,
                    PAGE_SIZE,
                ) {
                    Ok(group) => group,
                    Err(e) => {
                        self.error = Some(e.to_string());
                        return;
                    }
                };
            }
            for child in &group.items {
                let key = Some(child.page.id.clone());
                if child.has_children
                    && if restoring {
                        saved.contains(&child.page.id)
                            && pending.len() + tree.groups.len() < RESTORE_GROUP_LIMIT
                    } else {
                        self.sidebar_tree.groups.contains_key(&key)
                    }
                {
                    pending.push(key);
                }
            }
            if !reset_anchor {
                if let Some(anchor) = anchor {
                    tree.positions.insert(parent.clone(), anchor.clone());
                }
            }
            tree.groups.insert(parent, group);
        }
        tree.rows = tree.build_rows();
        self.sidebar_tree = tree;
    }

    pub fn toggle_sidebar_page(&mut self, id: &str) {
        let key = Some(id.to_owned());
        let opening = !self.sidebar_tree.groups.contains_key(&key);
        if opening {
            match self.store.page_children(
                &self.workspace.id,
                Some(id),
                PagePosition::Start,
                PAGE_SIZE,
            ) {
                Ok(group) => {
                    self.sidebar_tree.groups.insert(key, group);
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    return;
                }
            }
        } else {
            self.sidebar_tree.remove_descendants(&key);
        }
        self.sidebar_tree.rows = self.sidebar_tree.build_rows();
        {
            let mut prefs = self.shared_preferences.lock().unwrap();
            let ids = prefs
                .expanded_pages
                .entry(self.workspace.id.clone())
                .or_default();
            if opening {
                if !ids.iter().any(|v| v == id) {
                    ids.push(id.to_owned());
                }
            } else {
                ids.retain(|v| v != id);
            }
        }
        self.save_preferences();
    }

    pub fn page_sidebar_group(
        &mut self,
        parent: Option<String>,
        cursor: PageCursor,
        forward: bool,
    ) {
        let position = if forward {
            PagePosition::After(&cursor)
        } else {
            PagePosition::Before(&cursor)
        };
        let result = self
            .store
            .page_children(&self.workspace.id, parent.as_deref(), position, PAGE_SIZE)
            .and_then(|group| {
                if group.items.is_empty() {
                    self.store
                        .page_children(
                            &self.workspace.id,
                            parent.as_deref(),
                            PagePosition::Start,
                            PAGE_SIZE,
                        )
                        .map(|group| (group, false))
                } else {
                    Ok((group, true))
                }
            });
        match result {
            Ok((group, keep_anchor)) => {
                self.sidebar_tree.remove_descendants(&parent);
                self.sidebar_tree.groups.insert(parent.clone(), group);
                if keep_anchor {
                    self.sidebar_tree
                        .positions
                        .insert(parent, (cursor, forward));
                }
                self.sidebar_tree.rows = self.sidebar_tree.build_rows();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::{PageKind, new_id};

    #[test]
    fn sibling_paging_expansion_and_reload_keep_pages_reachable() {
        let path = std::env::temp_dir().join(format!("moss-sidebar-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        let workspace = s.workspace.id.clone();
        let parent = s
            .store
            .create_page(&workspace, None, "Parent", "📄", PageKind::Document)
            .unwrap();
        let child = s
            .store
            .create_page(
                &workspace,
                Some(&parent.id),
                "Child",
                "📄",
                PageKind::Document,
            )
            .unwrap();
        for n in 0..55 {
            s.store
                .create_page(
                    &workspace,
                    None,
                    &format!("Root {n}"),
                    "📄",
                    PageKind::Document,
                )
                .unwrap();
        }
        s.refresh_pages();
        assert!(s.pages.len() <= 30);
        assert!(s.sidebar_tree.rows.len() <= 51);
        let next = s
            .sidebar_tree
            .rows
            .iter()
            .find_map(|row| match row {
                SidebarRow::Paging {
                    parent: None, next, ..
                } => next.clone(),
                _ => None,
            })
            .unwrap();
        s.page_sidebar_group(None, next, true);
        assert!(s.sidebar_tree.rows.iter().any(|row| matches!(
            row,
            SidebarRow::Paging {
                previous: Some(_),
                ..
            }
        )));
        let previous = s
            .sidebar_tree
            .rows
            .iter()
            .find_map(|row| match row {
                SidebarRow::Paging {
                    parent: None,
                    previous,
                    ..
                } => previous.clone(),
                _ => None,
            })
            .unwrap();
        s.refresh_pages();
        assert!(s.sidebar_tree.rows.iter().any(|row| matches!(
            row,
            SidebarRow::Paging {
                previous: Some(_),
                ..
            }
        )));
        s.page_sidebar_group(None, previous, false);
        s.toggle_sidebar_page(&parent.id);
        assert!(s.sidebar_tree.rows.iter().any(
            |row| matches!(row, SidebarRow::Page { page, depth: 1, .. } if page.id == child.id)
        ));
        drop(s);
        let reopened = State::open(path.clone(), None).unwrap();
        assert!(reopened.sidebar_tree.rows.iter().any(
            |row| matches!(row, SidebarRow::Page { page, depth: 1, .. } if page.id == child.id)
        ));
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn disclosure_expands_without_opening_the_page() {
        let path = std::env::temp_dir().join(format!("moss-sidebar-ui-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        let parent = s
            .store
            .create_page(&s.workspace.id, None, "Folder", "📄", PageKind::Document)
            .unwrap();
        let child = s
            .store
            .create_page(
                &s.workspace.id,
                Some(&parent.id),
                "Nested",
                "📄",
                PageKind::Document,
            )
            .unwrap();
        s.refresh_pages();
        let active = s.tab().unwrap().page.id.clone();
        let mut rt = rune::Runtime::new(
            s,
            crate::ui::view,
            (1440., 900.),
            rune::Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.frame(0.);
        let (x, y) = rt
            .find(&format!("page-disclosure-{}", parent.id))
            .expect("disclosure");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 120.);
        assert_eq!(rt.state.tab().unwrap().page.id, active);
        assert!(rt.find(&format!("page-open-{}", child.id)).is_some());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn deleted_last_sibling_page_falls_back_to_first_page() {
        let path = std::env::temp_dir().join(format!("moss-sidebar-delete-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        for n in 0..52 {
            s.store
                .create_page(
                    &s.workspace.id,
                    None,
                    &format!("Page {n}"),
                    "📄",
                    PageKind::Document,
                )
                .unwrap();
        }
        s.refresh_pages();
        let cursor = s
            .sidebar_tree
            .rows
            .iter()
            .find_map(|row| match row {
                SidebarRow::Paging { next, .. } => next.clone(),
                _ => None,
            })
            .unwrap();
        s.page_sidebar_group(None, cursor, true);
        let ids: Vec<_> = s
            .sidebar_tree
            .rows
            .iter()
            .filter_map(|row| match row {
                SidebarRow::Page { page, .. } => Some(page.id.clone()),
                _ => None,
            })
            .collect();
        for id in ids {
            s.store.delete_page(&id).unwrap();
        }
        s.refresh_pages();
        assert_eq!(
            s.sidebar_tree
                .rows
                .iter()
                .filter(|row| matches!(row, SidebarRow::Page { .. }))
                .count(),
            50
        );
        assert!(!s.sidebar_tree.positions.contains_key(&None));
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
}
