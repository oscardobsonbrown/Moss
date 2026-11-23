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

#[test]
fn side_peek_supports_turn_into_duplicate_delete_markdown_and_list_continuation() {
    for mode in ["side", "center"] {
        let (mut s, path) = fixture();
        s.new_page(true, None);
        let db = s.tab().unwrap().database.clone().unwrap();
        let record = s
            .store
            .create_record(&db.id, "Sheet record", &Default::default())
            .unwrap();
        s.open_database_record(&db.id, &record.id, mode);
        let page = s.record_peek.as_ref().unwrap().page.id.clone();
        s.record_peek.as_mut().unwrap().blocks[0]
            .edit
            .replace_text("First item");
        s.save_peek_blocks();
        let mut rt = Runtime::with_text(
            s,
            ui::view,
            (1200., 800.),
            Env::default(),
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.on_key = Some(key);
        rt.frame(0.);
        let hover_click = |rt: &mut Runtime<State>, target: &str| {
            let (x, y) = rt.find(target).unwrap_or_else(|| panic!("missing {target}"));
            rt.pointer_move(x, y);
            rt.frame(1. / 60.);
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            rt.frame(1. / 60.);
        };
        hover_click(&mut rt, "peek-block-menu-0");
        assert!(rt.state.block_commands.as_ref().is_some_and(|m| m.peek));
        for name in [
            "Heading 1",
            "Heading 2",
            "To-do list",
            "Bulleted list",
            "Code",
            "Toggle",
            "Quote",
            "Callout",
            "Table",
            "Divider",
            "Page",
        ] {
            assert!(
                rt.find(&format!("block-kind-{name}")).is_some(),
                "mode={mode} missing {name}"
            );
        }
        assert!(rt.find("duplicate-block").is_some(), "mode={mode}");
        assert!(rt.find("delete-block").is_some(), "mode={mode}");
        for _ in 0..20 {
            rt.frame(1. / 60.);
        }
        click(&mut rt, "block-kind-Heading 2");
        assert_eq!(
            rt.state.store.get_blocks(&page).unwrap()[0].kind,
            BlockKind::Heading2
        );
        hover_click(&mut rt, "peek-block-menu-0");
        for _ in 0..20 {
            rt.frame(1. / 60.);
        }
        click(&mut rt, "duplicate-block");
        assert_eq!(rt.state.record_peek.as_ref().unwrap().blocks.len(), 2);
        hover_click(&mut rt, "peek-block-menu-1");
        assert!(rt.state.block_commands.as_ref().is_some_and(|m| m.peek));
        for _ in 0..20 {
            rt.frame(1. / 60.);
        }
        click(&mut rt, "delete-block");
        let peek = rt.state.record_peek.as_ref().unwrap();
        assert_eq!(peek.blocks.len(), 1);
        assert!(peek.blocks[0].edit.focused);
        assert_eq!(peek.blocks[0].edit.range(), (10, 10));
        // Append via blank area, use markdown shortcut + list continuation + empty-list exit.
        click(&mut rt, "peek-append-block");
        assert_eq!(rt.state.record_peek.as_ref().unwrap().blocks.len(), 2);
        press(&mut rt, Key::Text("-".into()));
        press(&mut rt, Key::Text(" ".into()));
        assert_eq!(
            rt.state.record_peek.as_ref().unwrap().blocks[1].block.kind,
            BlockKind::BulletedList
        );
        press(&mut rt, Key::Text("Bullet one".into()));
        press(&mut rt, Key::Enter);
        let peek = rt.state.record_peek.as_ref().unwrap();
        assert_eq!(peek.blocks.len(), 3);
        assert_eq!(peek.blocks[2].block.kind, BlockKind::BulletedList);
        press(&mut rt, Key::Enter);
        assert_eq!(
            rt.state.record_peek.as_ref().unwrap().blocks[2].block.kind,
            BlockKind::Paragraph
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn inter_block_delete_merge_and_arrows_work_in_full_page_and_side_peek() {
    use crate::text_colors::{Channel, Palette, active_color, apply_inline};
    use rune::edit::Mark;
    for peek in [false, true] {
        let (mut s, path) = fixture();
        if peek {
            s.new_page(true, None);
            let db = s.tab().unwrap().database.clone().unwrap();
            let record = s
                .store
                .create_record(&db.id, "Record title", &Default::default())
                .unwrap();
            s.open_database_record(&db.id, &record.id, "side");
            s.record_peek.as_mut().unwrap().title.focused = false;
        } else {
            s.new_page(false, None);
            s.tabs[s.active].title.replace_text("Page title");
            s.save_title();
        }
        let page = s.editor_page_id(peek).unwrap();
        {
            let blocks = s.editor_blocks_mut(peek).unwrap();
            blocks[0].edit.replace_text("Alpha");
            blocks[0].edit.focused = false;
        }
        s.append_editor_block_in(peek);
        {
            let blocks = s.editor_blocks_mut(peek).unwrap();
            blocks[1].edit.replace_text("Beta");
            blocks[1].edit.set_selection(0, 4);
            blocks[1].edit.toggle_mark(Mark::Bold);
            blocks[1].edit.set_caret(0);
            blocks[1].block.text = "Beta".into();
            apply_inline(&mut blocks[1].block, 0..4, Channel::Text, Some(Palette::Blue));
            blocks[1].edit.focused = false;
        }
        s.append_editor_block_in(peek);
        {
            let blocks = s.editor_blocks_mut(peek).unwrap();
            blocks[2].edit.replace_text("Gamma");
            blocks[2].edit.focused = false;
        }
        s.append_editor_block_in(peek);
        s.save_editor_blocks(peek);
        let mut rt = Runtime::with_text(
            s,
            ui::view,
            (1200., 800.),
            Env::default(),
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.on_key = Some(key);
        rt.frame(0.);

        // 1. Pressing Delete on an empty block deletes it and moves caret to end of previous block.
        press(&mut rt, Key::Delete);
        {
            let blocks = rt.state.editor_blocks_mut(peek).unwrap();
            assert_eq!(blocks.len(), 3, "peek={peek}");
            assert!(blocks[2].edit.focused, "peek={peek}");
            assert_eq!(blocks[2].edit.range(), (5, 5), "peek={peek}");
        }

        // 2. Pressing Escape selects block 2, then Delete removes it and moves caret to end of block 1.
        press(&mut rt, Key::Escape);
        assert_eq!(
            rt.state
                .editor_selection
                .as_ref()
                .map(|sel| sel.ids.len()),
            Some(1),
            "peek={peek}"
        );
        press(&mut rt, Key::Delete);
        {
            let blocks = rt.state.editor_blocks_mut(peek).unwrap();
            assert_eq!(blocks.len(), 2, "peek={peek}");
            assert!(blocks[1].edit.focused, "peek={peek}");
            assert_eq!(blocks[1].edit.range(), (4, 4), "peek={peek}");
        }

        // 3. Arrow Up from block 1 -> block 0 -> title, then Down -> block 0 -> block 1.
        press(&mut rt, Key::Up);
        assert!(rt.state.editor_blocks_mut(peek).unwrap()[0].edit.focused);
        press(&mut rt, Key::Up);
        if peek {
            assert!(rt.state.record_peek.as_ref().unwrap().title.focused);
        } else {
            assert!(rt.state.tab().unwrap().title.focused);
        }
        press(&mut rt, Key::Down);
        assert!(rt.state.editor_blocks_mut(peek).unwrap()[0].edit.focused);
        assert_eq!(rt.state.editor_blocks_mut(peek).unwrap()[0].edit.range(), (0, 0));
        press(&mut rt, Key::Down);
        assert!(rt.state.editor_blocks_mut(peek).unwrap()[1].edit.focused);

        // 4. Backspace at offset 0 of non-empty block 1 merges text, bold mark, and inline color into block 0.
        rt.state.editor_blocks_mut(peek).unwrap()[1].edit.set_caret(0);
        rt.frame(1. / 60.);
        press(&mut rt, Key::Backspace);
        {
            let blocks = rt.state.editor_blocks_mut(peek).unwrap();
            assert_eq!(blocks.len(), 1, "peek={peek}");
            assert_eq!(blocks[0].edit.text, "AlphaBeta", "peek={peek}");
            assert!(blocks[0].edit.focused, "peek={peek}");
            assert_eq!(blocks[0].edit.range(), (5, 5), "peek={peek}");
            assert!(
                blocks[0]
                    .edit
                    .marks()
                    .iter()
                    .any(|m| m.mark == Mark::Bold && (m.start, m.end) == (5, 9)),
                "peek={peek}"
            );
            let saved = rt.state.store.get_blocks(&page).unwrap();
            assert_eq!(
                active_color(&saved[0], Some(5..9), Channel::Text),
                Some(Palette::Blue),
                "peek={peek}"
            );
        }

        // 5. Forward Delete at end of block 0 merges next block upward.
        rt.state.append_editor_block_in(peek);
        rt.state.editor_blocks_mut(peek).unwrap()[1]
            .edit
            .replace_text("Tail");
        rt.state.save_editor_blocks(peek);
        rt.state.focus_block_index(0, None, peek, None);
        rt.frame(1. / 60.);
        press(&mut rt, Key::Delete);
        {
            let blocks = rt.state.editor_blocks_mut(peek).unwrap();
            assert_eq!(blocks.len(), 1, "peek={peek}");
            assert_eq!(blocks[0].edit.text, "AlphaBetaTail", "peek={peek}");
            assert_eq!(blocks[0].edit.range(), (9, 9), "peek={peek}");
        }

        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn dragging_or_pasting_an_image_inserts_and_loads_it_in_full_page_and_side_peek() {
    use std::sync::Arc;
    let png: Vec<u8> = vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8,
        2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0,
        0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    for peek in [false, true] {
        let (mut s, path) = fixture();
        s.page_cover.set_waker(Arc::new(|| {}));
        let image_path = path.join("dropped.png");
        std::fs::write(&image_path, &png).unwrap();
        if peek {
            s.new_page(true, None);
            let db = s.tab().unwrap().database.clone().unwrap();
            let record = s
                .store
                .create_record(&db.id, "Media record", &Default::default())
                .unwrap();
            s.open_database_record(&db.id, &record.id, "side");
            s.record_peek.as_mut().unwrap().blocks[0]
                .edit
                .replace_text("Intro");
            s.save_peek_blocks();
        } else {
            s.new_page(false, None);
            s.tabs[s.active].blocks[0].edit.replace_text("Intro");
            s.save_blocks();
        }
        let page = s.editor_page_id(peek).unwrap();
        let first_id = s.editor_blocks_mut(peek).unwrap()[0].block.id.clone();
        let mut rt = Runtime::with_text(
            s,
            ui::view,
            (1200., 800.),
            Env::default(),
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.on_key = Some(key);
        rt.poll = Some(crate::poll);
        rt.frame(0.);
        rt.frame(0.02);
        let block_key = if peek {
            format!("peek-drag-block-{first_id}")
        } else {
            format!("block-{first_id}")
        };
        let rect = rt.presented(&block_key).unwrap().0;
        let drop_pos = (rect[0] + rect[2] / 2., rect[1] + rect[3] - 2.);
        rt.drag_files(vec![image_path.clone()], Some(drop_pos));
        rt.frame(0.02);
        assert!(
            rt.presented("editor-block-drop-guide").is_some(),
            "peek={peek} expected file drop guide"
        );
        rt.drop_files(vec![image_path.clone()], Some(drop_pos));
        for _ in 0..80 {
            rt.frame(0.02);
            let saved = rt.state.store.get_blocks(&page).unwrap();
            if saved.len() == 2
                && saved[1].kind == BlockKind::Image
                && saved[1].config.get("asset").is_some()
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let saved = rt.state.store.get_blocks(&page).unwrap();
        assert_eq!(saved.len(), 2, "peek={peek}");
        assert_eq!(saved[1].kind, BlockKind::Image, "peek={peek}");
        assert!(saved[1].config.get("asset").is_some(), "peek={peek}");

        // Now paste a PNG image via clipboard while focused on block 0.
        let input_key = if peek {
            format!("peek-block-{first_id}")
        } else {
            format!("block-input-{first_id}")
        };
        click(&mut rt, &input_key);
        let clip = rune::ClipboardContent::new("")
            .with_image(rune::ClipboardImage::from_png(png.clone()).unwrap());
        rt.paste_content(clip);
        for _ in 0..80 {
            rt.frame(0.02);
            let saved = rt.state.store.get_blocks(&page).unwrap();
            if saved.len() == 3
                && saved[1].kind == BlockKind::Image
                && saved[1].config.get("asset").is_some()
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let saved = rt.state.store.get_blocks(&page).unwrap();
        assert_eq!(saved.len(), 3, "peek={peek}");
        assert_eq!(saved[1].kind, BlockKind::Image, "peek={peek}");
        assert!(saved[1].config.get("asset").is_some(), "peek={peek}");
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
