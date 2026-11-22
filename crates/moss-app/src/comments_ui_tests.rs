use super::*;
use moss_core::comments::CommentAnchor;

#[test]
fn selection_comment_can_be_created_replied_resolved_and_reopened() {
    let (mut s, path) = fixture();
    s.new_page(false, None);
    s.tabs[s.active].blocks[0]
        .edit
        .replace_text("A useful sentence");
    s.save_blocks();
    let page = s.tab().unwrap().page.id.clone();
    let block = s.tab().unwrap().blocks[0].block.id.clone();
    s.tabs[s.active].blocks[0].edit.set_selection(2, 8);
    s.comment_for_block(&page, &block, false);
    assert!(matches!(
        s.comments.anchor,
        Some(CommentAnchor::Text {
            start: 2,
            end: 8,
            ..
        })
    ));
    s.comments.input.replace_text("A comment");
    s.submit_comment();
    let thread = s.comments.threads[&page][0].id.clone();
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (1440., 900.),
        Env::default(),
        rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
    );
    rt.frame(0.);
    rt.frame(0.02);
    assert!(
        rt.find(&format!("comment-marker-{block}-{thread}"))
            .is_some()
    );
    assert_eq!(
        crate::ui::comments::ranges(&rt.state, &page, &block, "A useful sentence"),
        vec![2..8]
    );
    click(&mut rt, &format!("comment-reply-{thread}"));
    assert_eq!(rt.state.comments.reply.as_deref(), Some(thread.as_str()));
    click(&mut rt, "comment-input");
    press(&mut rt, Key::Text("A reply".into()));
    click(&mut rt, "comment-submit");
    assert_eq!(
        rt.state.comments.threads[&page][0].messages.len(),
        2,
        "{:?}, draft {:?}, reply {:?}",
        rt.state.comments.error,
        rt.state.comments.input.text,
        rt.state.comments.reply
    );
    click(&mut rt, &format!("comment-resolve-{thread}"));
    assert!(rt.state.comments.threads[&page][0].resolved);
    assert!(
        rt.find(&format!("comment-marker-{block}-{thread}"))
            .is_none()
    );
    click(&mut rt, &format!("comment-resolve-{thread}"));
    assert!(!rt.state.comments.threads[&page][0].resolved);
    drop(rt);
    let mut reopened = State::open(path.clone(), None).unwrap();
    reopened.load_comments(&page);
    assert_eq!(reopened.comments.threads[&page][0].messages.len(), 2);
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn comment_marker_follows_wrapped_text() {
    let (mut s, path) = fixture();
    s.new_page(false, None);
    let text = "This sentence wraps across several lines in the editor. ".repeat(24);
    s.tabs[s.active].blocks[0].edit.replace_text(&text);
    s.save_blocks();
    let page = s.tab().unwrap().page.id.clone();
    let block = s.tab().unwrap().blocks[0].block.id.clone();
    let start = text.len() - 20;
    s.tabs[s.active].blocks[0]
        .edit
        .set_selection(start, start + 6);
    s.comment_for_block(&page, &block, false);
    s.comments.input.replace_text("A comment");
    s.submit_comment();
    s.comments.open = false;
    let thread = s.comments.threads[&page][0].id.clone();
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (800., 900.),
        Env::default(),
        rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
    );
    for _ in 0..20 {
        rt.frame(0.02);
    }
    let elements = rt.elements();
    let marker = format!("comment-marker-{block}-{thread}");
    let highlight = format!("comment-highlight-{thread}");
    let marker_y = elements
        .iter()
        .find(|e| e.key.as_deref() == Some(marker.as_str()))
        .unwrap()
        .rect[1];
    let highlight_y = elements
        .iter()
        .find(|e| e.key.as_deref() == Some(highlight.as_str()))
        .unwrap()
        .rect[1];
    assert!(
        (marker_y - highlight_y).abs() < 2.,
        "marker {marker_y}, highlight {highlight_y}"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn agent_presence_is_scoped_to_current_page_and_removed() {
    let (mut s, path) = fixture();
    s.new_page(false, None);
    let page = s.tab().unwrap().page.id.clone();
    s.set_agent_presence(&page, vec![("session-1".into(), "Agent One".into())]);
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (1000., 900.),
        Env::default(),
        rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
    );
    rt.frame(0.);
    rt.frame(0.02);
    assert!(rt.find("presence-agent-session-1").is_some());
    click(&mut rt, "presence-agent-session-1");
    assert!(rt.find("presence-name-session-1").is_some());
    rt.update(|s, _| s.set_agent_presence(&page, vec![]));
    rt.frame(0.02);
    assert!(rt.find("presence-agent-session-1").is_none());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn comment_marker_has_accessible_name_and_keyboard_action_at_both_scales() {
    for scale in [1., 2.] {
        let (mut s, path) = fixture();
        s.new_page(false, None);
        s.tabs[s.active].blocks[0].edit.replace_text("Comment here");
        s.save_blocks();
        let page = s.tab().unwrap().page.id.clone();
        let block = s.tab().unwrap().blocks[0].block.id.clone();
        s.tabs[s.active].blocks[0].edit.set_selection(0, 7);
        s.comment_for_block(&page, &block, false);
        s.comments.input.replace_text("A comment");
        s.submit_comment();
        s.comments.open = false;
        let thread = s.comments.threads[&page][0].id.clone();
        let key = format!("comment-marker-{block}-{thread}");
        let mut rt = Runtime::with_text(
            s,
            ui::view,
            (1200., 900.),
            Env::new(true, scale),
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.frame(0.);
        rt.frame(0.02);
        let element = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(&key))
            .unwrap();
        assert_eq!(element.role, "button");
        assert_eq!(element.label.as_deref(), Some("Open comments"));
        assert!(element.focusable);
        for _ in 0..120 {
            if rt
                .elements()
                .iter()
                .any(|e| e.key.as_deref() == Some(&key) && e.focused)
            {
                break;
            }
            press(&mut rt, Key::Tab);
        }
        assert!(
            rt.elements()
                .iter()
                .any(|e| e.key.as_deref() == Some(&key) && e.focused)
        );
        press(&mut rt, Key::Text(" ".into()));
        assert!(rt.state.comments.open);
        assert_eq!(rt.state.comments.page_id.as_deref(), Some(page.as_str()));
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn comments_on_same_line_share_one_accessible_marker() {
    let (mut s, path) = fixture();
    s.new_page(false, None);
    s.tabs[s.active].blocks[0]
        .edit
        .replace_text("One shared line");
    s.save_blocks();
    let page = s.tab().unwrap().page.id.clone();
    let block = s.tab().unwrap().blocks[0].block.id.clone();
    for comment in ["First comment", "Second comment"] {
        s.open_comments(
            &page,
            Some(CommentAnchor::Text {
                block_id: block.clone(),
                start: 0,
                end: 3,
                quote: "One".into(),
            }),
        );
        s.comments.input.replace_text(comment);
        s.submit_comment();
    }
    s.comments.open = false;
    let first_thread = s.comments.threads[&page][0].id.clone();
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (1200., 900.),
        Env::default(),
        rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
    );
    for _ in 0..20 {
        rt.frame(0.02);
    }
    let marker = format!("comment-marker-{block}-{first_thread}");
    let elements = rt.elements();
    let markers: Vec<_> = elements
        .iter()
        .filter(|e| {
            e.key
                .as_deref()
                .is_some_and(|key| key.starts_with(&format!("comment-marker-{block}-")))
        })
        .collect();
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].label.as_deref(), Some("Open 2 comments"));
    assert!(markers[0].focusable);
    click(&mut rt, &marker);
    for thread in &rt.state.comments.threads[&page] {
        assert!(rt.find(&format!("comment-thread-{}", thread.id)).is_some());
    }
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn table_cells_do_not_show_history_or_comment_buttons() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.clone().unwrap();
    let title = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    s.store
        .create_record(&db.id, "Task", &Default::default())
        .unwrap();
    assert!(s.query_records());
    s.sidebar_keyboard_controls = true;
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (1440., 900.),
        Env::default(),
        rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
    );
    rt.frame(0.);
    rt.frame(0.02);
    assert!(rt.presented("table-open-0").is_some());
    for key in [
        format!("cell-comment-0-{title}"),
        format!("cell-history-0-{title}"),
        format!("cell-comment-0-{status}"),
        format!("cell-history-0-{status}"),
    ] {
        assert!(rt.find(&key).is_none(), "{key} should not render in table cells");
    }
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
