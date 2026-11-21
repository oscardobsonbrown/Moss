use super::*;
use crate::state::State;
use moss_core::new_id;
use rune::{TextEdit, edit::Mark};
use std::path::PathBuf;

fn fixture() -> (State, PathBuf) {
    let path = std::env::temp_dir().join(format!("moss-editor-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    state.new_page(false, None);
    (state, path)
}

#[test]
fn checkbox_click_saves_the_task_in_pages_and_previews() {
    use rune::{Env, Runtime};
    for peek in [false, true] {
        let (mut s, path) = fixture();
        if peek {
            s.new_page(true, None);
            s.create_peek(Default::default());
            s.record_peek.as_mut().unwrap().blocks[0].block.kind = BlockKind::Todo;
            s.save_peek_blocks();
        } else {
            s.tabs[s.active].blocks[0].block.kind = BlockKind::Todo;
            s.save_blocks();
        }
        let page = if peek {
            s.record_peek.as_ref().unwrap().page.id.clone()
        } else {
            s.tab().unwrap().page.id.clone()
        };
        let mut rt = Runtime::with_text(
            s,
            crate::ui::view,
            (1400., 1000.),
            Env::default(),
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.frame(0.);
        rt.frame(0.02);
        let r = rt
            .presented(if peek { "peek-todo-0" } else { "todo-0" })
            .unwrap()
            .0;
        let (x, y) = (r[0] + r[2] / 2., r[1] + r[3] / 2.);
        let changes = rt.state.store.local_change_count();
        rt.pointer_move(x, y);
        rt.frame(0.02);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(
            rt.state.store.get_blocks(&page).unwrap()[0].checked,
            "peek={peek}"
        );
        assert_eq!(
            rt.state.store.local_change_count() - changes,
            2,
            "peek={peek}"
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn checkbox_save_keeps_unsaved_edits_and_reports_a_missing_task() {
    let (mut s, path) = fixture();
    let page = s.tab().unwrap().page.id.clone();
    let id = s.tab().unwrap().blocks[0].block.id.clone();
    s.tabs[s.active].blocks[0].block.kind = BlockKind::Todo;
    s.save_blocks();
    s.tabs[s.active].blocks[0].edit.replace_text("Unsaved text");
    s.tabs[s.active].dirty_blocks = true;
    s.toggle_todo(&id, false);
    let saved = s.store.get_blocks(&page).unwrap();
    assert!(saved[0].checked);
    assert_eq!(saved[0].text, "Unsaved text");
    assert!(!s.tab().unwrap().dirty_blocks);

    let mut changed = saved[0].clone();
    changed.kind = BlockKind::Paragraph;
    s.store.save_blocks(&page, &[changed.clone()]).unwrap();
    s.toggle_todo(&id, false);
    assert!(s.tab().unwrap().dirty_blocks);
    assert!(s.error.as_deref().unwrap().contains("not saved"));
    assert_eq!(s.store.get_blocks(&page).unwrap(), vec![changed]);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn task_rows_stay_editable_and_draggable_after_checkbox_updates() {
    use rune::{Env, Key, KeyEvent, Mods, Runtime};
    let (mut s, path) = fixture();
    s.tabs[s.active].blocks[0].block.kind = BlockKind::Todo;
    s.tabs[s.active].blocks[0].edit.replace_text("First");
    s.append_editor_block();
    s.tabs[s.active].blocks[1].block.kind = BlockKind::Todo;
    s.tabs[s.active].blocks[1].edit.replace_text("Second");
    s.tabs[s.active].blocks[1].edit.focused = false;
    s.save_blocks();
    let ids: Vec<_> = s
        .tab()
        .unwrap()
        .blocks
        .iter()
        .map(|b| b.block.id.clone())
        .collect();
    let mut rt = Runtime::with_text(
        s,
        crate::ui::view,
        (1400., 1000.),
        Env::default(),
        rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
    );
    rt.on_key = Some(crate::key);
    rt.frame(0.);
    rt.frame(0.02);
    let checkbox = rt.presented("todo-0").unwrap().0;
    let (x, y) = (
        checkbox[0] + checkbox[2] / 2.,
        checkbox[1] + checkbox[3] / 2.,
    );
    rt.pointer_move(x, y);
    rt.frame(0.02);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.02);
    let handle = rt.presented("block-menu-1").unwrap().0;
    let target = rt.presented(&format!("block-{}", ids[0])).unwrap().0;
    let from = [handle[0] + handle[2] / 2., handle[1] + handle[3] / 2.];
    let to = [from[0], target[1] + 2.];
    rt.pointer_move(from[0], from[1]);
    rt.frame(0.02);
    rt.pointer_down(from[0], from[1]);
    rt.pointer_move(to[0], to[1]);
    rt.frame(0.02);
    rt.pointer_up(to[0], to[1]);
    rt.frame(0.02);
    assert_eq!(rt.state.tab().unwrap().blocks[0].block.id, ids[1]);
    let input = rt.presented(&format!("block-input-{}", ids[1])).unwrap().0;
    rt.pointer_down(input[0] + 6., input[1] + input[3] / 2.);
    rt.pointer_up(input[0] + 6., input[1] + input[3] / 2.);
    rt.frame(0.02);
    for key in [Key::End, Key::Text("!".into())] {
        rt.key(KeyEvent {
            key,
            mods: Mods::default(),
        });
        rt.frame(0.02);
    }
    assert_eq!(rt.state.tab().unwrap().blocks[0].edit.text, "Second!");
    assert!(rt.state.tab().unwrap().blocks[1].block.checked);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
#[ignore = "performance diagnostic"]
fn checkbox_click_performance() {
    use rune::{Env, Runtime};
    use std::time::Instant;
    for count in [10, 100] {
        let (mut s, path) = fixture();
        let page = s.tab().unwrap().page.id.clone();
        let blocks = &mut s.tabs[s.active].blocks;
        blocks.clear();
        for i in 0..count {
            let mut block = moss_core::Block::paragraph(&page, format!("Task {i}"), i as i64);
            block.kind = BlockKind::Todo;
            blocks.push(crate::state::EditableBlock {
                edit: TextEdit::new(&block.text).rich(),
                block,
            });
        }
        s.save_blocks();
        let id = s.tab().unwrap().blocks[0].block.id.clone();
        let mut toggles = Vec::new();
        for _ in 0..30 {
            let start = Instant::now();
            s.toggle_todo(&id, false);
            toggles.push(start.elapsed().as_secs_f64() * 1000.);
        }
        let mut saves = Vec::new();
        for _ in 0..30 {
            let start = Instant::now();
            s.save_blocks();
            saves.push(start.elapsed().as_secs_f64() * 1000.);
        }
        let mut rt = Runtime::with_text(
            s,
            crate::ui::view,
            (1400., 1000.),
            Env::default(),
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.frame(0.);
        rt.frame(0.02);
        let r = rt.presented("todo-0").unwrap().0;
        let (x, y) = (r[0] + r[2] / 2., r[1] + r[3] / 2.);
        rt.pointer_move(x, y);
        rt.frame(0.02);
        let mut frames = Vec::new();
        for _ in 0..30 {
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            let start = Instant::now();
            rt.frame(0.02);
            frames.push(start.elapsed().as_secs_f64() * 1000.);
        }
        for (name, samples) in [
            ("single toggle", &mut toggles),
            ("save", &mut saves),
            ("click frame", &mut frames),
        ] {
            samples.sort_by(f64::total_cmp);
            println!(
                "blocks={count} {name}: median={:.3}ms p95={:.3}ms max={:.3}ms",
                samples[15], samples[28], samples[29]
            );
        }
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
#[test]
fn typing_three_hyphens_makes_a_divider_and_keeps_typing_on_the_next_line() {
    use rune::{Env, Key, KeyEvent, Mods, Runtime};
    for peek in [false, true] {
        let (mut s, path) = fixture();
        if peek {
            s.new_page(true, None);
            s.create_peek(Default::default());
            s.record_peek.as_mut().unwrap().title.focused = false;
            s.record_peek.as_mut().unwrap().blocks[0].edit.focused = true;
        } else {
            s.tabs[s.active].blocks[0].edit.focused = true;
        }
        let page = if peek {
            s.record_peek.as_ref().unwrap().page.id.clone()
        } else {
            s.tab().unwrap().page.id.clone()
        };
        let mut rt = Runtime::with_text(
            s,
            crate::ui::view,
            (1400., 1000.),
            Env::default(),
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.on_key = Some(crate::key);
        rt.frame(0.);
        for key in [
            Key::Text("-".into()),
            Key::Text("-".into()),
            Key::Text("-".into()),
            Key::Text("After".into()),
        ] {
            rt.key(KeyEvent {
                key,
                mods: Mods::default(),
            });
            rt.frame(0.02);
        }
        let blocks = if peek {
            &rt.state.record_peek.as_ref().unwrap().blocks
        } else {
            &rt.state.tab().unwrap().blocks
        };
        assert_eq!(blocks.len(), 2, "peek={peek}");
        assert_eq!(blocks[0].block.kind, BlockKind::Divider);
        assert!(blocks[0].edit.text.is_empty());
        assert_eq!(blocks[1].edit.text, "After");
        let saved = rt.state.store.get_blocks(&page).unwrap();
        assert_eq!(saved[0].kind, BlockKind::Divider);
        assert_eq!(saved[1].text, "After");
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn divider_shortcut_does_not_replace_text_or_non_paragraph_blocks() {
    let (mut s, path) = fixture();
    let block = &mut s.tabs[s.active].blocks[0];
    let id = block.block.id.clone();
    block.edit.replace_text("--");
    block.edit.set_caret(1);
    assert!(!s.editor_markdown_divider(&id, false));
    let block = &mut s.tabs[s.active].blocks[0];
    block.edit.set_caret(2);
    block.block.kind = BlockKind::Code;
    assert!(!s.editor_markdown_divider(&id, false));
    assert_eq!(s.tab().unwrap().blocks[0].edit.text, "--");
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn enter_splits_selection_and_rich_marks_and_persists_both_blocks() {
    let (mut s, path) = fixture();
    let page = s.tab().unwrap().page.id.clone();
    let tab = &mut s.tabs[s.active];
    let id = tab.blocks[0].block.id.clone();
    let mut edit = TextEdit::new("hello bold world").rich();
    edit.set_selection(6, 10);
    edit.toggle_mark(Mark::Bold);
    edit.set_selection(8, 10);
    tab.blocks[0].edit = edit;
    assert!(s.editor_enter(&id));
    let tab = s.tab().unwrap();
    assert_eq!(tab.blocks[0].edit.text, "hello bo");
    assert_eq!(tab.blocks[1].edit.text, " world");
    assert_eq!(
        (
            tab.blocks[0].edit.marks()[0].start,
            tab.blocks[0].edit.marks()[0].end
        ),
        (6, 8)
    );
    assert!(tab.blocks[1].edit.marks().is_empty());
    assert!(tab.blocks[1].edit.focused);
    let blocks = s.store.get_blocks(&page).unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(
        crate::rich_text::load(&blocks[0]).marks(),
        tab.blocks[0].edit.marks()
    );
    assert_eq!(crate::rich_text::load(&blocks[1]).text, " world");
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn enter_preserves_a_mark_across_the_split() {
    let (mut s, path) = fixture();
    let block = &mut s.tabs[s.active].blocks[0];
    let id = block.block.id.clone();
    let mut edit = TextEdit::new("hello bold world").rich();
    edit.set_selection(6, 10);
    edit.toggle_mark(Mark::Bold);
    edit.set_caret(8);
    block.edit = edit;
    assert!(s.editor_enter(&id));
    let tab = s.tab().unwrap();
    assert_eq!(tab.blocks[0].edit.text, "hello bo");
    assert_eq!(tab.blocks[1].edit.text, "ld world");
    assert_eq!(
        (
            tab.blocks[0].edit.marks()[0].start,
            tab.blocks[0].edit.marks()[0].end
        ),
        (6, 8)
    );
    assert_eq!(
        (
            tab.blocks[1].edit.marks()[0].start,
            tab.blocks[1].edit.marks()[0].end
        ),
        (0, 2)
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn enter_continues_nonempty_lists_and_exits_empty_lists() {
    for kind in [
        BlockKind::BulletedList,
        BlockKind::NumberedList,
        BlockKind::Todo,
    ] {
        let (mut s, path) = fixture();
        let block = &mut s.tabs[s.active].blocks[0];
        block.block.kind = kind;
        block.edit.replace_text("item");
        block.edit.set_caret(4);
        let id = block.block.id.clone();
        assert!(s.editor_enter(&id));
        let tab = s.tab().unwrap();
        assert_eq!(tab.blocks.len(), 2);
        assert_eq!(tab.blocks[1].block.kind, kind);
        let next = tab.blocks[1].block.id.clone();
        assert!(s.editor_enter(&next));
        let tab = s.tab().unwrap();
        assert_eq!(tab.blocks.len(), 2);
        assert_eq!(tab.blocks[1].block.kind, BlockKind::Paragraph);
        assert_eq!(
            s.store.get_blocks(&tab.page.id).unwrap()[1].kind,
            BlockKind::Paragraph
        );
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
}
#[test]
fn typed_space_shortcuts_only_match_exact_prefix_at_the_caret() {
    let cases = [
        ("#", BlockKind::Heading1),
        ("##", BlockKind::Heading2),
        ("###", BlockKind::Heading3),
        ("-", BlockKind::BulletedList),
        ("*", BlockKind::BulletedList),
        ("+", BlockKind::BulletedList),
        ("1.", BlockKind::NumberedList),
        ("a.", BlockKind::NumberedList),
        ("i.", BlockKind::NumberedList),
        ("[]", BlockKind::Todo),
        ("[ ]", BlockKind::Todo),
        (">", BlockKind::Toggle),
        ("\"", BlockKind::Quote),
        ("---", BlockKind::Divider),
    ];
    for (prefix, expected) in cases {
        let (mut s, path) = fixture();
        let block = &mut s.tabs[s.active].blocks[0];
        block.edit.replace_text(prefix);
        block.edit.set_caret(prefix.len());
        let id = block.block.id.clone();
        assert!(s.editor_markdown_space(&id));
        let tab = s.tab().unwrap();
        assert_eq!(tab.blocks[0].block.kind, expected);
        assert!(tab.blocks[0].edit.text.is_empty());
        assert_eq!(
            tab.blocks.len(),
            if expected == BlockKind::Divider { 2 } else { 1 }
        );
        assert_eq!(s.store.get_blocks(&tab.page.id).unwrap()[0].kind, expected);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
    let (mut s, path) = fixture();
    let block = &mut s.tabs[s.active].blocks[0];
    block.edit.replace_text("#\ntext");
    block.edit.set_caret(1);
    let id = block.block.id.clone();
    assert!(!s.editor_markdown_space(&id));
    assert_eq!(s.tab().unwrap().blocks[0].edit.text, "#\ntext");
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn blank_writing_area_never_splits_existing_text_or_adds_extra_empty_blocks() {
    let (mut s, path) = fixture();
    let tab = &mut s.tabs[s.active];
    tab.title.focused = true;
    tab.blocks[0].edit.replace_text("Keep this text");
    tab.blocks[0].edit.set_caret(0);
    s.append_editor_block();
    let tab = s.tab().unwrap();
    assert_eq!(tab.blocks.len(), 2);
    assert_eq!(tab.blocks[0].edit.text, "Keep this text");
    assert!(!tab.title.focused);
    assert!(tab.blocks[1].edit.focused);
    let id = tab.blocks[1].block.id.clone();
    let page = tab.page.id.clone();
    assert_eq!(s.store.get_blocks(&page).unwrap()[0].text, "Keep this text");
    s.append_editor_block();
    let tab = s.tab().unwrap();
    assert_eq!(tab.blocks.len(), 2);
    assert_eq!(tab.blocks[1].block.id, id);
    assert!(tab.blocks[1].edit.focused);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn backspace_on_empty_structured_block_converts_without_deleting() {
    for kind in [
        BlockKind::BulletedList,
        BlockKind::NumberedList,
        BlockKind::Todo,
        BlockKind::Heading1,
    ] {
        let (mut s, path) = fixture();
        let block = &mut s.tabs[s.active].blocks[0];
        block.block.kind = kind;
        block.block.checked = true;
        block.edit.focused = true;
        let id = block.block.id.clone();
        let page = s.tab().unwrap().page.id.clone();
        assert!(s.editor_backspace_empty(&id));
        let block = &s.tab().unwrap().blocks[0];
        assert_eq!(block.block.id, id);
        assert_eq!(block.block.kind, BlockKind::Paragraph);
        assert!(!block.block.checked);
        assert!(block.edit.focused);
        let saved = s.store.get_blocks(&page).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].kind, BlockKind::Paragraph);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn backspace_after_divider_removes_divider_and_keeps_editable_paragraph() {
    let (mut s, path) = fixture();
    let tab = &mut s.tabs[s.active];
    tab.blocks[0].block.kind = BlockKind::Divider;
    let divider = tab.blocks[0].block.id.clone();
    let paragraph = moss_core::Block::paragraph(&tab.page.id, "", 1);
    let id = paragraph.id.clone();
    let mut edit = TextEdit::new("").rich();
    edit.focused = true;
    tab.blocks.push(crate::state::EditableBlock {
        block: paragraph,
        edit,
    });
    let page = tab.page.id.clone();
    assert!(s.editor_backspace_empty(&id));
    let tab = s.tab().unwrap();
    assert_eq!(tab.blocks.len(), 1);
    assert_eq!(tab.blocks[0].block.id, id);
    assert!(tab.blocks[0].edit.focused);
    let saved = s.store.get_blocks(&page).unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].id, id);
    assert_ne!(saved[0].id, divider);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn backspace_on_empty_paragraph_focuses_previous_text_and_first_line_is_unchanged() {
    let (mut s, path) = fixture();
    let tab = &mut s.tabs[s.active];
    tab.blocks[0].edit.replace_text("Before");
    let paragraph = moss_core::Block::paragraph(&tab.page.id, "", 1);
    let id = paragraph.id.clone();
    let mut edit = TextEdit::new("").rich();
    edit.focused = true;
    tab.blocks.push(crate::state::EditableBlock {
        block: paragraph,
        edit,
    });
    let page = tab.page.id.clone();
    assert!(s.editor_backspace_empty(&id));
    let tab = s.tab().unwrap();
    assert_eq!(tab.blocks.len(), 1);
    assert!(tab.blocks[0].edit.focused);
    assert_eq!(tab.blocks[0].edit.range(), (6, 6));
    assert_eq!(s.store.get_blocks(&page).unwrap()[0].text, "Before");
    let first = tab.blocks[0].block.id.clone();
    s.tabs[s.active].blocks[0].edit.replace_text("");
    assert!(!s.editor_backspace_empty(&first));
    assert_eq!(s.tab().unwrap().blocks.len(), 1);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
