use super::*;

#[test]
fn block_handle_stays_reachable_and_preserves_selection() {
    let (mut s, path) = fixture();
    s.new_page(false, None);
    let id = s.tab().unwrap().blocks[0].block.id.clone();
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1000., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    let input = format!("block-input-{id}");
    click(&mut rt, &input);
    press(&mut rt, Key::Text("Selected block text".into()));
    let active = rt.state.active;
    rt.state.tabs[active].blocks[0].edit.set_selection(0, 8);
    rt.frame(0.);
    let info = |rt: &Runtime<State>, name: &str| {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(name))
            .unwrap()
    };
    let input_rect = info(&rt, &input).rect;
    let handle_rect = info(&rt, "block-menu-0").rect;
    let y = handle_rect[1] + handle_rect[3] / 2.;
    for x in [
        input_rect[0] + 2.,
        input_rect[0],
        handle_rect[0] + handle_rect[2] + 2.,
        handle_rect[0] + handle_rect[2] / 2.,
    ] {
        rt.pointer_move(x, y);
        rt.frame(1. / 60.);
        assert!(
            info(&rt, "block-menu-0").opacity > 0.9,
            "handle hidden at {x}"
        );
    }
    click(&mut rt, "block-menu-0");
    assert!(rt.state.block_commands.is_some());
    let edit = &rt.state.tab().unwrap().blocks[0].edit;
    assert_eq!(edit.text, "Selected block text");
    assert_eq!(edit.range(), (0, 8));
    let menu = info(&rt, "block-commands");
    assert!(
        menu.rect[1] >= handle_rect[1],
        "menu should open beside its handle"
    );
    press(&mut rt, Key::Escape);
    assert!(rt.state.block_commands.is_none());
    assert!(rt.state.tab().unwrap().blocks[0].edit.focused);
    click(&mut rt, "block-menu-0");
    assert!(rt.state.block_commands.is_some());
    assert_eq!(rt.state.tab().unwrap().blocks[0].edit.range(), (0, 8));
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn unfocused_block_handle_remains_visible_through_gutter_and_open_menu() {
    let (mut s, path) = fixture();
    s.new_page(false, None);
    let page = s.tab().unwrap().page.id.clone();
    let block = s.tab().unwrap().blocks[0].block.id.clone();
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1000., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, &format!("title-{page}"));
    assert!(!rt.state.tab().unwrap().blocks[0].edit.focused);
    let info = |rt: &Runtime<State>, name: &str| {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(name))
            .unwrap()
    };
    let input = info(&rt, &format!("block-input-{block}")).rect;
    let handle = info(&rt, "block-menu-0").rect;
    let y = handle[1] + handle[3] / 2.;
    rt.pointer_move(input[0] + 2., y);
    for _ in 0..20 {
        rt.frame(1. / 60.);
    }
    for x in [
        input[0],
        handle[0] + handle[2] + 2.,
        handle[0] + handle[2] / 2.,
    ] {
        rt.pointer_move(x, y);
        rt.frame(1. / 60.);
        assert!(
            info(&rt, "block-menu-0").opacity > 0.9,
            "handle hidden in gutter at {x}"
        );
    }
    click(&mut rt, "block-menu-0");
    assert!(rt.state.block_commands.is_some());
    press(&mut rt, Key::Tab);
    rt.pointer_move(900., 80.);
    for _ in 0..20 {
        rt.frame(1. / 60.);
    }
    assert!(
        info(&rt, "block-menu-0").opacity > 0.9,
        "open menu keeps handle visible"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
