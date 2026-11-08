use crate::state::State;
use moss_core::*;
use rune::TextEdit;
use serde_json::json;

impl State {
    pub fn begin_page(&mut self, parent: Option<String>) {
        if !self.flush_current() {
            return;
        }
        match self.store.create_page(
            &self.workspace.id,
            parent.as_deref(),
            "",
            "▤",
            PageKind::Document,
        ) {
            Ok(page) => {
                self.error = None;
                if let Some(parent) = page.parent_id.as_deref() {
                    self.refresh_child_pages(parent);
                }
                self.refresh_pages();
                self.open_page(&page.id, true);
                if let Some(tab) = self
                    .tabs
                    .get_mut(self.active)
                    .filter(|tab| tab.page.id == page.id)
                {
                    tab.title.focused = true;
                }
            }
            Err(e) => self.error = Some(format!("Could not create a page: {e}")),
        }
    }

    pub fn choose_page_type(&mut self, database: bool) {
        self.choose_page_layout(database.then_some(ViewKind::Table));
    }
    pub fn choose_dashboard_page(&mut self) {
        self.choose_page_layout(Some(ViewKind::Dashboard));
    }
    fn choose_page_layout(&mut self, kind: Option<ViewKind>) {
        if !self.tab().is_some_and(|tab| tab.choosing_type) || !self.flush_current() {
            return;
        }
        if kind.is_none() {
            let page_id = self.tabs[self.active].page.id.clone();
            let blocks = match self.store.get_blocks(&page_id) {
                Ok(blocks) => blocks,
                Err(e) => {
                    self.error = Some(format!("Could not open the page: {e}"));
                    return;
                }
            };
            let tab = &mut self.tabs[self.active];
            if !blocks.is_empty() {
                tab.blocks = blocks
                    .into_iter()
                    .map(|block| {
                        let edit = crate::rich_text::load(&block);
                        crate::state::EditableBlock { block, edit }
                    })
                    .collect();
            }
            tab.choosing_type = false;
            tab.title.focused = false;
            tab.blocks[0].edit.focused = true;
            self.error = None;
            return;
        }
        let tab = &self.tabs[self.active];
        let mut definition = database_definition(&tab.title.text);
        if kind == Some(ViewKind::Dashboard) {
            definition.views.truncate(1);
            definition.views[0].kind = ViewKind::Dashboard;
            definition.views[0].name = "Dashboard".into();
            definition.views[0].config = json!({});
        }
        match self
            .store
            .create_database_on_page(&tab.page.id, definition.clone())
        {
            Ok(page) => {
                definition.page_id = page.id.clone();
                definition.name = if page.title.is_empty() {
                    "Untitled database".into()
                } else {
                    page.title.clone()
                };
                let tab = &mut self.tabs[self.active];
                tab.title = TextEdit::new(&page.title);
                tab.page = page;
                tab.database = Some(definition);
                tab.blocks.clear();
                tab.child_pages = None;
                tab.choosing_type = false;
                self.error = None;
                self.refresh_pages();
                self.save_preferences();
            }
            Err(e) => {
                self.error = Some(format!(
                    "Could not create the database. Your page is still saved: {e}"
                ))
            }
        }
    }
    #[cfg(test)]
    pub fn new_page(&mut self, database: bool, parent: Option<String>) {
        if !self.flush_current() {
            return;
        }
        let name = if self.new_name.text.trim().is_empty() {
            if database {
                "Untitled database"
            } else {
                "Untitled"
            }
        } else {
            self.new_name.text.trim()
        }
        .to_owned();
        let result = if database {
            let db = database_definition(&name);
            self.store
                .create_database_page(&self.workspace.id, parent.as_deref(), &name, "▦", db)
        } else {
            self.store.create_page(
                &self.workspace.id,
                parent.as_deref(),
                &name,
                "▤",
                PageKind::Document,
            )
        };
        match result {
            Ok(p) => {
                self.error = None;
                if let Some(parent_id) = p.parent_id.as_deref() {
                    self.refresh_child_pages(parent_id);
                }
                self.refresh_pages();
                self.open_page(&p.id, true);
                self.tabs[self.active].choosing_type = false;
                self.new_name = TextEdit::new("");
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
}

fn database_definition(name: &str) -> Database {
    Database {
        id: new_id(),
        page_id: String::new(),
        name: name.to_owned(),
        properties: vec![
            Property {
                id: new_id(),
                name: "Name".into(),
                kind: PropertyKind::Title,
                config: json!({}),
            },
            Property {
                id: new_id(),
                name: "Status".into(),
                kind: PropertyKind::Status,
                config: json!({"options":["Not started","In progress","Done"]}),
            },
            Property {
                id: new_id(),
                name: "Date".into(),
                kind: PropertyKind::Date,
                config: json!({}),
            },
        ],
        views: ViewKind::ALL
            .into_iter()
            .filter(|k| {
                matches!(
                    k,
                    ViewKind::Table
                        | ViewKind::Board
                        | ViewKind::Calendar
                        | ViewKind::Timeline
                        | ViewKind::Gallery
                        | ViewKind::List
                )
            })
            .map(|kind| DatabaseView {
                id: new_id(),
                name: kind.label().into(),
                kind,
                query: RecordQuery::default(),
                config: json!({}),
            })
            .collect(),
    }
}
