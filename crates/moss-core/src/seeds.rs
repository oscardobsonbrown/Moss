use crate::{
    Block, Database, DatabaseView, PageKind, Property, PropertyKind, Record, RecordQuery, Store,
    ViewKind, Workspace, new_id, now,
};
use rusqlite::Result;
use serde_json::json;
use std::collections::BTreeMap;

impl Store {
    /// Seed only an empty store. Existing workspace data is never replaced.
    pub fn seed_demo_workspaces(&mut self) -> Result<Vec<Workspace>> {
        let existing = self.list_workspaces()?;
        if !existing.is_empty() {
            return Ok(existing);
        }
        let examples: [(&str, &str, &str, [&str; 6]); 5] = [
            (
                "Projects & Tasks",
                "📋",
                "Tasks",
                [
                    "Plan project",
                    "Draft proposal",
                    "Review design",
                    "Build prototype",
                    "Test workflow",
                    "Ship release",
                ],
            ),
            (
                "School",
                "🎓",
                "Assignments",
                [
                    "Read chapter",
                    "Research topic",
                    "Draft essay",
                    "Study maths",
                    "Prepare seminar",
                    "Submit assignment",
                ],
            ),
            (
                "Content Calendar",
                "🗓️",
                "Content",
                [
                    "Plan campaign",
                    "Write article",
                    "Draft newsletter",
                    "Record video",
                    "Review graphics",
                    "Publish recap",
                ],
            ),
            (
                "Sales CRM",
                "🤝",
                "Opportunities",
                [
                    "Northwind discovery",
                    "Orchard follow-up",
                    "River proposal",
                    "Summit demo",
                    "Cedar negotiation",
                    "Harbor onboarding",
                ],
            ),
            (
                "Meal Planning",
                "🥗",
                "Meals",
                [
                    "Overnight oats",
                    "Lentil bowl",
                    "Tofu stir-fry",
                    "Chicken salad",
                    "Pasta primavera",
                    "Vegetable soup",
                ],
            ),
        ];
        let mut result = Vec::new();
        for (name, icon, db_name, rows) in examples {
            let w = self.create_workspace(name, icon)?;
            let welcome = self.create_page(&w.id, None, "Welcome", icon, PageKind::Document)?;
            self.save_blocks(
                &welcome.id,
                &[Block::paragraph(
                    &welcome.id,
                    format!("Welcome to {name}. Use the database to organise your work."),
                    0,
                )],
            )?;
            let page = self.create_page(&w.id, None, db_name, "📁", PageKind::Database)?;
            let title = Property {
                id: new_id(),
                name: "Name".into(),
                kind: PropertyKind::Title,
                config: json!({}),
            };
            let status = Property {
                id: new_id(),
                name: "Status".into(),
                kind: PropertyKind::Status,
                config: json!({"options":["Not started","In progress","Done"]}),
            };
            let date = Property {
                id: new_id(),
                name: "Date".into(),
                kind: PropertyKind::Date,
                config: json!({}),
            };
            let notes = Property {
                id: new_id(),
                name: "Notes".into(),
                kind: PropertyKind::Text,
                config: json!({}),
            };
            let db_id = new_id();
            let relation = Property {
                id: new_id(),
                name: "Related".into(),
                kind: PropertyKind::Relation,
                config: json!({"target_database_id": db_id}),
            };
            let db = Database {
                id: db_id,
                page_id: page.id.clone(),
                name: db_name.into(),
                properties: vec![
                    title,
                    status.clone(),
                    date.clone(),
                    notes.clone(),
                    relation.clone(),
                ],
                views: ViewKind::ALL
                    .into_iter()
                    .map(|kind| DatabaseView {
                        id: new_id(),
                        name: kind.label().into(),
                        kind,
                        query: RecordQuery::default(),
                        config: json!({}),
                    })
                    .collect(),
            };
            self.save_database(&db)?;
            let record_ids: Vec<String> = rows.iter().map(|_| new_id()).collect();
            for (i, title) in rows.into_iter().enumerate() {
                let t = now();
                let mut values = BTreeMap::new();
                values.insert(
                    status.id.clone(),
                    json!(["Not started", "In progress", "Done"][i % 3]),
                );
                values.insert(date.id.clone(), json!(format!("2026-11-{:02}", 9 + i)));
                values.insert(notes.id.clone(), json!("Example item"));
                self.save_record(&Record {
                    id: record_ids[i].clone(),
                    database_id: db.id.clone(),
                    page_id: None,
                    title: title.into(),
                    values,
                    created_at: t.clone(),
                    updated_at: t,
                    deleted_at: None,
                })?;
            }
            if record_ids.len() > 1 {
                let mut record = self
                    .get_record(&record_ids[0])?
                    .ok_or_else(|| crate::store::invalid("seed record missing"))?;
                record
                    .values
                    .insert(relation.id.clone(), json!([record_ids[1]]));
                self.save_record(&record)?;
            }
            result.push(w);
        }
        Ok(result)
    }
}
