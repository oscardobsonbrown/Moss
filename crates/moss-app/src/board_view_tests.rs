use super::*;
use moss_core::{PropertyKind, new_id};
use rune::{Env, Runtime};
use std::time::{Duration, Instant};

fn fixture() -> (State, PathBuf, String) {
    let path = std::env::temp_dir().join(format!("moss-board-controller-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    state.new_page(true, None);
    assert!(state.select_database_view(1));
    let db = state.tab().unwrap().database.as_ref().unwrap().clone();
    (state, path, db.id)
}
fn wait(rt: &mut Runtime<State>, test: impl Fn(&State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !test(&rt.state) {
        assert!(
            Instant::now() < deadline,
            "Board reader did not finish: {:?}",
            rt.state.database_views.board.error
        );
        rt.frame(1. / 120.);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn board_reader_covers_off_page_records_and_controller_moves_and_trashes() {
    let (mut state, path, dbid) = fixture();
    let db = state.tab().unwrap().database.as_ref().unwrap().clone();
    let group = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .clone();
    for i in 0..125 {
        state
            .store
            .create_record(
                &dbid,
                &format!("Task {i}"),
                &[(group.id.clone(), json!("Not started"))].into(),
            )
            .unwrap();
    }
    assert!(state.query_records());
    let mut rt = Runtime::new(state, crate::ui::view, (1100., 800.), Env::default());
    rt.poll = Some(crate::poll);
    rt.frame(0.);
    wait(&mut rt, |s| {
        s.database_views
            .board
            .projection
            .as_ref()
            .is_some_and(|p| p.total == 125)
    });
    let projection = rt.state.database_views.board.projection.as_ref().unwrap();
    let bucket = projection.buckets.iter().find(|b| b.total > 0).unwrap();
    assert_eq!(bucket.records.len(), 50);
    let record = bucket.records[0].clone();
    rt.update(|s, _| {
        assert!(s.commit_board_move(&record, &group, None, json!("Done"), Value::Null, None));
        // Consecutive moves before the next reader frame must use the committed rank.
        let moved = s
            .database_views
            .board
            .projection
            .as_ref()
            .unwrap()
            .buckets
            .iter()
            .flat_map(|b| &b.records)
            .find(|r| r.id == record.id)
            .unwrap()
            .clone();
        assert!(s.commit_board_move(
            &moved,
            &group,
            None,
            json!("In progress"),
            Value::Null,
            None
        ));
        let moved = s
            .database_views
            .board
            .projection
            .as_ref()
            .unwrap()
            .buckets
            .iter()
            .flat_map(|b| &b.records)
            .find(|r| r.id == record.id)
            .unwrap()
            .clone();
        assert!(s.commit_board_move(&moved, &group, None, json!("Done"), Value::Null, None));
    });
    rt.frame(0.);
    wait(&mut rt, |s| {
        !s.database_views.board.loading
            && s.database_views
                .board
                .projection
                .as_ref()
                .unwrap()
                .buckets
                .iter()
                .any(|b| b.group == json!("Done") && b.total == 1)
    });
    assert_eq!(
        rt.state
            .store
            .get_record(&record.id)
            .unwrap()
            .unwrap()
            .values[&group.id],
        json!("Done")
    );
    rt.update(|s, _| {
        assert!(s.trash_board_card(&record.id));
    });
    rt.frame(0.);
    wait(&mut rt, |s| {
        s.database_views
            .board
            .projection
            .as_ref()
            .is_some_and(|p| p.total == 124)
    });
    assert!(
        rt.state
            .store
            .get_record(&record.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some()
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn board_frozen_snapshot_does_not_reload_or_write_until_released() {
    let (mut state, path, dbid) = fixture();
    state
        .store
        .create_record(&dbid, "First", &Default::default())
        .unwrap();
    let mut rt = Runtime::new(state, crate::ui::view, (850., 600.), Env::default());
    rt.poll = Some(crate::poll);
    rt.frame(0.);
    wait(&mut rt, |s| {
        s.database_views
            .board
            .projection
            .as_ref()
            .is_some_and(|p| p.total == 1)
    });
    let changes = rt.state.store.local_change_count();
    rt.update(|s, _| {
        s.database_views.board.frozen = true;
        s.database_views.board.invalidate();
    });
    for _ in 0..10 {
        rt.frame(1. / 120.);
    }
    assert_eq!(
        rt.state
            .database_views
            .board
            .projection
            .as_ref()
            .unwrap()
            .total,
        1
    );
    assert_eq!(rt.state.store.local_change_count(), changes);
    assert!(!rt.state.database_views.board.loading);
    rt.update(|s, _| {
        s.database_views.board.frozen = false;
    });
    rt.frame(0.);
    wait(&mut rt, |s| !s.database_views.board.loading);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn board_last_page_clamps_after_its_final_card_is_trashed() {
    let (mut state, path, dbid) = fixture();
    let group = state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    for i in 0..51 {
        state
            .store
            .create_record(
                &dbid,
                &format!("Task {i}"),
                &[(group.clone(), json!("Not started"))].into(),
            )
            .unwrap();
    }
    let mut rt = Runtime::new(state, crate::ui::view, (850., 600.), Env::default());
    rt.poll = Some(crate::poll);
    rt.frame(0.);
    wait(&mut rt, |s| {
        s.database_views
            .board
            .projection
            .as_ref()
            .is_some_and(|p| p.total == 51)
    });
    rt.update(|s, _| s.page_board_bucket(&json!("Not started"), &Value::Null, true));
    rt.frame(0.);
    wait(&mut rt, |s| {
        !s.database_views.board.loading
            && s.database_views
                .board
                .projection
                .as_ref()
                .unwrap()
                .buckets
                .iter()
                .any(|bucket| bucket.offset == 50)
    });
    let last = rt
        .state
        .database_views
        .board
        .projection
        .as_ref()
        .unwrap()
        .buckets
        .iter()
        .find(|bucket| bucket.offset == 50)
        .unwrap()
        .records[0]
        .id
        .clone();
    rt.update(|s, _| {
        assert!(s.trash_board_card(&last));
    });
    rt.frame(0.);
    wait(&mut rt, |s| {
        !s.database_views.board.loading
            && s.database_views.board.projection.as_ref().is_some_and(|p| {
                p.total == 50
                    && p.buckets
                        .iter()
                        .any(|bucket| bucket.offset == 0 && bucket.records.len() == 50)
            })
    });
    assert_eq!(
        rt.state
            .database_views
            .board
            .offsets
            .get(&bucket_key(&json!("Not started"), &Value::Null)),
        None
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
