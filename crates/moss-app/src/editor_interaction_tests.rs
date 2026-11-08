use super::*;

#[test]
fn slash_and_markdown_work_from_the_real_editor_and_code_keeps_literal_slashes() {
    let (mut s, path) = fixture();
    s.new_page(false, None);
    let id = s.tab().unwrap().blocks[0].block.id.clone();
    let mut rt = Runtime::with_text(
        s, ui::view, (850., 600.), Env { system_dark: false, scale: 2. },
        rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, &format!("block-input-{id}"));
    press(&mut rt, Key::Text("/".into()));
    assert!(rt.state.block_commands.is_some());
    let panel = rt.presented("block-commands").unwrap().0;
    assert!(panel[0] >= 0. && panel[1] >= 0. && panel[0] + panel[2] <= 850. && panel[1] + panel[3] <= 600., "{panel:?}");
    press(&mut rt, Key::Text("h2".into()));
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.tab().unwrap().blocks[0].block.kind, BlockKind::Heading2);
    press(&mut rt, Key::Text("A heading".into()));
    press(&mut rt, Key::Enter);
    let active = rt.state.active;
    assert_eq!(rt.state.tab().unwrap().blocks[0].edit.text, "A heading");
    assert_eq!(rt.state.tab().unwrap().blocks[1].block.kind, BlockKind::Paragraph);
    press(&mut rt, Key::Text("##".into()));
    press(&mut rt, Key::Text(" ".into()));
    assert_eq!(rt.state.tab().unwrap().blocks[1].block.kind, BlockKind::Heading2);
    press(&mut rt, Key::Backspace);
    assert_eq!(rt.state.tab().unwrap().blocks[1].block.kind, BlockKind::Paragraph);
    press(&mut rt, Key::Text("/".into()));
    press(&mut rt, Key::Text("literal words".into()));
    press(&mut rt, Key::Escape);
    assert!(rt.state.block_commands.is_none());
    assert_eq!(rt.state.tab().unwrap().blocks[1].edit.text, "/literal words");
    rt.update(|s, tx| {
        s.tabs[active].blocks[1].block.kind = BlockKind::Code;
        s.tabs[active].blocks[1].edit.replace_text("");
        s.tabs[active].blocks[1].edit.focused = true;
        tx.instant();
    });
    rt.frame(0.);
    press(&mut rt, Key::Text("/".into()));
    assert!(rt.state.block_commands.is_none());
    assert_eq!(rt.state.tab().unwrap().blocks[1].edit.text, "/");
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.tab().unwrap().blocks[1].edit.text, "/\n");
    let page_id = rt.state.tab().unwrap().page.id.clone();
    let stored = rt.state.store.get_blocks(&page_id).unwrap();
    assert_eq!(stored[0].text, "A heading");
    assert_eq!(stored[1].text, "/\n");
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
