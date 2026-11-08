use super::*;
use crate::{now, properties::with_database};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

fn rename_filter(filter: &mut FilterExpr, property: &str, old: &str, new: &str) {
    match filter {
        FilterExpr::Rule(rule) => {
            if rule.property_id == property
                && matches!(rule.op, FilterOp::Eq | FilterOp::Ne)
                && rule.value.as_str() == Some(old)
            {
                rule.value = json!(new);
            }
        }
        FilterExpr::All(items) | FilterExpr::Any(items) => {
            for item in items {
                rename_filter(item, property, old, new);
            }
        }
    }
}
pub(super) fn valid_group(property: &Property, value: &Value) -> bool {
    if value.is_null() {
        return true;
    }
    match property.kind {
        PropertyKind::Status
        | PropertyKind::Select
        | PropertyKind::Text
        | PropertyKind::Url
        | PropertyKind::Email
        | PropertyKind::Phone => value.is_string(),
        PropertyKind::Number => value.is_number(),
        PropertyKind::Checkbox => value.is_boolean(),
        PropertyKind::Date => value
            .as_str()
            .or_else(|| value.get("start").and_then(Value::as_str))
            .and_then(|s| s.get(..10))
            .is_some_and(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()),
        _ => false,
    }
}
fn json_group_expr(property: &Property, args: &mut Vec<SqlValue>) -> String {
    args.push(SqlValue::Text(property.id.clone()));
    "COALESCE((SELECT CASE type WHEN 'null' THEN 'null' WHEN 'text' THEN CASE WHEN value='' THEN 'null' ELSE json_quote(value) END WHEN 'true' THEN 'true' WHEN 'false' THEN 'false' WHEN 'object' THEN json(value) WHEN 'array' THEN json(value) ELSE CAST(value AS TEXT) END FROM json_each(records.values_json) WHERE key=? LIMIT 1),'null')".into()
}
fn group_match_sql(property: &Property, value: &Value, args: &mut Vec<SqlValue>) -> String {
    if value.is_number() {
        args.push(SqlValue::Text(property.id.clone()));
        args.push(
            value
                .as_i64()
                .map(SqlValue::Integer)
                .unwrap_or_else(|| SqlValue::Real(value.as_f64().unwrap())),
        );
        "(SELECT value FROM json_each(records.values_json) WHERE key=? AND type IN ('integer','real') LIMIT 1) IS ?".into()
    } else {
        let expression = json_group_expr(property, args);
        args.push(SqlValue::Text(group_key(value)));
        format!("{expression}=?")
    }
}

fn effective_rank(tx: &rusqlite::Connection, view_id: &str, record_id: &str) -> Result<Vec<u8>> {
    tx.query_row("SELECT COALESCE(bo.rank,CAST(records.id AS BLOB)) FROM records LEFT JOIN board_record_order bo ON bo.record_id=records.id AND bo.view_id=?1 WHERE records.id=?2",params![view_id,record_id],|r|r.get(0))
}
fn rank_between(low: Option<&[u8]>, high: Option<&[u8]>) -> Result<Vec<u8>> {
    if low.zip(high).is_some_and(|(a, b)| a >= b) {
        return Err(invalid("Card order changed; reload the Board."));
    }
    let mut upper = high;
    let mut key = Vec::new();
    for index in 0..512 {
        let a = low.and_then(|s| s.get(index)).copied().unwrap_or(0);
        let b = upper.and_then(|s| s.get(index)).copied().unwrap_or(255);
        if b > a.saturating_add(1) {
            key.push(a + (b - a) / 2);
            return Ok(key);
        }
        key.push(a);
        if a < b {
            upper = None;
        }
    }
    Err(invalid("Card order is too dense; reload this Board."))
}

impl Store {
    pub fn board_record_rank(&self, view_id: &str, record_id: &str) -> Result<Vec<u8>> {
        effective_rank(&self.conn, view_id, record_id)
    }
    pub fn board_record_matches(
        &self,
        database_id: &str,
        view_id: &str,
        record_id: &str,
        title_search: &str,
    ) -> Result<bool> {
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("Database is unavailable."))?;
        let view = db
            .views
            .iter()
            .find(|v| v.id == view_id)
            .ok_or_else(|| invalid("View is unavailable."))?;
        let mut args = vec![
            SqlValue::Text(database_id.into()),
            SqlValue::Text(record_id.into()),
        ];
        let mut sql = String::from(
            "SELECT 1 FROM records WHERE database_id=? AND id=? AND deleted_at IS NULL",
        );
        if let Some(filter) = &view.query.filter {
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(filter, &db, &mut args, 0)?);
        }
        if !title_search.is_empty() {
            let property = db
                .properties
                .iter()
                .find(|p| p.kind == PropertyKind::Title)
                .ok_or_else(|| invalid("Title property is unavailable."))?;
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(
                &FilterExpr::Rule(FilterRule {
                    property_id: property.id.clone(),
                    op: FilterOp::Contains,
                    value: json!(title_search),
                }),
                &db,
                &mut args,
                0,
            )?);
        }
        Ok(self
            .conn
            .query_row(&sql, params_from_iter(args), |_| Ok(()))
            .optional()?
            .is_some())
    }
    /// Move both dimensions and the per-view card position in one guarded transaction.
    pub fn move_board_record(
        &mut self,
        database_id: &str,
        expected_view: &DatabaseView,
        expected: &Record,
        property: &Property,
        subproperty: Option<&Property>,
        group: Value,
        subgroup: Value,
        before: Option<&str>,
        expected_rank: &[u8],
    ) -> Result<Record> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let raw: String = tx.query_row(
            "SELECT data FROM databases WHERE id=?1",
            [database_id],
            |r| r.get(0),
        )?;
        let db: Database = decode(raw)?;
        let owner = search::page(&tx, &db.page_id)?
            .ok_or_else(|| invalid("Database page is unavailable."))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("Database is in Trash."));
        }
        let view = db
            .views
            .iter()
            .find(|v| v.id == expected_view.id && v.kind == ViewKind::Board)
            .ok_or_else(|| invalid("Board view is unavailable."))?;
        if view.query.filter != expected_view.query.filter
            || view.query.group_by != expected_view.query.group_by
            || view.config.get("board_subgroup_by") != expected_view.config.get("board_subgroup_by")
            || view.query.sorts != expected_view.query.sorts
        {
            return Err(invalid(
                "Board grouping or sorting changed; try the move again.",
            ));
        }
        let current_property = group_property(&db, view)?;
        let current_sub = subgroup_property(&db, view)?;
        if current_property != property || current_sub != subproperty {
            return Err(invalid("Grouping property changed; try the move again."));
        }
        let raw: String = tx
            .query_row(
                "SELECT data FROM records WHERE id=?1",
                [&expected.id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| invalid("Card is unavailable."))?;
        let mut record: Record = decode(raw)?;
        if record.database_id != database_id
            || expected.database_id != database_id
            || record.deleted_at.is_some()
        {
            return Err(invalid("Card is unavailable in this Board."));
        }
        for p in std::iter::once(property).chain(subproperty) {
            if normalize(record.values.get(&p.id).unwrap_or(&Value::Null))
                != normalize(expected.values.get(&p.id).unwrap_or(&Value::Null))
            {
                return Err(invalid(
                    "Card group changed in another editor; try the move again.",
                ));
            }
        }
        if effective_rank(&tx, &view.id, &record.id)? != expected_rank {
            return Err(invalid(
                "Card position changed in another editor; try the move again.",
            ));
        }
        let group = normalize(&group);
        let subgroup = normalize(&subgroup);
        if !valid_group(property, &group) || subproperty.is_some_and(|p| !valid_group(p, &subgroup))
        {
            return Err(invalid("Invalid value for this Board group."));
        }
        let changed = group_value(&record, property) != group
            || subproperty.is_some_and(|p| group_value(&record, p) != subgroup);
        if !view.query.sorts.is_empty() && !changed {
            return Err(invalid("Turn off sorting to reorder cards manually."));
        }
        if before == Some(record.id.as_str()) {
            return Ok(record);
        }
        if let Some(before) = before {
            let raw: String = tx
                .query_row("SELECT data FROM records WHERE id=?1", [before], |r| {
                    r.get(0)
                })
                .optional()?
                .ok_or_else(|| invalid("Destination card is unavailable."))?;
            let destination: Record = decode(raw)?;
            if destination.deleted_at.is_some()
                || destination.database_id != database_id
                || group_value(&destination, property) != group
                || subproperty.is_some_and(|p| group_value(&destination, p) != subgroup)
            {
                return Err(invalid("Destination group changed; try the move again."));
            }
        }
        if view.query.sorts.is_empty() {
            let high = before
                .map(|id| effective_rank(&tx, &view.id, id))
                .transpose()?;
            let mut args = vec![
                SqlValue::Text(view.id.clone()),
                SqlValue::Text(database_id.into()),
                SqlValue::Text(record.id.clone()),
            ];
            let mut sql = String::from(
                "SELECT COALESCE(bo.rank,CAST(records.id AS BLOB)) AS position FROM records LEFT JOIN board_record_order bo ON bo.record_id=records.id AND bo.view_id=? WHERE records.database_id=? AND records.id<>? AND records.deleted_at IS NULL AND ",
            );
            sql.push_str(&group_match_sql(property, &group, &mut args));
            if let Some(sub) = subproperty {
                sql.push_str(" AND ");
                sql.push_str(&group_match_sql(sub, &subgroup, &mut args));
            }
            if let Some(high) = &high {
                sql.push_str(" AND COALESCE(bo.rank,CAST(records.id AS BLOB))<?");
                args.push(SqlValue::Blob(high.clone()));
            }
            sql.push_str(" ORDER BY position DESC LIMIT 1");
            let low: Option<Vec<u8>> = tx
                .query_row(&sql, params_from_iter(args), |r| r.get(0))
                .optional()?;
            let rank = rank_between(low.as_deref(), high.as_deref())?;
            tx.execute("INSERT INTO board_record_order(view_id,record_id,rank) VALUES(?1,?2,?3) ON CONFLICT(view_id,record_id) DO UPDATE SET rank=excluded.rank",params![view.id,record.id,rank])?;
        }
        record.values.insert(property.id.clone(), group);
        if let Some(sub) = subproperty {
            record.values.insert(sub.id.clone(), subgroup);
        }
        record.updated_at = now();
        let values = encode(&record.values)?;
        tx.execute(
            "UPDATE records SET values_json=?1,data=?2 WHERE id=?3",
            params![values, encode(&record)?, record.id],
        )?;
        let target = search::page(&tx, record.page_id.as_deref().unwrap_or(&db.page_id))?
            .ok_or_else(|| invalid("Card page is unavailable."))?;
        if target.deleted_at.is_some()
            || target.workspace_id != owner.workspace_id
            || (record.page_id.is_some()
                && (target.kind != PageKind::Record
                    || target.parent_id.as_deref() != Some(owner.id.as_str())
                    || target.title != record.title))
        {
            return Err(invalid("Card page is unavailable."));
        }
        search::index(
            &tx,
            &format!("record:{}", record.id),
            &target,
            &format!("{} {values}", record.title),
        )?;
        crate::calculation::refresh(&tx, std::slice::from_ref(&record.id))?;
        let raw: String =
            tx.query_row("SELECT data FROM records WHERE id=?1", [&record.id], |r| {
                r.get(0)
            })?;
        record = decode(raw)?;
        tx.commit()?;
        Ok(record)
    }

    /// Add or rename an option and its saved values while retaining other option metadata.
    pub fn save_board_group_name(
        &mut self,
        database_id: &str,
        expected: &Property,
        old: Option<&Value>,
        name: &str,
    ) -> Result<Database> {
        let name = name.trim();
        if name.is_empty() || name.len() > 256 {
            return Err(invalid("Group name must contain 1 to 256 bytes."));
        }
        if !matches!(expected.kind, PropertyKind::Select | PropertyKind::Status) {
            return Err(invalid("Only Select and Status groups can be renamed."));
        }
        let old = old
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| invalid("Choose a named group to rename."))
            })
            .transpose()?;
        with_database(self, database_id, |tx, db, owner| {
            let default_group = db
                .properties
                .iter()
                .find(|p| matches!(p.kind, PropertyKind::Status | PropertyKind::Select))
                .map(|p| p.id.clone());
            let property = db
                .properties
                .iter_mut()
                .find(|p| p.id == expected.id)
                .ok_or_else(|| invalid("Grouping property is unavailable."))?;
            if property != expected {
                return Err(invalid(
                    "Group options changed in another editor; reopen this menu.",
                ));
            }
            let options = property
                .config
                .get_mut("options")
                .and_then(Value::as_array_mut)
                .ok_or_else(|| invalid("Group options are unavailable."))?;
            let option_name = |v: &Value| {
                v.as_str()
                    .or_else(|| v.get("name").and_then(Value::as_str))
                    .unwrap_or("")
                    .to_owned()
            };
            if options
                .iter()
                .any(|v| option_name(v) == name && Some(name) != old)
            {
                return Err(invalid("A group with this name already exists."));
            }
            if let Some(old) = old {
                let option = options
                    .iter_mut()
                    .find(|v| option_name(v) == old)
                    .ok_or_else(|| invalid("Group option is unavailable."))?;
                if let Some(object) = option.as_object_mut() {
                    object.insert("name".into(), json!(name));
                } else {
                    *option = json!(name);
                }
                let mut after = String::new();
                loop {
                    let records: Vec<Record> = {
                        let mut statement=tx.prepare("SELECT data FROM records WHERE database_id=?1 AND id>?2 ORDER BY id LIMIT 256")?;
                        let rows = statement
                            .query_map(params![database_id, after], |r| r.get::<_, String>(0))?;
                        rows.map(|r| decode(r?)).collect::<Result<_>>()?
                    };
                    if records.is_empty() {
                        break;
                    }
                    after = records.last().unwrap().id.clone();
                    let mut changed = Vec::new();
                    for mut record in records {
                        if record.values.get(&expected.id).and_then(Value::as_str) != Some(old) {
                            continue;
                        }
                        record.values.insert(expected.id.clone(), json!(name));
                        record.updated_at = now();
                        let values = encode(&record.values)?;
                        tx.execute(
                            "UPDATE records SET values_json=?1,data=?2 WHERE id=?3",
                            params![values, encode(&record)?, record.id],
                        )?;
                        if record.deleted_at.is_none() {
                            let target =
                                search::page(tx, record.page_id.as_deref().unwrap_or(&owner.id))?
                                    .ok_or_else(|| invalid("Card page is unavailable."))?;
                            search::index(
                                tx,
                                &format!("record:{}", record.id),
                                &target,
                                &format!("{} {values}", record.title),
                            )?;
                        }
                        changed.push(record.id);
                    }
                    if !changed.is_empty() {
                        crate::calculation::refresh(tx, &changed)?;
                    }
                }
                for view in &mut db.views {
                    if let Some(filter) = &mut view.query.filter {
                        rename_filter(filter, &expected.id, old, name);
                    }
                    let primary = view.query.group_by.as_deref().or(default_group.as_deref())
                        == Some(expected.id.as_str());
                    let secondary = view.config.get("board_subgroup_by").and_then(Value::as_str)
                        == Some(expected.id.as_str());
                    for field in [
                        "board_group_order",
                        "board_hidden_groups",
                        "board_collapsed_groups",
                        "board_subgroup_order",
                        "board_hidden_subgroups",
                        "board_collapsed_subgroups",
                    ] {
                        if (field.contains("subgroup") && !secondary)
                            || (!field.contains("subgroup") && !primary)
                        {
                            continue;
                        }
                        if let Some(items) =
                            view.config.get_mut(field).and_then(Value::as_array_mut)
                        {
                            for item in items {
                                if item.as_str() == Some(&group_key(&json!(old))) {
                                    *item = json!(group_key(&json!(name)));
                                }
                            }
                        }
                    }
                }
            } else {
                options.push(json!(name));
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod rank_tests {
    use super::rank_between;
    #[test]
    fn rank_keys_stay_between_neighbors_at_both_edges() {
        let mut high = vec![0, 1];
        for _ in 0..1000 {
            let next = rank_between(None, Some(&high)).unwrap();
            assert!(next < high);
            high = next;
        }
        let mut low = vec![254, 255];
        for _ in 0..1000 {
            let next = rank_between(Some(&low), None).unwrap();
            assert!(next > low);
            low = next;
        }
        assert!(rank_between(Some(&low), Some(&high)).is_err());
    }
}
