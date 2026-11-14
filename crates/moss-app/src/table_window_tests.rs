use super::*;
use crate::{key, records, ui};
use moss_core::{RecordQuery, new_id};
use rune::{Env, Runtime};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn fixture(rows: usize) -> (State, PathBuf) {
    let path = std::env::temp_dir().join(format!("moss-window-test-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    state.new_name.replace_text("Window study");
    state.new_page(true, None);
    let db = state.tab().unwrap().database.as_ref().unwrap().id.clone();
    for i in 0..rows {
        state
            .store
            .create_record(&db, &format!("Row {i:04}"), &Default::default())
            .unwrap();
    }
    assert!(state.query_records());
    (state, path)
}
fn wait_until(rt: &mut Runtime<State>, test: impl Fn(&State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !test(&rt.state) {
        assert!(
            Instant::now() < deadline,
            "window did not finish: {:?}",
            rt.state
                .tab()
                .unwrap()
                .table_window
                .as_ref()
                .and_then(|w| w.error.as_deref())
        );
        rt.frame(1. / 120.);
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn indexed_window_reaches_last_row_and_keeps_cache_bounded() {
    let (state, path) = fixture(1400);
    let db_id = state.tab().unwrap().database.as_ref().unwrap().id.clone();
    let last = state
        .store
        .query_records(
            &db_id,
            &RecordQuery {
                offset: 1399,
                limit: 1,
                ..Default::default()
            },
        )
        .unwrap()[0]
        .id
        .clone();
    let mut rt = Runtime::new(
        state,
        ui::view,
        (1000., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.poll = Some(records::poll);
    rt.frame(0.);
    wait_until(&mut rt, |s| {
        s.tab().unwrap().table_window.as_ref().unwrap().total == Some(1400)
    });
    assert!(rt.state.tab().unwrap().records.len() <= WINDOW);
    rt.update(|_, tx| tx.scroll_to_y("db-body", 1400. * ROW));
    rt.frame(1. / 120.);
    wait_until(&mut rt, |s| {
        let tab = s.tab().unwrap();
        let w = tab.table_window.as_ref().unwrap();
        w.start > 0 && tab.records[..w.window_len].iter().any(|r| r.id == last)
    });
    let tab = rt.state.tab().unwrap();
    assert!(tab.records.len() <= WINDOW + 100);
    assert!(tab.table_window.as_ref().unwrap().visible.end <= 1400);
    assert!(rt.elements().iter().any(|e| {
        e.key
            .as_deref()
            .is_some_and(|k| k.starts_with("cell-") && k.ends_with("-title"))
    }));
    rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., f32::MAX));
    rt.frame(1. / 120.);
    let local = rt.state.tab().unwrap().records.iter().position(|r| r.id == last).unwrap();
    let key = format!("cell-{local}-title");
    assert!(rt.find(&key).is_some(), "last record must remain visible after a cache change");
    let (x, y) = rt.find(&key).unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.);
    assert_eq!(rt.state.editing_cell.as_ref().map(|cell| &cell.record_id), Some(&last));
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn cache_change_keeps_the_scroll_position() {
    let (state, path) = fixture(1400);
    let mut rt = Runtime::new(state, ui::view, (1000., 700.), Env { system_dark: false, scale: 1. });
    rt.poll = Some(records::poll);
    rt.frame(0.);
    wait_until(&mut rt, |s| s.tab().unwrap().table_window.as_ref().unwrap().total == Some(1400));
    rt.poll = None;
    rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., 700. * ROW));
    rt.frame(1. / 120.);
    let before = rt.state.tab().unwrap().table_window.as_ref().unwrap().scroll.get().offset[1];
    rt.poll = Some(records::poll);
    wait_until(&mut rt, |s| s.tab().unwrap().table_window.as_ref().unwrap().start > 0);
    rt.frame(1. / 120.);
    let after = rt.state.tab().unwrap().table_window.as_ref().unwrap().scroll.get().offset[1];
    assert!((after - before).abs() < 2., "cache change moved the scroll from {before} to {after}");
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn bottom_anchor_does_not_override_a_later_scroll() {
    let (state, path) = fixture(1400);
    let mut rt = Runtime::new(
        state,
        ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 2.,
        },
    );
    rt.poll = Some(records::poll);
    rt.frame(0.);
    wait_until(&mut rt, |s| {
        s.tab().unwrap().table_window.as_ref().unwrap().total == Some(1400)
    });
    rt.update(|_, tx| tx.scroll_to_y("db-body", f32::MAX));
    rt.frame(1. / 120.);
    wait_until(&mut rt, |s| {
        s.tab().unwrap().table_window.as_ref().unwrap().start > 0
    });
    rt.frame(1. / 120.);
    let bottom = rt
        .state
        .tab()
        .unwrap()
        .table_window
        .as_ref()
        .unwrap()
        .scroll
        .get()
        .offset[1];
    rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., bottom - 500.));
    rt.frame(1. / 120.);
    rt.state.tabs[rt.state.active]
        .table_window
        .as_mut()
        .unwrap()
        .bottom_anchor = Some(bottom);
    rt.frame(1. / 120.);
    let actual = rt
        .state
        .tab()
        .unwrap()
        .table_window
        .as_ref()
        .unwrap()
        .scroll
        .get()
        .offset[1];
    assert!(
        actual < bottom - 100.,
        "later scroll was overridden: {actual}, bottom {bottom}"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn stale_response_is_rejected_after_a_local_write() {
    let (mut state, path) = fixture(1);
    let before = state.tab().unwrap().records[0].clone();
    let window = state.tabs[state.active].table_window.as_mut().unwrap();
    window.request_id = 7;
    let generation = window.generation;
    let active = state.active;
    mark_changed(&mut state, active);
    let mut stale = before.clone();
    stale.title = "Stale".into();
    let response = Response {
        page_id: state.tab().unwrap().page.id.clone(),
        generation,
        request_id: 7,
        result: Ok(Some(Loaded {
            window: RecordWindow {
                total: 1,
                revision: 1,
                start: 0,
                records: vec![stale],
            },
            icons: BTreeMap::new(),
            content: BTreeMap::new(),
            titles: BTreeMap::new(),
        })),
    };
    let active = state.active;
    assert!(!install(&mut state, active, response));
    assert_eq!(state.tab().unwrap().records[0], before);
    assert!(
        state
            .tab()
            .unwrap()
            .table_window
            .as_ref()
            .unwrap()
            .invalidate_pending
    );
    drop(state);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn unsupported_view_returns_to_paging_and_disconnected_worker_is_retryable() {
    let (mut state, path) = fixture(1);
    assert!(state.tab().unwrap().table_window.is_some());
    let active = state.active;
    state.tabs[active].database.as_mut().unwrap().views[0].config["wrap_cells"] =
        serde_json::json!(true);
    assert!(state.query_records());
    assert!(state.tab().unwrap().table_window.is_none());
    state.tabs[active].database.as_mut().unwrap().views[0].config["wrap_cells"] =
        serde_json::json!(false);
    assert!(state.query_records());
    assert!(state.tab().unwrap().table_window.is_some());
    let (sender, receiver) = mpsc::channel::<Response>();
    drop(sender);
    state.table_window_jobs.receiver = Some(receiver);
    state.table_window_jobs.in_flight = 1;
    state.tabs[active].table_window.as_mut().unwrap().loading = true;
    let mut tx = Tx::default();
    assert!(poll(&mut state, &mut tx));
    let window = state.tab().unwrap().table_window.as_ref().unwrap();
    assert!(!window.loading);
    assert!(window.error.as_deref().unwrap().contains("Retry"));
    assert_eq!(state.table_window_jobs.in_flight, 0);
    drop(state);
    std::fs::remove_dir_all(path).unwrap();
}

fn response(page_id: String, generation: u64, request_id: u64, rows: Vec<Record>) -> Response {
    Response {
        page_id,
        generation,
        request_id,
        result: Ok(Some(Loaded {
            window: RecordWindow {
                total: 6,
                revision: 1,
                start: 0,
                records: rows,
            },
            icons: BTreeMap::new(),
            content: BTreeMap::new(),
            titles: BTreeMap::new(),
        })),
    }
}

#[test]
fn new_draft_count_waits_for_fresh_rank_without_double_counting() {
    let (state, path) = fixture(5);
    let mut rt = Runtime::new(
        state,
        ui::view,
        (1000., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.poll = Some(records::poll);
    rt.frame(0.);
    wait_until(&mut rt, |s| {
        s.tab().unwrap().table_window.as_ref().unwrap().total == Some(5)
    });
    let active = rt.state.active;
    let page_id = rt.state.tab().unwrap().page.id.clone();
    let old_rows = rt.state.tab().unwrap().records.clone();
    let window = rt.state.tabs[active].table_window.as_mut().unwrap();
    window.request_id = 55;
    let generation = window.generation;
    rt.update(|s, _| s.create_inline(Default::default(), false));
    rt.frame(0.);
    let window = rt.state.tab().unwrap().table_window.as_ref().unwrap();
    assert_eq!(window.total, Some(5));
    assert_eq!(window.displayed_total(), Some(6));
    assert!(
        window.visible.end <= 5,
        "the pending count must not invent a ranked row"
    );
    assert_eq!(
        rt.elements()
            .into_iter()
            .find(|node| node.key.as_deref() == Some("database-page-count"))
            .unwrap()
            .text
            .as_deref(),
        Some("6 records")
    );
    assert!(!install(
        &mut rt.state,
        active,
        response(page_id.clone(), generation, 55, old_rows)
    ));
    assert_eq!(
        rt.state
            .tab()
            .unwrap()
            .table_window
            .as_ref()
            .unwrap()
            .displayed_total(),
        Some(6)
    );
    assert!(rt.state.finish_cell(false));
    wait_until(&mut rt, |s| {
        let window = s.tab().unwrap().table_window.as_ref().unwrap();
        window.total == Some(6) && window.pending_created == 0
    });
    assert_eq!(
        rt.state
            .tab()
            .unwrap()
            .table_window
            .as_ref()
            .unwrap()
            .displayed_total(),
        Some(6)
    );
    rt.state.store.delete_page(&page_id).unwrap();
    rt.state.create_inline(Default::default(), false);
    assert!(
        rt.state
            .error
            .as_deref()
            .is_some_and(|error| error.contains("Could not create"))
    );
    assert_eq!(
        rt.state
            .tab()
            .unwrap()
            .table_window
            .as_ref()
            .unwrap()
            .displayed_total(),
        Some(6)
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn cache_swaps_preserve_created_rows_and_defer_active_drafts() {
    let (mut state, path) = fixture(5);
    let original = state.tab().unwrap().records.clone();
    state.create_inline(Default::default(), false);
    state.cell.replace_text("Draft remains");
    let created = state.editing_cell.as_ref().unwrap().record_id.clone();
    let page_id = state.tab().unwrap().page.id.clone();
    let active = state.active;
    let window = state.tabs[active].table_window.as_mut().unwrap();
    window.request_id = 7;
    let generation = window.generation;
    assert!(!install(
        &mut state,
        active,
        response(page_id.clone(), generation, 7, original[..2].to_vec())
    ));
    // A response from before creation is rejected. A fresh response is deferred while editing.
    assert!(
        state
            .tab()
            .unwrap()
            .table_window
            .as_ref()
            .unwrap()
            .deferred
            .is_none()
    );
    state.tabs[active]
        .table_window
        .as_mut()
        .unwrap()
        .invalidate_pending = false;
    assert!(!install(
        &mut state,
        active,
        response(page_id.clone(), generation, 7, original[..2].to_vec())
    ));
    assert_eq!(state.cell.text, "Draft remains");
    assert!(
        state
            .tab()
            .unwrap()
            .table_window
            .as_ref()
            .unwrap()
            .deferred
            .is_some()
    );
    assert!(state.finish_cell(false));
    let deferred = state.tabs[active]
        .table_window
        .as_mut()
        .unwrap()
        .deferred
        .take()
        .unwrap();
    assert!(install(&mut state, active, deferred));
    assert_eq!(
        state
            .tab()
            .unwrap()
            .records
            .iter()
            .filter(|r| r.id == created)
            .count(),
        1
    );
    let pinned = state
        .tab()
        .unwrap()
        .records
        .iter()
        .find(|r| r.id == created)
        .unwrap()
        .clone();
    let mut included = original[..1].to_vec();
    included.push(pinned);
    state.tabs[active].table_window.as_mut().unwrap().request_id = 8;
    assert!(install(
        &mut state,
        active,
        response(page_id.clone(), generation, 8, included)
    ));
    assert_eq!(
        state
            .tab()
            .unwrap()
            .records
            .iter()
            .filter(|r| r.id == created)
            .count(),
        1
    );
    state.tabs[active].table_window.as_mut().unwrap().request_id = 9;
    assert!(install(
        &mut state,
        active,
        response(page_id, generation, 9, original[..2].to_vec())
    ));
    assert_eq!(
        state
            .tab()
            .unwrap()
            .records
            .iter()
            .filter(|r| r.id == created)
            .count(),
        1
    );
    assert_eq!(
        state.tab().unwrap().records.last().unwrap().title,
        "Draft remains"
    );
    drop(state);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn dirty_peek_does_not_block_a_deep_window_or_lose_its_draft() {
    let (state, path) = fixture(1400);
    let mut rt = Runtime::new(
        state,
        ui::view,
        (1000., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.poll = Some(records::poll);
    rt.frame(0.);
    wait_until(&mut rt, |s| {
        s.tab().unwrap().table_window.as_ref().unwrap().total == Some(1400)
    });
    rt.update(|s, tx| {
        s.open_record(0);
        let peek = s.record_peek.as_mut().unwrap();
        peek.title.replace_text("Unsaved title");
        peek.dirty_title = true;
        peek.blocks[0].edit.replace_text("Unsaved body");
        peek.dirty_blocks = true;
        tx.instant();
    });
    rt.frame(1. / 120.);
    rt.update(|_, tx| tx.scroll_to_y("db-body", 1400. * ROW));
    rt.frame(1. / 120.);
    wait_until(&mut rt, |s| {
        s.tab().unwrap().table_window.as_ref().unwrap().start > 0
    });
    let peek = rt.state.record_peek.as_ref().unwrap();
    assert_eq!(peek.title.text, "Unsaved title");
    assert_eq!(peek.blocks[0].edit.text, "Unsaved body");
    assert!(peek.dirty_title && peek.dirty_blocks);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn dropping_state_interrupts_and_joins_an_active_loader() {
    let (mut state, path) = fixture(640);
    assert!(state.request_table_window(0..20));
    assert!(state.table_window_jobs.in_flight > 0);
    drop(state);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn scrolling_away_from_an_active_draft_keeps_its_row_and_text() {
    let (state, path) = fixture(640);
    let mut rt = Runtime::new(
        state,
        ui::view,
        (1000., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.poll = Some(records::poll);
    rt.frame(0.);
    wait_until(&mut rt, |s| {
        s.tab().unwrap().table_window.as_ref().unwrap().total == Some(640)
    });
    rt.update(|s, tx| {
        s.edit_record(0, None);
        s.cell.replace_text("Unsaved cell draft");
        tx.instant();
    });
    rt.frame(1. / 120.);
    let id = rt.state.editing_cell.as_ref().unwrap().record_id.clone();
    rt.update(|_, tx| tx.scroll_to_y("db-body", 640. * ROW));
    for _ in 0..5 {
        rt.frame(1. / 120.);
    }
    assert_eq!(rt.state.editing_cell.as_ref().unwrap().record_id, id);
    assert_eq!(rt.state.cell.text, "Unsaved cell draft");
    let window = rt.state.tab().unwrap().table_window.as_ref().unwrap();
    assert_eq!(window.start, 0);
    assert!(window.scroll.get().offset[1] < (WINDOW as f32 * ROW));
    assert!(rt.find("database-cell-editor").is_some());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn rapid_window_resets_and_close_prune_obsolete_pending_requests() {
    let (mut state, path) = fixture(1);
    for request_id in 0..200 {
        let tab = state.tab().unwrap();
        let window = tab.table_window.as_ref().unwrap();
        let request = Request {
            page_id: tab.page.id.clone(),
            database: tab.database.clone().unwrap(),
            workspace: state.workspace.id.clone(),
            generation: window.generation,
            request_id,
            start: 0,
            revision: None,
        };
        state.table_window_jobs.pending.insert(
            format!("{}:{}", request.page_id, request.generation),
            request,
        );
        state.tabs[state.active].table_window = Some(TableWindow::new(1));
        prune_pending(&mut state);
        assert!(state.table_window_jobs.pending.is_empty());
    }
    let tab = state.tab().unwrap();
    let window = tab.table_window.as_ref().unwrap();
    let request = Request {
        page_id: tab.page.id.clone(),
        database: tab.database.clone().unwrap(),
        workspace: state.workspace.id.clone(),
        generation: window.generation,
        request_id: 201,
        start: 0,
        revision: None,
    };
    state.table_window_jobs.pending.insert(
        format!("{}:{}", request.page_id, request.generation),
        request,
    );
    prune_pending(&mut state);
    assert_eq!(state.table_window_jobs.pending.len(), 1);
    state.tabs.clear();
    prune_pending(&mut state);
    assert!(state.table_window_jobs.pending.is_empty());
    drop(state);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn large_initial_limit_uses_bounded_window_and_still_reaches_the_end() {
    let (mut state, path) = fixture(640);
    let mut db = state.tab().unwrap().database.clone().unwrap();
    db.views[0].config["load_limit"] = serde_json::json!(999);
    state.store.save_database(&db).unwrap();
    let page_id = db.page_id.clone();
    state.tabs.clear();
    state.loading = true;
    state.open_page(&page_id, false);
    state.loading = false;
    assert_eq!(
        state.tab().unwrap().records.len(),
        INITIAL_WINDOW,
        "opening uses a bounded window"
    );
    state.tabs[state.active].database = Some(db);
    assert!(state.query_records());
    assert_eq!(state.tab().unwrap().records.len(), INITIAL_WINDOW);
    let mut rt = Runtime::new(
        state,
        ui::view,
        (1000., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.poll = Some(records::poll);
    rt.frame(0.);
    wait_until(&mut rt, |s| {
        s.tab().unwrap().table_window.as_ref().unwrap().total == Some(640)
    });
    rt.update(|_, tx| tx.scroll_to_y("db-body", 640. * ROW));
    rt.frame(1. / 120.);
    wait_until(&mut rt, |s| {
        s.tab().unwrap().table_window.as_ref().unwrap().visible.end == 640
            && !s.tab().unwrap().table_window.as_ref().unwrap().loading
    });
    assert!(rt.state.tab().unwrap().records.len() <= WINDOW);
    assert!(rt.find("table-new").is_some());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
