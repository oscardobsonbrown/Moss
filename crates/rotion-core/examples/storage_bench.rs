//! Build a disposable fixture, then time production store reads.
//!
//! Small smoke run:
//!   cargo run --release -p rotion-core --example storage_bench -- --small --path /tmp/rotion-bench-small.sqlite
//! Full run (100,000 pages, 1,000,000 records):
//!   cargo run --release -p rotion-core --example storage_bench -- --path /tmp/rotion-bench-full.sqlite
//! Optional: --samples N (default 30), --reuse to read an existing fixture. New paths must not exist.
//! The fixture is kept on disk for inspection. Remove it manually when done.
//! All samples follow fixture creation and a warm-up. OS cache is not controlled.
use rotion_core::{
    Block, BlockKind, Database, FilterExpr, FilterOp, FilterRule, Page, PageKind, Property,
    PropertyKind, Record, RecordQuery, SortRule, Store, Workspace,
};
use rusqlite::{Connection, params};
use serde::Serialize;
use serde_json::json;
use std::{collections::BTreeMap, env, error::Error, path::PathBuf, time::Instant};

const WORKSPACE: &str = "bench-workspace";
const DB_PAGE: &str = "bench-database-page";
const DATABASE: &str = "bench-database";
const TIME: &str = "2026-01-01T00:00:00Z";
const PAGE_BATCH: usize = 1_000;
const RECORD_BATCH: usize = 5_000;

fn json_string<T: Serialize>(v: &T) -> Result<String, Box<dyn Error>> {
    Ok(serde_json::to_string(v)?)
}
fn page_id(n: usize) -> String {
    format!("page-{n:09}")
}
fn page(id: String, title: String, kind: PageKind) -> Page {
    Page {
        id,
        workspace_id: WORKSPACE.into(),
        parent_id: None,
        title,
        icon: "📄".into(),
        kind,
        created_at: TIME.into(),
        updated_at: TIME.into(),
        deleted_at: None,
    }
}
fn insert_page(tx: &rusqlite::Transaction<'_>, p: &Page) -> Result<(), Box<dyn Error>> {
    tx.execute("INSERT INTO pages(id,workspace_id,parent_id,deleted_at,trash_root,data) VALUES (?1,?2,NULL,NULL,NULL,?3)", params![p.id,p.workspace_id,json_string(p)?])?;
    tx.execute(
        "INSERT INTO search_fts(key,page_id,workspace_id,body) VALUES (?1,?2,?3,?4)",
        params![format!("page:{}", p.id), p.id, p.workspace_id, p.title],
    )?;
    Ok(())
}
fn populate(path: &PathBuf, page_count: usize, record_count: usize) -> Result<(), Box<dyn Error>> {
    // Store initializes the production schema. Direct SQL only creates the fixture.
    drop(Store::open(path)?);
    let mut conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;")?;
    let workspace = Workspace {
        id: WORKSPACE.into(),
        name: "Performance fixture".into(),
        icon: "📚".into(),
        created_at: TIME.into(),
    };
    conn.execute(
        "INSERT INTO workspaces(id,data) VALUES (?1,?2)",
        params![WORKSPACE, json_string(&workspace)?],
    )?;
    let db_page = page(DB_PAGE.into(), "Work items".into(), PageKind::Database);
    let db = Database {
        id: DATABASE.into(),
        page_id: DB_PAGE.into(),
        name: "Work items".into(),
        properties: vec![
            Property {
                id: "title".into(),
                name: "Name".into(),
                kind: PropertyKind::Title,
                config: json!({}),
            },
            Property {
                id: "status".into(),
                name: "Status".into(),
                kind: PropertyKind::Status,
                config: json!({"options":["Open","Active","Done"]}),
            },
            Property {
                id: "date".into(),
                name: "Date".into(),
                kind: PropertyKind::Date,
                config: json!({}),
            },
            Property {
                id: "notes".into(),
                name: "Notes".into(),
                kind: PropertyKind::Text,
                config: json!({}),
            },
        ],
        views: vec![],
    };
    {
        let tx = conn.transaction()?;
        insert_page(&tx, &db_page)?;
        tx.execute(
            "INSERT INTO databases(id,page_id,data) VALUES (?1,?2,?3)",
            params![DATABASE, DB_PAGE, json_string(&db)?],
        )?;
        tx.commit()?;
    }
    // The database page is included in the requested page count.
    for start in (0..page_count - 1).step_by(PAGE_BATCH) {
        let tx = conn.transaction()?;
        for n in start..(start + PAGE_BATCH).min(page_count - 1) {
            let p = page(
                page_id(n),
                format!("Project note {n:09}"),
                PageKind::Document,
            );
            insert_page(&tx, &p)?;
            for order in 0..3 {
                let b = Block {
                    id: format!("block-{n:09}-{order}"),
                    page_id: p.id.clone(),
                    kind: if order == 0 {
                        BlockKind::Heading2
                    } else {
                        BlockKind::Paragraph
                    },
                    text: format!(
                        "Project {n}: planning notes, decisions, and next steps for milestone {order}."
                    ),
                    checked: false,
                    depth: 0,
                    sort_order: order,
                    config: json!({}),
                };
                tx.execute(
                    "INSERT INTO blocks(id,page_id,sort_order,data) VALUES (?1,?2,?3,?4)",
                    params![b.id, b.page_id, b.sort_order, json_string(&b)?],
                )?;
                tx.execute(
                    "INSERT INTO search_fts(key,page_id,workspace_id,body) VALUES (?1,?2,?3,?4)",
                    params![format!("block:{}", b.id), p.id, WORKSPACE, b.text],
                )?;
            }
        }
        tx.commit()?;
    }
    for start in (0..record_count).step_by(RECORD_BATCH) {
        let tx = conn.transaction()?;
        for n in start..(start + RECORD_BATCH).min(record_count) {
            let mut values = BTreeMap::new();
            values.insert("status".into(), json!(["Open", "Active", "Done"][n % 3]));
            values.insert(
                "date".into(),
                json!(format!("2026-{:02}-{:02}", n % 12 + 1, n % 28 + 1)),
            );
            values.insert(
                "notes".into(),
                json!(format!(
                    "Work item {n}: review the plan and record the next action."
                )),
            );
            let record = Record {
                id: format!("record-{n:09}"),
                database_id: DATABASE.into(),
                page_id: None,
                title: format!("Work item {n:09}"),
                values,
                created_at: TIME.into(),
                updated_at: TIME.into(),
                deleted_at: None,
            };
            let values = json_string(&record.values)?;
            tx.execute("INSERT INTO records(id,database_id,page_id,title,values_json,deleted_at,data) VALUES (?1,?2,NULL,?3,?4,NULL,?5)", params![record.id,DATABASE,record.title,values,json_string(&record)?])?;
            tx.execute(
                "INSERT INTO search_fts(key,page_id,workspace_id,body) VALUES (?1,?2,?3,?4)",
                params![
                    format!("record:{}", record.id),
                    DB_PAGE,
                    WORKSPACE,
                    format!("{} {}", record.title, values)
                ],
            )?;
        }
        tx.commit()?;
    }
    Ok(())
}

fn report(name: &str, times: &mut [f64], rows: usize) {
    times.sort_by(f64::total_cmp);
    let percentile = |pct: usize| times[((times.len() * pct).div_ceil(100)).saturating_sub(1)];
    println!(
        "{name}: samples={} returned_rows={rows} p50={:.3}ms p95={:.3}ms p99={:.3}ms max={:.3}ms",
        times.len(),
        percentile(50),
        percentile(95),
        percentile(99),
        times[times.len() - 1]
    );
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut path = None;
    let mut small = false;
    let mut reuse = false;
    let mut samples = 30usize;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--small" => small = true,
            "--reuse" => reuse = true,
            "--path" => path = Some(PathBuf::from(args.next().ok_or("--path needs a value")?)),
            "--samples" => samples = args.next().ok_or("--samples needs a value")?.parse()?,
            "--help" | "-h" => {
                println!(
                    "Usage: storage_bench --path NEW_FILE [--small] [--samples N] [--reuse]\nThe full fixture has 100,000 pages and 1,000,000 records. The file is kept."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    if samples == 0 || samples > 10_000 {
        return Err("samples must be between 1 and 10000".into());
    }
    let path = path.ok_or("--path is required and must not exist")?;
    // Fail instead of overwriting any existing user data.
    if !reuse {
        drop(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?,
        );
    }
    let (pages, records) = if small {
        (100usize, 1_000usize)
    } else {
        (100_000, 1_000_000)
    };
    let setup = Instant::now();
    if !reuse {
        populate(&path, pages, records)?;
    }
    let check = Connection::open(&path)?;
    let actual_pages: i64 = check.query_row("SELECT count(*) FROM pages", [], |r| r.get(0))?;
    let actual_blocks: i64 = check.query_row("SELECT count(*) FROM blocks", [], |r| r.get(0))?;
    let actual_records: i64 = check.query_row("SELECT count(*) FROM records", [], |r| r.get(0))?;
    if (actual_pages, actual_blocks, actual_records)
        != (pages as i64, ((pages - 1) * 3) as i64, records as i64)
    {
        return Err("fixture counts do not match the requested scale".into());
    }
    drop(check);
    println!(
        "fixture: pages={pages}, blocks={}, records={records}, path={}, setup={:.2}s",
        (pages - 1) * 3,
        path.display(),
        setup.elapsed().as_secs_f64()
    );
    println!("cache: OS cache is uncontrolled; samples follow warm-up. No uncached claim.");
    let start = Instant::now();
    let mut store = Store::open(&path)?;
    println!(
        "store open: {:.3}ms (OS cache not controlled)",
        start.elapsed().as_secs_f64() * 1000.0
    );
    let sidebar_start = Instant::now();
    let sidebar = store.list_pages(WORKSPACE)?;
    println!(
        "list all sidebar pages: returned_rows={} time={:.3}ms",
        sidebar.len(),
        sidebar_start.elapsed().as_secs_f64() * 1000.
    );
    drop(sidebar);
    let title_query = RecordQuery {
        sorts: vec![SortRule {
            property_id: "title".into(),
            descending: false,
        }],
        limit: 50,
        ..RecordQuery::default()
    };
    let filtered_query = RecordQuery {
        filter: Some(FilterExpr::Rule(FilterRule {
            property_id: "status".into(),
            op: FilterOp::Eq,
            value: json!("Active"),
        })),
        limit: 50,
        ..title_query.clone()
    };
    let deep_query = RecordQuery {
        offset: (records / 2) as u64,
        ..title_query.clone()
    };
    let mut page_times = Vec::with_capacity(samples);
    let mut block_times = Vec::with_capacity(samples);
    let mut viewport_times = Vec::with_capacity(samples);
    let mut title_times = Vec::with_capacity(samples);
    let mut filter_times = Vec::with_capacity(samples);
    let mut deep_times = Vec::with_capacity(samples);
    let mut write_times = Vec::with_capacity(samples);
    let mut returned = [0usize; 6];
    // The first iteration is a warm-up and is excluded from all percentiles.
    for i in 0..=samples {
        let id = page_id((i * 7919) % (pages - 1));
        let t = Instant::now();
        let p = store.get_page(&id)?;
        let page_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let b = store.get_blocks(&id)?;
        let block_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let vp = store.get_page(&id)?;
        let vb = store.get_blocks(&id)?;
        let viewport_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let a = store.query_records(DATABASE, &title_query)?;
        let title_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let f = store.query_records(DATABASE, &filtered_query)?;
        let filter_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let d = store.query_records(DATABASE, &deep_query)?;
        let deep_ms = t.elapsed().as_secs_f64() * 1000.0;
        if p.is_none()
            || vp.is_none()
            || b.len() != 3
            || vb.len() != 3
            || a.len() != 50
            || f.len() != 50
            || d.len() != 50
        {
            return Err("fixture read returned unexpected row count".into());
        }
        if i > 0 {
            page_times.push(page_ms);
            block_times.push(block_ms);
            viewport_times.push(viewport_ms);
            title_times.push(title_ms);
            filter_times.push(filter_ms);
            deep_times.push(deep_ms);
            returned = [1, b.len(), 1 + vb.len(), a.len(), f.len(), d.len()];
        }
    }
    let edit_id = page_id(0);
    let original = store.get_blocks(&edit_id)?;
    for i in 0..=samples {
        let mut edited = original.clone();
        edited[0]
            .text
            .push_str(if i % 2 == 0 { " edit" } else { " saved" });
        let start = Instant::now();
        store.save_blocks(&edit_id, &edited)?;
        if i > 0 {
            write_times.push(start.elapsed().as_secs_f64() * 1000.);
        }
    }
    store.save_blocks(&edit_id, &original)?;
    report("save 3 blocks and update FTS", &mut write_times, 3);
    report("get_page", &mut page_times, returned[0]);
    report("get_blocks", &mut block_times, returned[1]);
    report("page plus blocks", &mut viewport_times, returned[2]);
    report("title-sorted first 50", &mut title_times, returned[3]);
    report(
        "status-filtered title-sorted first 50",
        &mut filter_times,
        returned[4],
    );
    report(
        "title-sorted 50 at halfway offset",
        &mut deep_times,
        returned[5],
    );
    Ok(())
}
