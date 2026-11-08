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
