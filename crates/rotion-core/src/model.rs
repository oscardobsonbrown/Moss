use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub created_at: String,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    #[default]
    Document,
    Database,
    Record,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageFont {
    #[default]
    Default,
    Serif,
    Mono,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PageSettings {
    pub font: PageFont,
    pub small_text: bool,
    pub full_width: bool,
}
impl PageSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Page {
    pub id: String,
    pub workspace_id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub icon: String,
    pub kind: PageKind,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    // Omit defaults so archives made before page settings keep their checksum.
    #[serde(default, skip_serializing_if = "PageSettings::is_default")]
    pub settings: PageSettings,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    #[default]
    Paragraph,
    Heading1,
    Heading2,
    Heading3,
    BulletedList,
    NumberedList,
    Todo,
    Toggle,
    Quote,
    Callout,
    Code,
    Divider,
    Image,
    Video,
    Audio,
    Pdf,
    File,
    Bookmark,
    Embed,
    Equation,
    TableOfContents,
    SyncedBlock,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub id: String,
    pub page_id: String,
    pub kind: BlockKind,
    pub text: String,
    pub checked: bool,
    pub depth: u32,
    pub sort_order: i64,
    pub config: Value,
}

impl Block {
    pub fn paragraph(page_id: &str, text: impl Into<String>, sort_order: i64) -> Self {
        Self {
            id: new_id(),
            page_id: page_id.to_owned(),
            kind: BlockKind::Paragraph,
            text: text.into(),
            checked: false,
            depth: 0,
            sort_order,
            config: Value::Null,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PropertyKind {
    Title,
    Text,
    Number,
    Select,
    MultiSelect,
    Status,
    Date,
    Person,
    Files,
    Checkbox,
    Url,
    Email,
    Phone,
    Relation,
    Rollup,
    Formula,
    CreatedTime,
    EditedTime,
    UniqueId,
    Place,
    Button,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Property {
    pub id: String,
    pub name: String,
    pub kind: PropertyKind,
    /// Type-specific settings: options, target database, formula, or rollup settings.
    pub config: Value,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    #[default]
    Table,
    Board,
    Calendar,
    Timeline,
    Gallery,
    List,
    Chart,
    Feed,
    Map,
    Dashboard,
}

impl ViewKind {
    pub const ALL: [Self; 10] = [
        Self::Table,
        Self::Board,
        Self::Calendar,
        Self::Timeline,
        Self::Gallery,
        Self::List,
        Self::Chart,
        Self::Feed,
        Self::Map,
        Self::Dashboard,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Table => "Table",
            Self::Board => "Board",
            Self::Calendar => "Calendar",
            Self::Timeline => "Timeline",
            Self::Gallery => "Gallery",
            Self::List => "List",
            Self::Chart => "Chart",
            Self::Feed => "Feed",
            Self::Map => "Map",
            Self::Dashboard => "Dashboard",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DatabaseView {
    pub id: String,
    pub name: String,
    pub kind: ViewKind,
    pub query: RecordQuery,
    pub config: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Database {
    pub id: String,
    pub page_id: String,
    pub name: String,
    pub properties: Vec<Property>,
    pub views: Vec<DatabaseView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Record {
    pub id: String,
    pub database_id: String,
    pub page_id: Option<String>,
    pub title: String,
    pub values: BTreeMap<String, Value>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Ne,
    Contains,
    NotContains,
    Gt,
    Ge,
    Lt,
    Le,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FilterRule {
    pub property_id: String,
    pub op: FilterOp,
    pub value: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FilterExpr {
    Rule(FilterRule),
    All(Vec<FilterExpr>),
    Any(Vec<FilterExpr>),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SortRule {
    pub property_id: String,
    pub descending: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RecordQuery {
    pub filter: Option<FilterExpr>,
    pub sorts: Vec<SortRule>,
    pub group_by: Option<String>,
    pub limit: u32,
    pub offset: u64,
}

impl Default for RecordQuery {
    fn default() -> Self {
        Self {
            filter: None,
            sorts: Vec::new(),
            group_by: None,
            limit: 200,
            offset: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SearchHit {
    pub page_id: String,
    pub workspace_id: String,
    pub title: String,
    pub snippet: String,
    pub kind: PageKind,
}
