use crate::{
    FilterExpr, FilterOp, FilterRule, PageKind, Property, PropertyKind, Record, Store, ViewKind,
    search,
    store::{decode, filter_sql, invalid},
};
use chrono::{Datelike, NaiveDate};
use rusqlite::{Result, params_from_iter, types::Value as SqlValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const MAX_GROUPS: usize = 128;
const MAX_SAMPLES: usize = 40;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    #[default]
    VerticalBar,
    HorizontalBar,
    Line,
    Donut,
    Number,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Aggregate {
    #[default]
    Count,
    Sum,
    Average,
    Min,
    Max,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DateBucket {
    Day,
    Week,
    #[default]
    Month,
    Quarter,
    Year,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ChartConfig {
    pub kind: ChartType,
    pub category_property_id: Option<String>,
    pub value_property_id: Option<String>,
    pub aggregation: Aggregate,
    pub date_bucket: DateBucket,
    pub series_property_id: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ChartRecord {
    pub id: String,
    pub title: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ChartPoint {
    pub category: String,
    pub series: Option<String>,
    pub category_key: Option<String>,
    pub series_key: Option<String>,
    pub value: Option<f64>,
    pub count: u64,
    pub records: Vec<ChartRecord>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ChartProjection {
    pub config: ChartConfig,
    pub points: Vec<ChartPoint>,
    pub total: u64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ChartPage {
    pub records: Vec<ChartRecord>,
    pub next: Option<String>,
}

#[derive(Default)]
struct Group {
    count: u64,
    numeric_count: u64,
    sum: f64,
    min: Option<f64>,
    max: Option<f64>,
    records: Vec<ChartRecord>,
}
fn property<'a>(db: &'a crate::Database, id: &Option<String>) -> Result<Option<&'a Property>> {
    id.as_ref()
        .map(|id| {
            db.properties
                .iter()
                .find(|p| p.id == *id)
                .ok_or_else(|| invalid("Chart property is missing. Choose another property."))
        })
        .transpose()
}
fn record_value<'a>(record: &'a Record, property: &Property) -> Value {
    if property.kind == PropertyKind::Title {
        json!(record.title)
    } else {
        record
            .values
            .get(&property.id)
            .cloned()
            .unwrap_or(Value::Null)
    }
}
fn category(value: Value, property: &Property, bucket: DateBucket) -> Result<Option<String>> {
    if value.is_null() || value.as_str().is_some_and(str::is_empty) {
        return Ok(None);
    }
    if crate::calculation_error(&value).is_some() {
        return Err(invalid("A calculated value has an error."));
    }
    if property.kind == PropertyKind::Date {
        let raw = value
            .as_str()
            .or_else(|| value.get("start").and_then(Value::as_str))
            .ok_or_else(|| invalid("Invalid date value."))?;
        let date = NaiveDate::parse_from_str(raw.get(..10).unwrap_or(raw), "%Y-%m-%d")
            .map_err(|_| invalid("Invalid date value."))?;
        return Ok(Some(match bucket {
            DateBucket::Day => date.to_string(),
            DateBucket::Week => {
                let week = date.iso_week();
                format!("{}-W{:02}", week.year(), week.week())
            }
            DateBucket::Month => format!("{}-{:02}", date.year(), date.month()),
            DateBucket::Quarter => format!("{}-Q{}", date.year(), (date.month0() / 3) + 1),
            DateBucket::Year => date.year().to_string(),
        }));
    }
    match value {
        Value::String(s) => Ok(Some(s)),
        Value::Bool(b) => Ok(Some(b.to_string())),
        Value::Number(n) => Ok(Some(n.to_string())),
        _ => Err(invalid("This property cannot be used as a chart category.")),
    }
}
fn supported_category(p: &Property) -> bool {
    matches!(
        p.kind,
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
}
impl Store {
    /// Fetch one bounded page of a category, in stable record-ID order.
    pub fn chart_records(
        &self,
        database_id: &str,
        view_id: &str,
        config: &ChartConfig,
        extra_filter: Option<&FilterExpr>,
        title_search: &str,
        category_key: Option<&str>,
        series_key: Option<&str>,
        after: Option<&str>,
    ) -> Result<ChartPage> {
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("Database missing."))?;
        let owner = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("Database page missing."))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("Database is unavailable."));
        }
        let view = db
            .views
            .iter()
            .find(|v| v.id == view_id)
            .ok_or_else(|| invalid("View missing."))?;
        if view.kind != ViewKind::Chart {
            return Err(invalid("View is not a chart."));
        }
        let axis = property(&db, &config.category_property_id)?;
        let series = property(&db, &config.series_property_id)?;
        if config.kind != ChartType::Number && axis.is_none() {
            return Err(invalid("Choose a category property."));
        }
        if axis.is_some_and(|p| !supported_category(p))
            || series.is_some_and(|p| !supported_category(p))
        {
            return Err(invalid("Choose a supported category or series property."));
        }
        let mut args = vec![SqlValue::Text(database_id.into())];
        let mut sql =
            String::from("SELECT data FROM records WHERE database_id=? AND deleted_at IS NULL");
        for filter in [view.query.filter.as_ref(), extra_filter]
            .into_iter()
            .flatten()
        {
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(filter, &db, &mut args, 0)?);
        }
        if !title_search.is_empty() {
            let title = db
                .properties
                .iter()
                .find(|p| p.kind == PropertyKind::Title)
                .ok_or_else(|| invalid("Title property missing."))?;
            let search = FilterExpr::Rule(FilterRule {
                property_id: title.id.clone(),
                op: FilterOp::Contains,
                value: json!(title_search),
            });
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(&search, &db, &mut args, 0)?);
        }
        if let Some(after) = after {
            sql.push_str(" AND id > ?");
            args.push(SqlValue::Text(after.into()));
        }
        sql.push_str(" ORDER BY id ASC");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(args))?;
        let mut records = Vec::with_capacity(MAX_SAMPLES);
        let mut has_more = false;
        while let Some(row) = rows.next()? {
            let record: Record = decode(row.get(0)?)?;
            let axis_value = if config.kind == ChartType::Number {
                None
            } else {
                axis.map(|p| category(record_value(&record, p), p, config.date_bucket))
                    .transpose()?
                    .flatten()
            };
            let series_value = if config.kind == ChartType::Number {
                None
            } else {
                series
                    .map(|p| category(record_value(&record, p), p, config.date_bucket))
                    .transpose()?
                    .flatten()
            };
            if axis_value.as_deref() != category_key || series_value.as_deref() != series_key {
                continue;
            }
            if records.len() == MAX_SAMPLES {
                has_more = true;
                break;
            }
            records.push(ChartRecord {
                id: record.id,
                title: record.title,
            });
        }
        let next = if has_more {
            records.last().map(|r| r.id.clone())
        } else {
            None
        };
        Ok(ChartPage { records, next })
    }

    /// Stream the full matching query into a bounded projection; paging does not affect it.
    pub fn chart_projection(
        &self,
        database_id: &str,
        view_id: &str,
        config: &ChartConfig,
        extra_filter: Option<&FilterExpr>,
        title_search: &str,
    ) -> Result<ChartProjection> {
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("Database missing."))?;
        let owner = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("Database page missing."))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("Database is unavailable."));
        }
        let view = db
            .views
            .iter()
            .find(|v| v.id == view_id)
            .ok_or_else(|| invalid("View missing."))?;
        if view.kind != ViewKind::Chart {
            return Err(invalid("View is not a chart."));
        }
        let axis = property(&db, &config.category_property_id)?;
        let series = property(&db, &config.series_property_id)?;
        let value = property(&db, &config.value_property_id)?;
        if config.kind != ChartType::Number && axis.is_none() {
            return Err(invalid("Choose a category property."));
        }
        if axis.is_some_and(|p| !supported_category(p))
            || series.is_some_and(|p| !supported_category(p))
        {
            return Err(invalid("Choose a supported category or series property."));
        }
        if config.aggregation != Aggregate::Count
            && !value.is_some_and(|p| {
                matches!(
                    p.kind,
                    PropertyKind::Number | PropertyKind::Formula | PropertyKind::Rollup
                )
            })
        {
            return Err(invalid("Choose a numeric value property."));
        }
        let mut args = vec![SqlValue::Text(database_id.into())];
        let mut sql =
            String::from("SELECT data FROM records WHERE database_id=? AND deleted_at IS NULL");
        for filter in [view.query.filter.as_ref(), extra_filter]
            .into_iter()
            .flatten()
        {
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(filter, &db, &mut args, 0)?);
        }
        if !title_search.is_empty() {
            let title = db
                .properties
                .iter()
                .find(|p| p.kind == PropertyKind::Title)
                .ok_or_else(|| invalid("Title property missing."))?;
            let search = FilterExpr::Rule(FilterRule {
                property_id: title.id.clone(),
                op: FilterOp::Contains,
                value: json!(title_search),
            });
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(&search, &db, &mut args, 0)?);
        }
        sql.push_str(" ORDER BY id ASC");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(args))?;
        let mut groups: BTreeMap<(Option<String>, Option<String>), Group> = BTreeMap::new();
        let mut total = 0;
        while let Some(row) = rows.next()? {
            let record: Record = decode(row.get(0)?)?;
            let key = (
                if config.kind == ChartType::Number {
                    None
                } else {
                    axis.map(|p| category(record_value(&record, p), p, config.date_bucket))
                        .transpose()?
                        .flatten()
                },
                if config.kind == ChartType::Number {
                    None
                } else {
                    series
                        .map(|p| category(record_value(&record, p), p, config.date_bucket))
                        .transpose()?
                        .flatten()
                },
            );
            if !groups.contains_key(&key) && groups.len() >= MAX_GROUPS {
                return Err(invalid(
                    "Too many chart groups. Add a filter or choose another property.",
                ));
            }
            let group = groups.entry(key).or_default();
            group.count += 1;
            total += 1;
            if group.records.len() < MAX_SAMPLES {
                group.records.push(ChartRecord {
                    id: record.id.clone(),
                    title: record.title.clone(),
                });
            }
            if config.aggregation != Aggregate::Count {
                let v = record_value(&record, value.unwrap());
                if !v.is_null() {
                    let n = v
                        .as_f64()
                        .ok_or_else(|| invalid("A chart value is not numeric."))?;
                    group.numeric_count += 1;
                    group.sum += n;
                    if !group.sum.is_finite() {
                        return Err(invalid("Chart total is too large."));
                    }
                    group.min = Some(group.min.map_or(n, |m| m.min(n)));
                    group.max = Some(group.max.map_or(n, |m| m.max(n)));
                }
            }
        }
        if groups.is_empty() && config.kind == ChartType::Number {
            groups.insert((None, None), Group::default());
        }
        let points = groups
            .into_iter()
            .map(|((category, series), g)| ChartPoint {
                category_key: category.clone(),
                series_key: series.clone(),
                category: category.unwrap_or_else(|| {
                    if config.kind == ChartType::Number {
                        "Total".into()
                    } else {
                        "(Empty)".into()
                    }
                }),
                series: if config.kind != ChartType::Number && config.series_property_id.is_some() {
                    Some(series.unwrap_or_else(|| "(Empty)".into()))
                } else {
                    series
                },
                value: match config.aggregation {
                    Aggregate::Count => Some(g.count as f64),
                    Aggregate::Sum => (g.numeric_count > 0).then_some(g.sum),
                    Aggregate::Average => {
                        (g.numeric_count > 0).then(|| g.sum / g.numeric_count as f64)
                    }
                    Aggregate::Min => g.min,
                    Aggregate::Max => g.max,
                },
                count: g.count,
                records: g.records,
            })
            .collect();
        Ok(ChartProjection {
            config: config.clone(),
            points,
            total,
        })
    }
}
#[cfg(test)]
#[path = "chart_tests.rs"]
mod tests;
