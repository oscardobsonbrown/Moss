use crate::state::{EditableBlock, State, Tab};
use chrono::Datelike;
use moss_core::*;
use rune::TextEdit;
use std::collections::BTreeMap;

impl State {
    pub fn open_page(&mut self, id: &str, new_tab: bool) {
        if !self.loading && !self.flush_current() {
            return;
        }
        self.modal = None;
        if let Some(i) = self.tabs.iter().position(|t| t.page.id == id) {
            self.active = i;
            self.filter = TextEdit::new(&self.tabs[i].title_search);
            self.record_navigation(id);
            self.save_preferences();
            return;
        }
        let result = (|| -> Result<Tab, Box<dyn std::error::Error>> {
            let page = self.store.get_page(id)?.ok_or("Page was not found")?;
            let mut blocks = self.store.get_blocks(id)?;
            let choosing_type = blocks.is_empty() && page.kind == PageKind::Document;
            if blocks.is_empty() {
                blocks.push(Block::paragraph(id, "", 0));
            }
            let child_pages = if matches!(page.kind, PageKind::Document | PageKind::Record) {
                match self.store.page_children(
                    &self.workspace.id,
                    Some(id),
                    PagePosition::Start,
                    crate::child_pages::PAGE_SIZE,
                ) {
                    Ok(children) => Some(children),
                    Err(e) => {
                        self.error = Some(format!("Could not load child pages: {e}"));
                        None
                    }
                }
            } else {
                None
            };
            let database = self.store.get_database_by_page(id)?;
            let record_context = if page.kind == PageKind::Record {
                match self.store.get_record_by_page(id)? {
                    Some(record) => self
                        .store
                        .get_database(&record.database_id)?
                        .map(|db| (db, record)),
                    None => None,
                }
            } else {
                None
            };
            let selected_view = database
                .as_ref()
                .and_then(|db| {
                    self.prefs
                        .selected_views
                        .get(&self.workspace.id)
                        .and_then(|views| views.get(id))
                        .and_then(|view_id| db.views.iter().position(|view| &view.id == view_id))
                })
                .unwrap_or(0);
            let limit = database
                .as_ref()
                .and_then(|db| db.views.get(selected_view))
                .map(|view| crate::table_window::initial_limit(view, "", 0))
                .unwrap_or(100) as usize;
            let mut records = if let Some(db) = &database {
                let mut query = db
                    .views
                    .get(selected_view)
                    .map(|view| view.query.clone())
                    .unwrap_or_default();
                query.offset = 0;
                query.limit = limit as u32 + 1;
                self.store.query_records(&db.id, &query)?
            } else {
                vec![]
            };
            let has_more_records = records.len() > limit;
            records.truncate(limit);
            let cursor_query = database
                .as_ref()
                .and_then(|db| db.views.get(selected_view))
                .map(|view| &view.query);
            let paging_next = database.as_ref().zip(cursor_query).and_then(|(db, query)| {
                records.last().and_then(|record| {
                    self.store
                        .record_cursor(&db.id, query, record)
                        .ok()
                        .flatten()
                })
            });
            let paging_previous = database.as_ref().zip(cursor_query).and_then(|(db, query)| {
                records.first().and_then(|record| {
                    self.store
                        .record_cursor(&db.id, query, record)
                        .ok()
                        .flatten()
                })
            });
            let (record_icons, record_has_content) = if database.is_some() {
                let ids: Vec<_> = records.iter().map(|r| r.id.clone()).collect();
                match self.store.record_page_appearance(&self.workspace.id, &ids) {
                    Ok(appearance) => crate::records::split_record_appearance(appearance),
                    Err(e) => {
                        self.error = Some(format!("Could not load page icons: {e}"));
                        (BTreeMap::new(), BTreeMap::new())
                    }
                }
            } else {
                (BTreeMap::new(), BTreeMap::new())
            };
            let gallery_previews = if database
                .as_ref()
                .and_then(|db| db.views.get(selected_view))
                .is_some_and(|view| view.kind == ViewKind::Gallery)
            {
                let ids: Vec<_> = records.iter().map(|r| r.id.clone()).collect();
                match self.store.record_page_previews(&self.workspace.id, &ids) {
                    Ok(previews) => previews,
                    Err(e) => {
                        self.error = Some(format!("Could not load Gallery previews: {e}"));
                        BTreeMap::new()
                    }
                }
            } else {
                BTreeMap::new()
            };
            let gallery_covers = if database
                .as_ref()
                .and_then(|db| db.views.get(selected_view))
                .is_some_and(|view| view.kind == ViewKind::Gallery)
            {
                let ids: Vec<_> = records.iter().map(|r| r.id.clone()).collect();
                match self.store.record_page_covers(&self.workspace.id, &ids) {
                    Ok(covers) => covers,
                    Err(e) => {
                        self.error = Some(format!("Could not load Gallery covers: {e}"));
                        BTreeMap::new()
                    }
                }
            } else {
                BTreeMap::new()
            };
            let relation_titles = match &database {
                Some(db) => match crate::relation_titles::load(&self.store, db, &records) {
                    Ok(titles) => titles,
                    Err(e) => {
                        self.error = Some(format!("Could not load related titles: {e}"));
                        BTreeMap::new()
                    }
                },
                None => BTreeMap::new(),
            };
            let calendar_month = database
                .as_ref()
                .and_then(|db| db.views.get(selected_view))
                .filter(|view| view.kind == ViewKind::Calendar)
                .and_then(|view| {
                    self.prefs
                        .calendar_positions
                        .get(&self.workspace.id)
                        .and_then(|positions| positions.get(&view.id))
                })
                .and_then(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
                .unwrap_or_else(|| {
                    let today = chrono::Local::now().date_naive();
                    if database
                        .as_ref()
                        .and_then(|db| db.views.get(selected_view))
                        .is_some_and(crate::calendar_view::week)
                    {
                        today
                    } else {
                        today.with_day(1).unwrap()
                    }
                });
            let mut tab = Tab {
                choosing_type,
                title: TextEdit::new(&page.title),
                page,
                blocks: blocks
                    .iter()
                    .cloned()
                    .map(|block| {
                        let edit = crate::rich_text::load(&block);
                        EditableBlock { block, edit }
                    })
                    .collect(),
                page_links: crate::page_blocks::load_links(
                    &self.store,
                    &self.workspace.id,
                    &blocks,
                )?,
                child_pages,
                child_page_cursor: None,
                database,
                record_context,
                records,
                table_window: None,
                created_record_ids: Vec::new(),
                has_more_records,
                gallery_previews,
                gallery_covers,
                record_icons,
                record_has_content,
                relation_titles,
                summary_values: BTreeMap::new(),
                summary_key: String::new(),
                summary_epoch: 0,
                title_search: String::new(),
                view: selected_view,
                offset: 0,
                paging_next,
                paging_previous,
                paging_request: None,
                calendar_month,
                timeline_start: chrono::Local::now().date_naive(),
                dirty_title: false,
                dirty_blocks: false,
            };
            if crate::table_window::supported(&tab) {
                tab.table_window = Some(crate::table_window::TableWindow::new(tab.records.len()));
            }
            Ok(tab)
        })();
        match result {
            Ok(tab) => {
                if new_tab || self.tabs.is_empty() {
                    self.tabs.push(tab);
                    self.active = self.tabs.len() - 1;
                } else {
                    self.tabs[self.active] = tab;
                }
                self.filter = TextEdit::new("");
                self.record_navigation(id);
                self.refresh_table_summaries();
                self.save_preferences();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
}
