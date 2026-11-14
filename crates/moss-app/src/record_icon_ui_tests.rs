use super::*;
use chrono::Local;
use moss_core::PropertyKind;
use serde_json::json;

fn icon_label(rt: &Runtime<State>, view: &str) -> String {
    let key = if view == "board" {
        format!("board-icon-{}", rt.state.tab().unwrap().records[0].id)
    } else {
        format!("record-icon-{view}-0")
    };
    let icon = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(&key) && !e.exiting)
        .unwrap_or_else(|| panic!("missing {key}"));
    assert!((icon.target[2] - 16.).abs() < 0.1 && (icon.target[3] - 16.).abs() < 0.1);
    icon.label.unwrap()
}

fn all_views(rt: &mut Runtime<State>, label: &str) {
    for (view, key) in [
        (0, "table"),
        (1, "board"),
        (2, "calendar"),
        (3, "timeline"),
        (4, "gallery"),
        (5, "list"),
    ] {
        rt.update(|s, tx| {
            s.tabs[s.active].view = view;
            assert!(s.query_records());
            tx.instant();
        });
        rt.frame(1. / 120.);
        if key == "calendar" && !label.starts_with("Page icon ") {
            assert!(rt.find("record-icon-calendar-0").is_none());
        } else {
            assert_eq!(icon_label(rt, key), label, "view {key}");
        }
    }
}

#[test]
fn page_icons_follow_body_content_and_preserve_custom_icons_across_views() {
    let (mut rt, path) = database_runtime(0);
    let date = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    let values = [(date, json!(Local::now().date_naive().to_string()))].into();
    rt.state.create_inline(values, false);
    let id = rt.state.tab().unwrap().records[0].id.clone();
    rt.state.cell.replace_text("Metadata only");
    assert!(rt.state.finish_cell(false));
    rt.invalidate();
    rt.frame(1. / 120.);
    all_views(&mut rt, "Empty page");
    for (view, key) in [(0, "table"), (1, "board"), (5, "list")] {
        rt.update(|s, tx| {
            s.tabs[s.active].view = view;
            s.query_records();
            s.edit_record(0, None);
            tx.instant();
        });
        rt.frame(1. / 120.);
        assert_eq!(icon_label(&rt, key), "Empty page");
        press(&mut rt, Key::Escape);
    }

    rt.update(|s, tx| {
        s.tabs[s.active].view = 0;
        s.query_records();
        s.open_record(0);
        tx.instant();
    });
    rt.frame(1. / 120.);
    let page = rt.state.record_peek.as_ref().unwrap().page.id.clone();
    let block = rt.state.record_peek.as_ref().unwrap().blocks[0]
        .block
        .id
        .clone();
    click(&mut rt, &format!("peek-block-{block}"));
    press(&mut rt, Key::Text("Body text".into()));
    assert_eq!(icon_label(&rt, "table"), "Page with content");
    click(&mut rt, "peek-close");
    all_views(&mut rt, "Page with content");

    rt.update(|s, tx| {
        s.tabs[s.active].view = 0;
        s.query_records();
        s.open_record(0);
        tx.instant();
    });
    rt.frame(1. / 120.);
    click(&mut rt, &format!("peek-block-{block}"));
    press(&mut rt, Key::Command('a'));
    press(&mut rt, Key::Backspace);
    assert_eq!(icon_label(&rt, "table"), "Empty page");
    click(&mut rt, "peek-close");
    all_views(&mut rt, "Empty page");

    rt.update(|s, tx| {
        let mut page = s.store.get_page(&page).unwrap().unwrap();
        page.icon = "🌱".into();
        s.store.update_page(&page).unwrap();
        tx.instant();
    });
    rt.frame(1. / 120.);
    assert_eq!(rt.state.tab().unwrap().records[0].id, id);
    all_views(&mut rt, "Page icon 🌱");
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
