use crate::state::State;
use moss_core::{PageCursor, PagePosition};

pub const PAGE_SIZE: u32 = 50;

impl State {
    pub fn page_children_move(&mut self, cursor: PageCursor, forward: bool) {
        if !self.flush_current() {
            return;
        }
        let Some(tab) = self.tab() else { return };
        let parent = tab.page.id.clone();
        let position = if forward {
            PagePosition::After(&cursor)
        } else {
            PagePosition::Before(&cursor)
        };
        let result = self
            .store
            .page_children(&self.workspace.id, Some(&parent), position, PAGE_SIZE)
            .and_then(|pages| {
                if pages.items.is_empty() {
                    self.store
                        .page_children(
                            &self.workspace.id,
                            Some(&parent),
                            PagePosition::Start,
                            PAGE_SIZE,
                        )
                        .map(|pages| (pages, None))
                } else {
                    Ok((pages, Some((cursor, forward))))
                }
            });
        match result {
            Ok((pages, cursor)) => {
                self.tabs[self.active].child_pages = Some(pages);
                self.tabs[self.active].child_page_cursor = cursor;
            }
            Err(e) => self.error = Some(format!("Could not load child pages: {e}")),
        }
    }
    pub fn refresh_open_child_pages(&mut self) {
        self.refresh_page_links();
        let parents: Vec<_> = self
            .tabs
            .iter()
            .filter(|tab| {
                matches!(
                    tab.page.kind,
                    moss_core::PageKind::Document | moss_core::PageKind::Record
                )
            })
            .map(|tab| tab.page.id.clone())
            .collect();
        for parent in parents {
            self.refresh_child_pages(&parent);
        }
    }
    pub fn update_child_page(&mut self, page: &moss_core::Page) {
        for tab in &mut self.tabs {
            if tab.page_links.contains_key(&page.id) {
                tab.page_links.insert(page.id.clone(), page.clone());
            }
            if let Some(children) = &mut tab.child_pages {
                if let Some(child) = children
                    .items
                    .iter_mut()
                    .find(|child| child.page.id == page.id)
                {
                    child.page = page.clone();
                }
            }
        }
    }

    pub fn refresh_child_pages(&mut self, parent_id: &str) {
        let targets: Vec<_> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| {
                tab.page.id == parent_id
                    && matches!(
                        tab.page.kind,
                        moss_core::PageKind::Document | moss_core::PageKind::Record
                    )
            })
            .map(|(index, tab)| (index, tab.child_page_cursor.clone()))
            .collect();
        for (index, cursor) in targets {
            let position = match &cursor {
                None => PagePosition::Start,
                Some((cursor, true)) => PagePosition::After(cursor),
                Some((cursor, false)) => PagePosition::Before(cursor),
            };
            match self
                .store
                .page_children(&self.workspace.id, Some(parent_id), position, PAGE_SIZE)
            {
                Ok(mut pages) => {
                    if pages.items.is_empty() && cursor.is_some() {
                        match self.store.page_children(
                            &self.workspace.id,
                            Some(parent_id),
                            PagePosition::Start,
                            PAGE_SIZE,
                        ) {
                            Ok(first) => {
                                pages = first;
                                self.tabs[index].child_page_cursor = None;
                            }
                            Err(e) => {
                                self.error = Some(format!("Could not refresh child pages: {e}"));
                                continue;
                            }
                        }
                    }
                    self.tabs[index].child_pages = Some(pages);
                    if self.error.as_deref().is_some_and(|e| {
                        e.starts_with("Could not load child pages:")
                            || e.starts_with("Could not refresh child pages:")
                    }) {
                        self.error = None;
                    }
                }
                Err(e) => self.error = Some(format!("Could not refresh child pages: {e}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::{PageKind, new_id};
    use rune::{Env, Runtime};

    #[test]
    fn document_children_are_bounded_and_page_in_both_directions() {
        let path = std::env::temp_dir().join(format!("moss-children-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        let parent = s.tab().unwrap().page.id.clone();
        for i in 0..51 {
            s.store
                .create_page(
                    &s.workspace.id,
                    Some(&parent),
                    &format!("Child {i}"),
                    "",
                    PageKind::Document,
                )
                .unwrap();
        }
        s.refresh_child_pages(&parent);
        let first = s.tab().unwrap().child_pages.as_ref().unwrap();
        assert_eq!(first.items.len(), PAGE_SIZE as usize);
        let cursor = first.next.clone().unwrap();
        s.page_children_move(cursor, true);
        let last = s.tab().unwrap().child_pages.as_ref().unwrap();
        assert_eq!(last.items.len(), 1);
        assert!(last.next.is_none());
        let previous = last.previous.clone().unwrap();
        s.store
            .create_page(
                &s.workspace.id,
                Some(&parent),
                "Another child",
                "",
                PageKind::Document,
            )
            .unwrap();
        s.refresh_child_pages(&parent);
        assert_eq!(
            s.tab().unwrap().child_pages.as_ref().unwrap().items.len(),
            2
        );
        s.page_children_move(previous, false);
        assert_eq!(
            s.tab().unwrap().child_pages.as_ref().unwrap().items.len(),
            PAGE_SIZE as usize
        );
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn record_child_link_blocks_navigation_until_draft_saves() {
        let path = std::env::temp_dir().join(format!("moss-record-child-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        s.create_inline(Default::default(), false);
        s.finish_cell(false);
        let record_id = s.tab().unwrap().records[0].id.clone();
        let record_page = s.store.ensure_record_page(&record_id).unwrap();
        let child = s
            .store
            .create_page(
                &s.workspace.id,
                Some(&record_page.id),
                "Nested note",
                "",
                PageKind::Document,
            )
            .unwrap();
        s.open_page(&record_page.id, true);
        assert_eq!(
            s.tab().unwrap().child_pages.as_ref().unwrap().items[0]
                .page
                .id,
            child.id
        );
        s.tabs[s.active].blocks[0].block.page_id = "invalid".into();
        s.tabs[s.active].blocks[0]
            .edit
            .replace_text("Unsaved draft");
        s.save_blocks();
        assert!(s.tab().unwrap().dirty_blocks);
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (1440., 900.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.frame(0.);
        let key = format!("child-page-{}", child.id);
        let (x, y) = rt.find(&key).unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 120.);
        assert_eq!(rt.state.tab().unwrap().page.id, record_page.id);
        assert_eq!(rt.state.tab().unwrap().blocks[0].edit.text, "Unsaved draft");
        rt.update(|s, _| {
            s.tabs[s.active].blocks[0].block.page_id = record_page.id.clone();
            s.save_blocks();
        });
        rt.frame(1. / 120.);
        let (x, y) = rt.find(&key).unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 120.);
        assert_eq!(rt.state.tab().unwrap().page.id, child.id);
        assert_eq!(
            rt.state.store.get_blocks(&record_page.id).unwrap()[0].text,
            "Unsaved draft"
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn stale_next_retry_rename_and_focus_refresh_restore_children() {
        let path = std::env::temp_dir().join(format!("moss-children-cache-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        let parent = s.tab().unwrap().page.id.clone();
        let mut children = Vec::new();
        for n in 0..51 {
            children.push(
                s.store
                    .create_page(
                        &s.workspace.id,
                        Some(&parent),
                        &format!("Child {n}"),
                        "",
                        PageKind::Document,
                    )
                    .unwrap(),
            );
        }
        s.refresh_child_pages(&parent);
        let cursor = s
            .tab()
            .unwrap()
            .child_pages
            .as_ref()
            .unwrap()
            .next
            .clone()
            .unwrap();
        let last = s
            .store
            .page_children(
                &s.workspace.id,
                Some(&parent),
                PagePosition::After(&cursor),
                PAGE_SIZE,
            )
            .unwrap();
        for child in last.items {
            s.store.delete_page(&child.page.id).unwrap();
        }
        s.page_children_move(cursor, true);
        assert_eq!(
            s.tab().unwrap().child_pages.as_ref().unwrap().items.len(),
            50
        );
        assert!(s.tab().unwrap().child_page_cursor.is_none());
        let child = s.tab().unwrap().child_pages.as_ref().unwrap().items[0]
            .page
            .clone();
        s.open_page(&child.id, true);
        s.tabs[s.active].title.replace_text("Renamed");
        s.save_title();
        assert_eq!(
            s.tabs[0]
                .child_pages
                .as_ref()
                .unwrap()
                .items
                .iter()
                .find(|c| c.page.id == child.id)
                .unwrap()
                .page
                .title,
            "Renamed"
        );
        s.store.delete_page(&child.id).unwrap();
        s.refresh_open_child_pages();
        assert!(
            !s.tabs[0]
                .child_pages
                .as_ref()
                .unwrap()
                .items
                .iter()
                .any(|c| c.page.id == child.id)
        );
        s.tabs[0].child_pages = None;
        s.error = Some("Could not load child pages: test".into());
        s.refresh_child_pages(&parent);
        assert!(s.tabs[0].child_pages.is_some());
        assert!(s.error.is_none());
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn initial_child_load_failure_shows_retry() {
        let path = std::env::temp_dir().join(format!("moss-child-retry-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.tabs[s.active].child_pages = None;
        s.error = Some("Could not load child pages: test".into());
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (1440., 900.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.frame(0.);
        let (x, y) = rt.find("child-pages-retry-button").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 120.);
        assert!(rt.state.tab().unwrap().child_pages.is_some());
        assert!(rt.state.error.is_none());
        assert!(rt.find("child-pages-retry-button").is_none());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
