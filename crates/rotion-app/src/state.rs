use rotion_core::*;
use rune::TextEdit;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};

pub struct EditableBlock {
    pub block: Block,
    pub edit: TextEdit,
}
pub struct Tab {
    pub page: Page,
    pub title: TextEdit,
    pub blocks: Vec<EditableBlock>,
    pub database: Option<Database>,
    pub records: Vec<Record>,
    pub view: usize,
    pub offset: u64,
    dirty_title: bool,
    dirty_blocks: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    Search,
    Settings,
    PageActions,
    Appearance,
    Trash,
    Workspace,
    Block(usize),
    Views,
    Record(usize),
    NewPage,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Preferences {
    pub dark: Option<bool>,
    pub workspace: String,
    pub tabs: BTreeMap<String, Vec<String>>,
    pub active: BTreeMap<String, usize>,
}
pub struct State {
    pub store: Store,
    pub path: PathBuf,
    pub workspaces: Vec<Workspace>,
    pub workspace: Workspace,
    pub pages: Vec<Page>,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub prefs: Preferences,
    pub shared_preferences: std::sync::Arc<std::sync::Mutex<Preferences>>,
    pub modal: Option<Modal>,
    pub search: TextEdit,
    pub all_workspaces: bool,
    pub hits: Vec<SearchHit>,
    pub trash: Vec<Page>,
    pub filter: TextEdit,
    pub new_name: TextEdit,
    pub error: Option<String>,
    loading: bool,
    pub cell: TextEdit,
    pub editing_property: Option<String>,
    pub handles: std::sync::Arc<std::sync::Mutex<BTreeMap<String, rune::AppHandle<State>>>>,
}
impl State {
    pub fn data_path() -> PathBuf {
        std::env::var_os("ROTION_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                    .join("Library/Application Support/Rotion")
            })
    }
    pub fn open(
        path: PathBuf,
        workspace_id: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(&path)?;
        let mut store = Store::open(path.join("workspace.sqlite3"))?;
        if store.list_workspaces()?.is_empty() {
            if std::env::var_os("ROTION_DEMO").is_some() {
                store.seed_demo_workspaces()?;
            } else {
                let w = store.create_workspace("Personal", "◈")?;
                let page =
                    store.create_page(&w.id, None, "My workspace", "📖", PageKind::Document)?;
                let mut blocks = vec![
                    Block::paragraph(&page.id, "A place for your notes, plans, and ideas.", 0),
                    Block::paragraph(&page.id, "Make room for what matters", 1),
                    Block::paragraph(
                        &page.id,
                        "Start writing here. Add a page from the sidebar, or press / in an empty block to choose a block type.",
                        2,
                    ),
                    Block::paragraph(&page.id, "Everything you write stays on this Mac.", 3),
                ];
                blocks[1].kind = BlockKind::Heading2;
                store.save_blocks(&page.id, &blocks)?;
            }
        }
        let workspaces = store.list_workspaces()?;
        let prefs: Preferences = std::fs::read(path.join("preferences.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let id = workspace_id.unwrap_or(&prefs.workspace);
        let workspace = workspaces
            .iter()
            .find(|w| w.id == id)
            .unwrap_or(&workspaces[0])
            .clone();
        let pages = store.list_pages(&workspace.id)?;
        let restore = prefs.tabs.get(&workspace.id).cloned().unwrap_or_default();
        let active = *prefs.active.get(&workspace.id).unwrap_or(&0);
        let mut s = Self {
            store,
            path,
            workspaces,
            workspace,
            pages,
            tabs: vec![],
            active: 0,
            shared_preferences: std::sync::Arc::new(std::sync::Mutex::new(prefs.clone())),
            prefs,
            modal: None,
            search: TextEdit::new(""),
            all_workspaces: false,
            hits: vec![],
            trash: vec![],
            filter: TextEdit::new(""),
            new_name: TextEdit::new(""),
            error: None,
            loading: true,
            cell: TextEdit::new(""),
            editing_property: None,
            handles: Default::default(),
        };
        for id in restore {
            if s.pages.iter().any(|p| p.id == id) {
                s.open_page(&id, true);
            }
        }
        if s.tabs.is_empty() {
            if let Some(p) = s.pages.first() {
                let id = p.id.clone();
                s.open_page(&id, true);
            }
        }
        if !s.tabs.is_empty() {
            s.active = active.min(s.tabs.len() - 1);
        }
        s.loading = false;
        Ok(s)
    }
    pub fn tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }
    pub fn save_preferences(&mut self) {
        if self.loading {
            return;
        }
        let owner = self.shared_preferences.clone();
        let mut shared = owner.lock().unwrap();
        shared.workspace = self.workspace.id.clone();
        shared.tabs.insert(
            self.workspace.id.clone(),
            self.tabs.iter().map(|t| t.page.id.clone()).collect(),
        );
        shared.active.insert(self.workspace.id.clone(), self.active);
        self.prefs = shared.clone();
        let result = serde_json::to_vec(&self.prefs)
            .map_err(|e| e.to_string())
            .and_then(|b| {
                std::fs::write(self.path.join("preferences.json.tmp"), b).map_err(|e| e.to_string())
            })
            .and_then(|_| {
                std::fs::rename(
                    self.path.join("preferences.json.tmp"),
                    self.path.join("preferences.json"),
                )
                .map_err(|e| e.to_string())
            });
        if let Err(e) = result {
            self.error = Some(format!("Could not save window settings: {e}"));
        }
    }
    pub fn set_theme(&mut self, dark: Option<bool>) {
        self.shared_preferences.lock().unwrap().dark = dark;
        self.prefs.dark = dark;
        if let Ok(handles) = self.handles.lock() {
            for h in handles.values() {
                h.update(move |s, _| s.prefs.dark = dark);
            }
        }
        self.save_preferences();
    }
    pub fn set_page_settings(&mut self, settings: PageSettings) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let mut page = tab.page.clone();
        page.settings = settings;
        page.updated_at = now();
        match self.store.update_page(&page) {
            Ok(()) => {
                self.tabs[self.active].page = page.clone();
                if let Some(p) = self.pages.iter_mut().find(|p| p.id == page.id) {
                    *p = page;
                }
            }
            Err(e) => self.error = Some(format!("Page settings are not saved: {e}")),
        }
    }
    pub fn open_page(&mut self, id: &str, new_tab: bool) {
        if !self.loading && !self.flush_current() {
            return;
        }
        self.modal = None;
        if let Some(i) = self.tabs.iter().position(|t| t.page.id == id) {
            self.active = i;
            self.save_preferences();
            return;
        }
        let result = (|| -> Result<Tab, Box<dyn std::error::Error>> {
            let page = self.store.get_page(id)?.ok_or("Page was not found")?;
            let mut blocks = self.store.get_blocks(id)?;
            if blocks.is_empty() {
                blocks.push(Block::paragraph(id, "", 0));
            }
            let database = self.store.get_database_by_page(id)?;
            let records = if let Some(db) = &database {
                self.store.query_records(
                    &db.id,
                    &RecordQuery {
                        limit: 100,
                        ..Default::default()
                    },
                )?
            } else {
                vec![]
            };
            Ok(Tab {
                title: TextEdit::new(&page.title),
                page,
                blocks: blocks
                    .into_iter()
                    .map(|block| {
                        let edit = crate::rich_text::load(&block);
                        EditableBlock { block, edit }
                    })
                    .collect(),
                database,
                records,
                view: 0,
                offset: 0,
                dirty_title: false,
                dirty_blocks: false,
            })
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
                self.save_preferences();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() || !self.flush_current() {
            return;
        }
        self.modal = None;
        self.tabs.remove(i);
        if i < self.active {
            self.active -= 1;
        } else {
            self.active = self.active.min(self.tabs.len().saturating_sub(1));
        }
        self.save_preferences();
    }
    pub fn flush_current(&mut self) -> bool {
        let Some(tab) = self.tab() else {
            return true;
        };
        let (title, blocks) = (tab.dirty_title, tab.dirty_blocks);
        if title {
            self.save_title();
        }
        if blocks {
            self.save_blocks();
        }
        self.tab().is_none_or(|t| !t.dirty_title && !t.dirty_blocks)
    }
    pub fn refresh_pages(&mut self) {
        match self.store.list_pages(&self.workspace.id) {
            Ok(p) => self.pages = p,
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn save_title(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        tab.dirty_title = true;
        tab.page.title = tab.title.text.clone();
        tab.page.updated_at = now();
        match self.store.update_page(&tab.page) {
            Ok(()) => tab.dirty_title = false,
            Err(e) => self.error = Some(format!("Title is not saved: {e}")),
        }
        if let Some(page) = self.pages.iter_mut().find(|p| p.id == tab.page.id) {
            *page = tab.page.clone();
        }
    }
    pub fn save_blocks(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        tab.dirty_blocks = true;
        let blocks: Vec<Block> = tab
            .blocks
            .iter_mut()
            .enumerate()
            .map(|(i, b)| {
                crate::rich_text::save(&mut b.block, &b.edit);
                b.block.sort_order = i as i64;
                b.block.clone()
            })
            .collect();
        match self.store.save_blocks(&tab.page.id, &blocks) {
            Ok(()) => tab.dirty_blocks = false,
            Err(e) => {
                self.error = Some(format!(
                    "Changes are not saved. Keep this page open and try again: {e}"
                ))
            }
        }
    }
    pub fn add_block(&mut self, after: usize) {
        let tab = &mut self.tabs[self.active];
        if tab.blocks.is_empty() {
            tab.blocks.push(EditableBlock {
                block: Block::paragraph(&tab.page.id, "", 0),
                edit: TextEdit::new("").rich(),
            });
        }
        let after = after.min(tab.blocks.len() - 1);
        let edit = &mut tab.blocks[after].edit;
        let (start, end) = edit.range();
        let mut suffix = edit.clone();
        suffix.set_selection(0, end);
        suffix.insert("");
        suffix.clear_history();
        edit.set_selection(start, edit.text.len());
        edit.insert("");
        edit.focused = false;
        let mut edit = suffix;
        edit.set_caret(0);
        edit.focused = true;
        let mut block = Block::paragraph(&tab.page.id, "", after as i64 + 1);
        let oldkind = tab.blocks[after].block.kind;
        if matches!(
            oldkind,
            BlockKind::BulletedList | BlockKind::NumberedList | BlockKind::Todo
        ) {
            block.kind = oldkind;
        }
        tab.blocks.insert(after + 1, EditableBlock { block, edit });
        self.save_blocks();
    }
    pub fn new_page(&mut self, database: bool, parent: Option<String>) {
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
        let result = self.store.create_page(
            &self.workspace.id,
            parent.as_deref(),
            &name,
            if database { "▦" } else { "▤" },
            if database {
                PageKind::Database
            } else {
                PageKind::Document
            },
        );
        match result {
            Ok(p) => {
                if database {
                    let db = Database {
                        id: new_id(),
                        page_id: p.id.clone(),
                        name: name.clone(),
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
                    };
                    if let Err(e) = self.store.save_database(&db) {
                        self.error = Some(e.to_string());
                        return;
                    }
                }
                self.refresh_pages();
                self.open_page(&p.id, true);
                self.new_name = TextEdit::new("");
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn open_search(&mut self) {
        self.search = TextEdit::new("");
        self.search.focused = true;
        self.hits.clear();
        self.modal = Some(Modal::Search);
    }
    pub fn run_search(&mut self) {
        match self.store.search(
            &self.search.text,
            if self.all_workspaces {
                None
            } else {
                Some(&self.workspace.id)
            },
            30,
        ) {
            Ok(h) => self.hits = h,
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn open_trash(&mut self) {
        match self.store.list_trash(&self.workspace.id) {
            Ok(p) => {
                self.trash = p;
                self.modal = Some(Modal::Trash)
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn trash_page(&mut self) {
        let Some(t) = self.tab() else { return };
        let id = t.page.id.clone();
        match self.store.delete_page(&id) {
            Ok(()) => {
                self.tabs.retain(|t| t.page.id != id);
                self.active = self.active.min(self.tabs.len().saturating_sub(1));
                self.refresh_pages();
                self.save_preferences();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn restore_page(&mut self, id: &str) {
        if let Err(e) = self.store.restore_page(id) {
            self.error = Some(e.to_string())
        }
        self.refresh_pages();
        self.open_trash();
    }
    pub fn query_records(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let Some(db) = &tab.database else { return };
        let mut q = db
            .views
            .get(tab.view)
            .map(|v| v.query.clone())
            .unwrap_or_default();
        q.offset = tab.offset;
        q.limit = 100;
        if !self.filter.text.is_empty() {
            q.filter = Some(FilterExpr::Rule(FilterRule {
                property_id: db
                    .properties
                    .iter()
                    .find(|p| p.kind == PropertyKind::Title)
                    .map(|p| p.id.clone())
                    .unwrap_or_default(),
                op: FilterOp::Contains,
                value: json!(self.filter.text),
            }));
        }
        match self.store.query_records(&db.id, &q) {
            Ok(r) => tab.records = r,
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn add_record(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let Some(db) = &tab.database else { return };
        let t = now();
        let r = Record {
            id: new_id(),
            database_id: db.id.clone(),
            page_id: None,
            title: "Untitled".into(),
            values: BTreeMap::new(),
            created_at: t.clone(),
            updated_at: t,
            deleted_at: None,
        };
        match self.store.save_record(&r) {
            Ok(()) => {
                tab.records.insert(0, r);
                self.modal = Some(Modal::Record(0));
                self.cell = TextEdit::new("Untitled");
                self.cell.focused = true;
                self.editing_property = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn edit_record(&mut self, i: usize, prop: Option<String>) {
        let tab = &self.tabs[self.active];
        let Some(r) = tab.records.get(i) else {
            return;
        };
        if let Some(id) = &prop {
            let kind = tab
                .database
                .as_ref()
                .and_then(|db| db.properties.iter().find(|p| &p.id == id))
                .map(|p| p.kind);
            if !matches!(
                kind,
                Some(
                    PropertyKind::Title
                        | PropertyKind::Text
                        | PropertyKind::Number
                        | PropertyKind::Select
                        | PropertyKind::Status
                        | PropertyKind::Date
                        | PropertyKind::Checkbox
                        | PropertyKind::Url
                        | PropertyKind::Email
                        | PropertyKind::Phone
                )
            ) {
                self.error =
                    Some("This property is read-only until its editor is available.".into());
                return;
            }
            if r.values
                .get(id)
                .is_some_and(|v| v.is_array() || v.is_object())
            {
                self.error = Some("This value needs a structured property editor.".into());
                return;
            }
        }
        self.error = None;
        self.cell = TextEdit::new(match &prop {
            None => r.title.clone(),
            Some(p) => r.values.get(p).map(display_value).unwrap_or_default(),
        });
        self.cell.focused = true;
        self.editing_property = prop;
        self.modal = Some(Modal::Record(i));
    }
    pub fn save_record(&mut self) {
        let Some(Modal::Record(i)) = self.modal else {
            return;
        };
        let tab = &mut self.tabs[self.active];
        let r = &mut tab.records[i];
        if let Some(p) = &self.editing_property {
            let kind = tab
                .database
                .as_ref()
                .and_then(|d| d.properties.iter().find(|x| &x.id == p))
                .map(|p| p.kind);
            let value = match kind {
                Some(PropertyKind::Number) => match self.cell.text.parse::<f64>() {
                    Ok(n) => json!(n),
                    Err(_) => {
                        self.error = Some("Enter a number.".into());
                        return;
                    }
                },
                Some(PropertyKind::Checkbox) => json!(self.cell.text == "true"),
                Some(PropertyKind::Date) if !self.cell.text.is_empty() => {
                    if chrono::NaiveDate::parse_from_str(&self.cell.text, "%Y-%m-%d").is_err() {
                        self.error = Some("Use a date in YYYY-MM-DD format.".into());
                        return;
                    }
                    json!(self.cell.text)
                }
                _ => json!(self.cell.text),
            };
            r.values.insert(p.clone(), value);
        } else {
            r.title = self.cell.text.clone();
        }
        r.updated_at = now();
        match self.store.save_record(r) {
            Ok(()) => {
                self.modal = None;
                self.query_records();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn switch_workspace(&mut self, id: &str) {
        if id == self.workspace.id {
            self.modal = None;
            return;
        }
        if let Ok(handles) = self.handles.lock() {
            if let Some(h) = handles.get(id) {
                h.open();
                self.modal = None;
                return;
            }
        }
        self.error =
            Some("This workspace window is not available. Restart Rotion to open it.".into());
    }
}
pub fn display_value(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(display_value).collect::<Vec<_>>().join(", "),
        _ => v.to_string(),
    }
}
