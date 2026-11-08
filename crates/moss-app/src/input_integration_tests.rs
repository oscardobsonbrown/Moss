use super::*;

fn height(rt: &Runtime<State>, key: &str) -> f32 {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap()
        .target[3]
}

fn long_title() -> String {
    "A long title with café and many words that should wrap across several lines ".repeat(4)
}

#[test]
fn page_titles_wrap_and_enter_moves_focus_to_the_body() {
    let (s, path) = fixture();
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (850., 600.),
        Env {
            system_dark: false,
            scale: 2.,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.frame(0.);

    // The new-page chooser keeps the wrapped title when Enter chooses a document.
    click(&mut rt, "sidebar-compose");
    let initial = height(&rt, "new-page-title");
    press(&mut rt, Key::Text(long_title()));
    assert!(height(&rt, "new-page-title") > initial + 40.);
    press(&mut rt, Key::Enter);
    assert!(!rt.state.tab().unwrap().choosing_type);
    press(&mut rt, Key::Text("Body one".into()));
    assert_eq!(rt.state.tab().unwrap().blocks[0].edit.text, "Body one");

    let page = rt.state.tab().unwrap().page.id.clone();
    let title_key = format!("title-{page}");
    assert!(height(&rt, &title_key) > 48.);
    click(&mut rt, &title_key);
    press(&mut rt, Key::Enter);
    press(&mut rt, Key::Text("Body two".into()));
    assert!(
        rt.state.tab().unwrap().blocks[0]
            .edit
            .text
            .contains("Body two")
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn marked_and_code_blocks_fit_the_page_width_and_wrap() {
    let (mut s, path) = fixture();
    s.new_page(false, None);
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (850., 600.),
        Env {
            system_dark: false,
            scale: 1.,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    for kind in [
        moss_core::BlockKind::BulletedList,
        moss_core::BlockKind::Code,
    ] {
        let active = rt.state.active;
        let block = &mut rt.state.tabs[active].blocks[0];
        block.block.kind = kind;
        block.edit.replace_text("");
        let block_id = block.block.id.clone();
        rt.frame(0.);
        let input_key = format!("block-input-{block_id}");
        click(&mut rt, &input_key);
        press(&mut rt, Key::Text(long_title()));
        let row_key = format!("block-{block_id}");
        let elements = rt.elements();
        let input = elements
            .iter()
            .find(|e| e.key.as_deref() == Some(&input_key))
            .unwrap()
            .target;
        let row = elements
            .iter()
            .find(|e| e.key.as_deref() == Some(&row_key))
            .unwrap()
            .target;
        assert!(
            input[0] + input[2] <= row[0] + row[2] + 0.1,
            "input {input:?}, row {row:?}"
        );
        assert!(input[3] > 46., "block wraps: {input:?}");
    }
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn record_peek_title_wraps_and_enter_moves_focus_to_the_body() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    s.create_peek(Default::default());
    let page = s.record_peek.as_ref().unwrap().page.id.clone();
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (850., 600.),
        Env {
            system_dark: true,
            scale: 1.,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    let title = format!("peek-title-{page}");
    click(&mut rt, &title);
    let initial = height(&rt, &title);
    press(&mut rt, Key::Text(long_title()));
    assert!(height(&rt, &title) > initial + 40.);
    press(&mut rt, Key::Enter);
    press(&mut rt, Key::Text("Peek body".into()));
    assert_eq!(
        rt.state.record_peek.as_ref().unwrap().blocks[0].edit.text,
        "Peek body"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
