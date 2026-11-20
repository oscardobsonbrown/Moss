use super::*;
use moss_core::*;
use serde_json::json;
use std::path::PathBuf;

fn fixture() -> (State, PathBuf) {
    let path = std::env::temp_dir().join(format!("moss-test-{}", new_id()));
    (State::open(path.clone(), None).unwrap(), path)
}
fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    rt.frame(1. / 120.);
}
fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("missing element {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
}

#[test]
fn text_toolbar_formats_a_pointer_selection_and_persists_links() {
    use crate::text_colors::{Channel, Palette, active_color};
    use rune::edit::Mark;

    fn block(s: &State, peek: bool) -> &state::EditableBlock {
        if peek {
            &s.record_peek.as_ref().unwrap().blocks[0]
        } else {
            &s.tab().unwrap().blocks[0]
        }
    }

    for (mode, scale) in [
        (None, 1.),
        (None, 2.),
        (Some("center"), 1.),
        (Some("side"), 2.),
    ] {
        let text = "Café and another phrase";
        let (mut s, path) = fixture();
        s.new_page(mode.is_some(), None);
        let peek = mode.is_some();
        let editable = if let Some(mode) = mode {
            let db = s.tab().unwrap().database.as_ref().unwrap().id.clone();
            let record = s
                .store
                .create_record(&db, "Record", &Default::default())
                .unwrap();
            s.open_database_record(&db, &record.id, mode);
            &mut s.record_peek.as_mut().unwrap().blocks[0]
        } else {
            &mut s.tabs[s.active].blocks[0]
        };
        editable.edit.replace_text(text);
        let id = editable.block.id.clone();
        let page = editable.block.page_id.clone();
        let mut next = moss_core::Block::paragraph(&page, "Next item", 1);
        next.kind = BlockKind::NumberedList;
        let next = state::EditableBlock {
            edit: crate::rich_text::load(&next),
            block: next,
        };
        if peek {
            s.record_peek.as_mut().unwrap().blocks.push(next);
        } else {
            s.tabs[s.active].blocks.push(next);
        }
        let workspace = s.workspace.id.clone();
        let target = s
            .store
            .create_page(
                &s.workspace.id,
                None,
                "Link destination",
                "",
                PageKind::Document,
            )
            .unwrap();
        let destination = format!("moss-page:{}", target.id);
        s.refresh_pages();
        if peek {
            s.save_peek_blocks();
        } else {
            s.save_blocks();
        }
        let mut rt = Runtime::with_text(
            s,
            ui::view,
            (1100., 900.),
            Env {
                system_dark: scale == 2.,
                scale,
            },
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.on_key = Some(key);
        rt.frame(0.);
        rt.frame(0.02);
        let input_key = if peek {
            format!("peek-block-{id}")
        } else {
            format!("block-input-{id}")
        };
        let [left, top, _, height] = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(&input_key))
            .unwrap()
            .target;
        let y = top + height / 2.;
        rt.pointer_down(left + 1., y);
        rt.frame(1. / 120.);
        rt.pointer_move(left + 41., y);
        rt.pointer_up(left + 41., y);
        rt.frame(1. / 120.);
        rt.frame(1. / 120.);
        let selection = block(&rt.state, peek).edit.range();
        assert!(selection.0 < selection.1 && selection.1 < text.len());
        let [_, toolbar_y, _, toolbar_h] = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("text-toolbar"))
            .expect("selection toolbar")
            .target;
        assert!(toolbar_y + toolbar_h < top);

        let styles = [
            ("text-toolbar-bold", Mark::Bold),
            ("text-toolbar-italic", Mark::Italic),
            ("text-toolbar-underline", Mark::Underline),
            ("text-toolbar-strike", Mark::Strike),
            ("text-toolbar-code", Mark::Code),
        ];
        for (button, mark) in &styles {
            for applied in [true, false, true] {
                click(&mut rt, button);
                rt.frame(1. / 120.);
                let edit = &block(&rt.state, peek).edit;
                assert_eq!(edit.range(), selection, "{mode:?} {button}");
                assert_eq!(edit.text, text);
                assert_eq!(edit.has_mark(mark), applied, "{mode:?} {button}");
                if applied {
                    assert!(edit.marks().iter().any(|range| {
                        range.mark == *mark && (range.start, range.end) == selection
                    }));
                }
            }
        }

        let range = selection.0..selection.1;
        for (swatch, channel, color) in [
            ("text-Blue", Channel::Text, Some(Palette::Blue)),
            (
                "background-Yellow",
                Channel::Background,
                Some(Palette::Yellow),
            ),
            ("background-Default", Channel::Background, None),
            (
                "background-Yellow",
                Channel::Background,
                Some(Palette::Yellow),
            ),
        ] {
            click(&mut rt, "text-toolbar-color");
            rt.frame(1. / 120.);
            click(&mut rt, &format!("editor-color-{swatch}"));
            rt.frame(1. / 120.);
            let saved = &rt.state.store.get_blocks(&page).unwrap()[0];
            assert_eq!(active_color(saved, Some(range.clone()), channel), color);
            assert_eq!(
                active_color(saved, Some(selection.1..text.len()), channel),
                None
            );
            assert_eq!(active_color(saved, None, channel), None);
            assert_eq!(block(&rt.state, peek).edit.range(), selection);
        }
        click(
            &mut rt,
            if peek {
                "peek-block-menu-0"
            } else {
                "block-menu-0"
            },
        );
        for _ in 0..30 {
            rt.frame(1. / 120.);
        }
        click(&mut rt, "block-color");
        rt.frame(1. / 120.);
        click(&mut rt, "editor-color-background-Pink");
        rt.frame(1. / 120.);
        let saved = &rt.state.store.get_blocks(&page).unwrap()[0];
        assert_eq!(
            active_color(saved, None, Channel::Background),
            Some(Palette::Pink)
        );
        assert_eq!(
            active_color(saved, Some(range), Channel::Background),
            Some(Palette::Yellow)
        );

        click(&mut rt, "text-toolbar-link");
        press(&mut rt, Key::Text("https://example.org".into()));
        press(&mut rt, Key::Enter);
        let edit = &block(&rt.state, peek).edit;
        assert_eq!(edit.range(), selection);
        assert_eq!(edit.active_link(), Some("https://example.org"));
        rt.frame(1. / 120.);
        click(&mut rt, "text-toolbar-link");
        for _ in 0..30 {
            rt.frame(1. / 120.);
        }
        click(&mut rt, "block-remove-text-link");
        assert!(block(&rt.state, peek).edit.active_link().is_none());
        rt.frame(1. / 120.);
        click(&mut rt, "text-toolbar-link");
        press(&mut rt, Key::Text("Link destination".into()));
        press(&mut rt, Key::Enter);
        assert_eq!(
            block(&rt.state, peek).edit.active_link(),
            Some(destination.as_str())
        );
        rt.frame(1. / 120.);
        for (label, kind) in [
            ("Heading 1", BlockKind::Heading1),
            ("Heading 3", BlockKind::Heading3),
            ("To-do list", BlockKind::Todo),
            ("Bulleted list", BlockKind::BulletedList),
            ("Numbered list", BlockKind::NumberedList),
            ("Quote", BlockKind::Quote),
            ("Callout", BlockKind::Callout),
            ("Code", BlockKind::Code),
            ("Text", BlockKind::Paragraph),
            ("Heading 2", BlockKind::Heading2),
        ] {
            click(&mut rt, "text-toolbar-type");
            assert!(rt.find("block-kind-Toggle").is_none());
            press(&mut rt, Key::Text(label.into()));
            press(&mut rt, Key::Enter);
            rt.frame(1. / 120.);
            assert_eq!(block(&rt.state, peek).block.kind, kind, "{mode:?} {label}");
            assert_eq!(block(&rt.state, peek).edit.text, text);
            assert_eq!(rt.state.store.get_blocks(&page).unwrap()[0].kind, kind);
            if kind == BlockKind::NumberedList {
                assert!(
                    rt.elements()
                        .iter()
                        .any(|el| el.text.as_deref() == Some("2."))
                );
            }
        }
        click(&mut rt, "text-toolbar-comment");
        assert!(matches!(
            rt.state.comments.anchor,
            Some(moss_core::comments::CommentAnchor::Text { start, end, .. })
                if (start, end) == selection
        ));
        assert!(rt.find("text-toolbar").is_none());
        for _ in 0..30 {
            rt.frame(1. / 120.);
        }
        click(&mut rt, "comment-input");
        press(&mut rt, Key::Text("Keep this phrase.".into()));
        assert_eq!(
            rt.state.comments.input.text, "Keep this phrase.",
            "{mode:?}"
        );
        click(&mut rt, "comment-submit");
        let threads = rt.state.store.list_comments(&workspace, &page).unwrap();
        assert_eq!(threads.len(), 1, "{mode:?} {:?}", rt.state.comments.error);
        assert!(matches!(
            &threads[0].anchor,
            moss_core::comments::CommentAnchor::Text { block_id, start, end, quote }
                if block_id == &id && (*start, *end) == selection && quote == &text[*start..*end]
        ));

        drop(rt);
        let mut reopened = State::open(path.clone(), None).unwrap();
        reopened.open_page(&page, false);
        let edit = &mut reopened.tabs[reopened.active].blocks[0].edit;
        edit.set_selection(selection.0, selection.1);
        assert_eq!(edit.text, text);
        for (_, mark) in &styles {
            assert!(edit.has_mark(mark), "{mode:?} {mark:?}");
        }
        assert_eq!(edit.active_link(), Some(destination.as_str()));
        let mut reopened = Runtime::with_text(
            reopened,
            ui::view,
            (1100., 760.),
            Env {
                system_dark: scale == 2.,
                scale,
            },
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        reopened.frame(0.);
        reopened.frame(0.02);
        click(&mut reopened, &destination);
        assert_eq!(reopened.state.tab().unwrap().page.id, target.id);
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn create_edit_format_search_and_reopen_a_page() {
    let (s, path) = fixture();
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, "sidebar-compose");
    for _ in 0..30 {
        rt.frame(1. / 120.);
    }
    assert!(rt.find("modal").is_none());
    click(&mut rt, "new-page-title");
    press(&mut rt, Key::Text("Project notebook".into()));
    assert_eq!(rt.state.tab().unwrap().title.text, "Project notebook");
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.tab().unwrap().page.title, "Project notebook");
    let block = rt.state.tab().unwrap().blocks[0].block.id.clone();
    click(&mut rt, &format!("block-input-{block}"));
    press(&mut rt, Key::Text("Keep this note offline.".into()));
    rt.state.tabs[rt.state.active].blocks[0]
        .edit
        .set_selection(0, 4);
    press(&mut rt, Key::Command('b'));
    let id = rt.state.tab().unwrap().page.id.clone();
    assert!(
        rt.state.store.get_blocks(&id).unwrap()[0].config["html"]
            .as_str()
            .unwrap()
            .contains("<strong>")
    );
    rt.state.open_search();
    rt.state.search.replace_text("offline");
    rt.state.run_search();
    assert!(rt.state.hits.iter().any(|h| h.page_id == id));
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    let tab = reopened.tabs.iter().find(|t| t.page.id == id).unwrap();
    assert_eq!(tab.blocks[0].edit.text, "Keep this note offline.");
    assert!(!tab.blocks[0].edit.marks().is_empty());
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn rich_split_keeps_marks_and_replaces_selection() {
    let (mut s, path) = fixture();
    s.tabs[s.active].blocks[0].edit =
        TextEdit::from_html("<p><strong>alpha beta gamma</strong></p>");
    s.tabs[s.active].blocks[0].edit.set_selection(5, 10);
    s.add_block(0);
    assert_eq!(s.tab().unwrap().blocks[0].edit.text, "alpha");
    assert_eq!(s.tab().unwrap().blocks[1].edit.text, " gamma");
    assert!(!s.tab().unwrap().blocks[0].edit.marks().is_empty());
    assert!(!s.tab().unwrap().blocks[1].edit.marks().is_empty());
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn empty_cells_stay_empty_and_structured_cells_are_protected() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    s.create_inline(Default::default(), false);
    s.modal = None;
    let status = s
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
    s.edit_record(0, Some(status.clone()));
    s.cell.replace_text("");
    assert!(s.finish_cell(false));
    assert_eq!(
        s.tab().unwrap().records[0].values[&status],
        serde_json::Value::Null
    );
    s.tabs[s.active].records[0]
        .values
        .insert(status.clone(), json!({"start":"2026-11-01"}));
    s.store.save_record(&s.tabs[s.active].records[0]).unwrap();
    s.modal = None;
    s.edit_record(0, Some(status.clone()));
    assert!(s.editing_cell.is_none());
    assert_eq!(
        s.tab().unwrap().records[0].values[&status],
        json!({"start":"2026-11-01"})
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn closing_an_earlier_tab_keeps_the_active_page() {
    let (mut s, path) = fixture();
    s.new_name.replace_text("Second");
    s.new_page(false, None);
    s.new_name.replace_text("Third");
    s.new_page(false, None);
    let id = s.tab().unwrap().page.id.clone();
    s.close_tab(0);
    assert_eq!(s.tab().unwrap().page.id, id);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn workspace_windows_share_theme_and_keep_each_others_tabs() {
    let (mut first, path) = fixture();
    let workspace = first
        .store
        .create_workspace("Other workspace", "O")
        .unwrap();
    first.save_preferences();
    let mut second = State::open(path.clone(), Some(&workspace.id)).unwrap();
    second.shared_preferences = first.shared_preferences.clone();
    second.new_name.replace_text("Second workspace page");
    second.new_page(false, None);
    first.new_name.replace_text("First workspace page");
    first.new_page(false, None);
    first.set_theme(Some(true));
    second.save_preferences();
    let saved: state::Preferences =
        serde_json::from_slice(&std::fs::read(path.join("preferences.json")).unwrap()).unwrap();
    assert_eq!(saved.dark, Some(true));
    assert_eq!(saved.tabs[&first.workspace.id].len(), 2);
    assert_eq!(saved.tabs[&second.workspace.id].len(), 1);
    drop(first);
    drop(second);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn all_six_views_show_the_same_saved_record_on_the_first_frame() {
    let (mut s, path) = fixture();
    s.new_name.replace_text("Tasks");
    s.new_page(true, None);
    s.create_inline(Default::default(), false);
    s.cell.replace_text("Plan offline work");
    assert!(s.finish_cell(false));
    let db = s.tab().unwrap().database.as_ref().unwrap();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    let date = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    let mut record = s.tab().unwrap().records[0].clone();
    record.values.insert(status, json!("In progress"));
    record.values.insert(date, json!("2026-11-07"));
    s.store.save_record(&record).unwrap();
    s.query_records();
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.frame(0.);
    let board_key = format!("board-card-{}", record.id);
    for (i, visible) in [
        "cell-0-title",
        board_key.as_str(),
        "calendar-0",
        "timeline-open-0",
        "record-card-0",
        "list-0",
    ]
    .into_iter()
    .enumerate()
    {
        let tab_key = format!("db-view-{i}");
        if rt.find(&tab_key).is_some() {
            click(&mut rt, &tab_key);
        } else {
            click(&mut rt, "db-views");
            click(&mut rt, &format!("select-view-{i}"));
        }
        assert_eq!(rt.state.tab().unwrap().view, i);
        assert!(
            rt.find(visible).is_some(),
            "view {i} has no record on its first frame"
        );
        assert_eq!(rt.state.tab().unwrap().records[0].id, record.id);
    }
    assert_eq!(
        rt.state
            .store
            .get_record(&record.id)
            .unwrap()
            .unwrap()
            .title,
        "Plan offline work"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn page_appearance_is_immediate_independent_and_survives_reopen() {
    let (s, path) = fixture();
    let first = s.tab().unwrap().page.id.clone();
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 1.,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.frame(0.);
    let width = |rt: &Runtime<State>| {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(&format!("document-{first}")))
            .unwrap()
            .target[2]
    };
    let normal = width(&rt);
    click(&mut rt, "page-actions");
    for _ in 0..30 {
        rt.frame(1. / 120.);
    }
    click(&mut rt, "appearance-menu-item");
    for _ in 0..30 {
        rt.frame(1. / 120.);
    }
    click(&mut rt, "font-serif");
    assert_eq!(rt.state.tab().unwrap().page.settings.font, PageFont::Serif);
    assert!(rt.state.modal == Some(state::Modal::Appearance));
    click(&mut rt, "page-full-width");
    assert!(width(&rt) > normal + 200.);
    click(&mut rt, "page-small-text");
    let expected = PageSettings {
        font: PageFont::Serif,
        full_width: true,
        small_text: true,
    };
    assert_eq!(
        rt.state.store.get_page(&first).unwrap().unwrap().settings,
        expected
    );
    press(&mut rt, Key::Escape);
    assert!(rt.state.modal == Some(state::Modal::PageActions));
    press(&mut rt, Key::Escape);
    assert!(rt.state.modal.is_none());
    rt.state.new_name.replace_text("Another page");
    rt.state.new_page(false, None);
    assert_eq!(
        rt.state.tab().unwrap().page.settings,
        PageSettings::default()
    );
    rt.state.open_page(&first, false);
    assert_eq!(rt.state.tab().unwrap().page.settings, expected);
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    assert_eq!(
        reopened.store.get_page(&first).unwrap().unwrap().settings,
        expected
    );
    let archive = reopened
        .store
        .export_workspace(&reopened.workspace.id)
        .unwrap();
    drop(reopened);
    let mut imported = Store::in_memory().unwrap();
    let workspace = imported.import_workspace(&archive).unwrap();
    assert!(
        imported
            .list_pages(&workspace.id)
            .unwrap()
            .iter()
            .any(|p| p.settings == expected)
    );
    std::fs::remove_dir_all(path).unwrap();
}

fn database_runtime(view: usize) -> (Runtime<State>, PathBuf) {
    let (mut s, path) = fixture();
    s.new_name.replace_text("Creation checks");
    s.new_page(true, None);
    s.tabs[s.active].view = view;
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1440., 1000.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.poll = Some(records::poll);
    rt.frame(0.);
    (rt, path)
}

#[test]
fn table_and_list_create_inline_autosave_and_keep_blank_items() {
    for (view, footer) in [(0, "table-new"), (5, "list-new")] {
        let (mut rt, path) = database_runtime(view);
        click(&mut rt, footer);
        assert!(rt.state.modal.is_none());
        assert!(rt.state.record_peek.is_none());
        assert!(rt.state.cell.focused);
        assert!(rt.find("database-cell-editor").is_some());
        let id = rt.state.tab().unwrap().records[0].id.clone();
        press(&mut rt, Key::Text("An inline item".into()));
        assert_eq!(
            rt.state.store.get_record(&id).unwrap().unwrap().title,
            "An inline item"
        );
        press(&mut rt, Key::Enter);
        assert!(rt.state.editing_cell.is_none());
        assert_eq!(rt.state.tab().unwrap().records.len(), 1);
        click(&mut rt, footer);
        let blank = rt.state.tab().unwrap().records[1].id.clone();
        press(&mut rt, Key::Escape);
        assert!(rt.state.editing_cell.is_none());
        assert_eq!(
            rt.state.store.get_record(&blank).unwrap().unwrap().title,
            ""
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn board_enter_starts_another_card_with_the_same_group() {
    let (mut rt, path) = database_runtime(1);
    click(&mut rt, "board-new-In progress");
    let id = rt.state.tab().unwrap().records[0].id.clone();
    let status = rt
        .state
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
    press(&mut rt, Key::Text("First card".into()));
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.tab().unwrap().records.len(), 2);
    assert_eq!(
        rt.state.store.get_record(&id).unwrap().unwrap().title,
        "First card"
    );
    assert!(rt.state.cell.focused);
    assert_eq!(rt.state.cell.text, "");
    press(&mut rt, Key::Text("Second card".into()));
    press(&mut rt, Key::Escape);
    for record in &rt.state.tab().unwrap().records {
        assert_eq!(record.values[&status], json!("In progress"));
    }
    assert_eq!(rt.state.tab().unwrap().records[1].title, "Second card");
    assert!(rt.state.record_peek.is_none());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn toolbar_and_gallery_open_real_record_pages_with_saved_body() {
    for (view, control, mode) in [
        (0, "db-new", state::PeekMode::Side),
        (4, "gallery-new", state::PeekMode::Center),
    ] {
        let (mut rt, path) = database_runtime(view);
        click(&mut rt, control);
        assert!(rt.state.modal.is_none());
        let peek = rt.state.record_peek.as_ref().unwrap();
        assert!(peek.mode == mode);
        assert!(rt.presented("peek-scroll").unwrap().0[3] > 200.);
        let id = peek.record.id.clone();
        let page = peek.page.id.clone();
        press(&mut rt, Key::Text("Saved record page".into()));
        press(&mut rt, Key::Enter);
        assert!(
            rt.state.record_peek.as_ref().unwrap().blocks[0]
                .edit
                .focused
        );
        press(&mut rt, Key::Text("This is the page body.".into()));
        assert_eq!(
            rt.state.store.get_blocks(&page).unwrap()[0].text,
            "This is the page body."
        );
        assert_eq!(
            rt.state.store.get_record(&id).unwrap().unwrap().title,
            "Saved record page"
        );
        click(&mut rt, "peek-full-page");
        assert!(rt.state.record_peek.is_none());
        assert_eq!(rt.state.tab().unwrap().page.id, page);
        rt.state.tabs[rt.state.active]
            .title
            .replace_text("Renamed full page");
        rt.state.save_title();
        assert_eq!(
            rt.state
                .tabs
                .iter()
                .find(|t| t.database.is_some())
                .unwrap()
                .records[0]
                .title,
            "Renamed full page"
        );
        assert_eq!(
            rt.state.store.get_record(&id).unwrap().unwrap().title,
            "Renamed full page"
        );
        rt.state.save_preferences();
        drop(rt);
        let reopened = State::open(path.clone(), None).unwrap();
        assert!(
            reopened
                .tabs
                .iter()
                .any(|t| t.page.id == page && t.title.text == "Renamed full page")
        );
        assert!(!reopened.pages.iter().any(|p| p.id == page));
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn calendar_double_click_and_timeline_drag_inherit_dates() {
    let (mut rt, path) = database_runtime(2);
    let month = chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.state.tabs[rt.state.active].calendar_month = month;
    rt.frame(0.);
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
    click(&mut rt, "calendar-day-2026-11-20");
    assert!(rt.state.tab().unwrap().records.is_empty());
    click(&mut rt, "calendar-day-2026-11-20");
    let peek = rt
        .state
        .record_peek
        .as_ref()
        .expect("Calendar double-click must open a page");
    assert!(peek.mode == state::PeekMode::Center);
    assert_eq!(peek.record.values[&date], json!("2026-11-20"));
    rt.state.close_peek();
    rt.state.tabs[rt.state.active].view = 3;
    rt.state.tabs[rt.state.active].timeline_start = month;
    rt.invalidate();
    rt.frame(0.);
    let track = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("timeline-create-track"))
        .unwrap()
        .target;
    let day_width = track[2] / 30.;
    let x = track[0] + 2. * day_width + 3.;
    let y = track[1] + 15.;
    rt.pointer_down(x, y);
    rt.pointer_move(x + 4. * day_width, y);
    rt.pointer_up(x + 4. * day_width, y);
    rt.frame(1. / 120.);
    assert!(rt.state.record_peek.is_none());
    assert!(rt.state.editing_cell.is_some());
    assert_eq!(
        rt.state.tab().unwrap().records.last().unwrap().values[&date],
        json!({"start":"2026-11-03","end":"2026-11-07"})
    );
    press(&mut rt, Key::Text("Timeline item".into()));
    press(&mut rt, Key::Enter);
    assert!(rt.state.editing_cell.is_none());
    click(&mut rt, "timeline-new");
    assert!(
        !rt.state
            .tab()
            .unwrap()
            .records
            .last()
            .unwrap()
            .values
            .contains_key(&date)
    );
    press(&mut rt, Key::Escape);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn invalid_cell_keeps_its_input_and_blocks_navigation() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    s.create_inline(Default::default(), false);
    s.finish_cell(false);
    let date = s
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
    s.edit_record(0, Some(date.clone()));
    s.cell.replace_text("not a date");
    assert!(!s.finish_cell(false));
    assert!(s.editing_cell.is_some());
    assert!(s.cell.focused);
    let current = s.tab().unwrap().page.id.clone();
    let other = s.pages.iter().find(|p| p.id != current).unwrap().id.clone();
    s.open_page(&other, true);
    assert_eq!(s.tab().unwrap().page.id, current);
    s.cell.replace_text("2026-11-20");
    assert!(s.finish_cell(false));
    assert_eq!(
        s.tab().unwrap().records[0].values[&date],
        json!("2026-11-20")
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn table_inline_grid_and_keyboard_selection_stay_stable() {
    let (mut rt, path) = database_runtime(0);
    let tab = &mut rt.state.tabs[rt.state.active];
    tab.database
        .as_mut()
        .unwrap()
        .properties
        .sort_by_key(|p| match p.kind {
            PropertyKind::Title => 0,
            PropertyKind::Date => 1,
            PropertyKind::Status => 2,
            _ => 3,
        });
    rt.frame(1. / 120.);
    click(&mut rt, "table-new");
    let db = rt.state.tab().unwrap().database.as_ref().unwrap();
    let date = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
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
    let record = rt.state.tab().unwrap().records[0].id.clone();
    let title_key = "cell-0-title";
    let date_key = format!("cell-0-{date}");
    let title = rt.presented(title_key).unwrap().0;
    let adjacent = rt.presented(&date_key).unwrap().0;
    assert!((title[2] - 280.).abs() < 1.);
    assert!((adjacent[2] - 200.).abs() < 1.);
    assert!((title[3] - 38.).abs() < 1.);
    assert!((title[1] - adjacent[1]).abs() < 0.1);
    assert!((title[3] - adjacent[3]).abs() < 0.1);
    let input = rt.presented("database-cell-editor").unwrap().0;
    assert!(input[1] >= title[1] && input[1] + input[3] <= title[1] + title[3]);
    press(&mut rt, Key::Text("Task".into()));
    press(&mut rt, Key::Tab);
    assert!(rt.state.editing_cell.is_none());
    assert_eq!(
        rt.state.selected_cell,
        Some((record.clone(), Some(date.clone())))
    );
    assert!(rt.state.property_picker.is_none());
    assert_eq!(
        rt.state.store.get_record(&record).unwrap().unwrap().title,
        "Task"
    );
    assert_eq!(rt.presented(title_key).unwrap().0, title);
    assert_eq!(rt.presented(&date_key).unwrap().0, adjacent);
    let resting_text = rt.presented("cell-text-0-title").unwrap().0;
    assert!((input[1] - resting_text[1]).abs() < 1.);
    assert!(
        (input[0] - resting_text[0]).abs() < 1.,
        "input={input:?}, rest={resting_text:?}"
    );
    press(&mut rt, Key::Tab);
    assert_eq!(
        rt.state.selected_cell,
        Some((record.clone(), Some(status.clone())))
    );
    assert!(rt.state.property_picker.is_none());
    press(&mut rt, Key::Enter);
    assert_eq!(
        rt.state.property_picker.as_ref().unwrap().property_id,
        status
    );
    press(&mut rt, Key::Escape);
    rt.frame(1. / 120.);
    rt.state.edit_record_property(&record, Some(date.clone()));
    rt.state.cell.replace_text("bad date");
    rt.frame(1. / 120.);
    press(&mut rt, Key::Tab);
    assert_eq!(
        rt.state
            .editing_cell
            .as_ref()
            .unwrap()
            .property_id
            .as_deref(),
        Some(date.as_str())
    );
    assert!(rt.state.cell.focused);
    assert!(rt.state.selected_cell.is_none());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn failed_view_query_keeps_the_previous_view_and_records() {
    let (mut rt, path) = database_runtime(0);
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    let ids: Vec<_> = rt
        .state
        .tab()
        .unwrap()
        .records
        .iter()
        .map(|r| r.id.clone())
        .collect();
    rt.state.tabs[rt.state.active]
        .database
        .as_mut()
        .unwrap()
        .views[4]
        .query
        .filter = Some(FilterExpr::Rule(FilterRule {
        property_id: "missing".into(),
        op: FilterOp::Eq,
        value: json!("x"),
    }));
    assert!(!rt.state.select_database_view(4));
    assert_eq!(rt.state.tab().unwrap().view, 0);
    assert_eq!(
        rt.state
            .tab()
            .unwrap()
            .records
            .iter()
            .map(|r| r.id.clone())
            .collect::<Vec<_>>(),
        ids
    );
    assert!(rt.state.error.is_some());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn table_checkbox_toggles_and_protects_invalid_values() {
    let (mut rt, path) = database_runtime(0);
    let mut db = rt.state.tab().unwrap().database.clone().unwrap();
    let id = new_id();
    db.properties.insert(
        1,
        Property {
            id: id.clone(),
            name: "Done".into(),
            kind: PropertyKind::Checkbox,
            config: json!({}),
        },
    );
    rt.state.store.save_database(&db).unwrap();
    rt.state.tabs[rt.state.active].database = Some(db);
    rt.state.create_inline(Default::default(), false);
    rt.state.cell.replace_text("Fixture item");
    assert!(rt.state.finish_cell(false));
    let record_id = rt.state.tab().unwrap().records[0].id.clone();
    rt.invalidate();
    rt.frame(1. / 120.);
    click(&mut rt, &format!("cell-0-{id}"));
    assert_eq!(
        rt.state
            .store
            .get_record(&record_id)
            .unwrap()
            .unwrap()
            .values[&id],
        true
    );
    assert!(rt.state.editing_cell.is_none());
    click(&mut rt, &format!("cell-0-{id}"));
    assert_eq!(
        rt.state
            .store
            .get_record(&record_id)
            .unwrap()
            .unwrap()
            .values[&id],
        false
    );
    rt.state.selected_cell = Some((record_id.clone(), Some(id.clone())));
    assert!(rt.state.table_selection_key(&KeyEvent {
        key: Key::Enter,
        mods: Mods::default()
    }));
    assert_eq!(
        rt.state
            .store
            .get_record(&record_id)
            .unwrap()
            .unwrap()
            .values[&id],
        true
    );
    let mut record = rt.state.store.get_record(&record_id).unwrap().unwrap();
    record.values.insert(id.clone(), json!("unexpected"));
    rt.state.store.save_record(&record).unwrap();
    rt.state.toggle_table_checkbox(&record_id, &id);
    assert!(rt.state.error.is_some());
    assert_eq!(
        rt.state
            .store
            .get_record(&record_id)
            .unwrap()
            .unwrap()
            .values[&id],
        "unexpected"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn gallery_creation_keeps_new_record_visible_while_filter_is_active() {
    let (mut rt, path) = database_runtime(4);
    rt.state.filter.replace_text("does not match");
    assert!(rt.state.query_records());
    rt.invalidate();
    rt.frame(1. / 120.);
    click(&mut rt, "gallery-new");
    let id = rt.state.record_peek.as_ref().unwrap().record.id.clone();
    press(&mut rt, Key::Text("A new note".into()));
    press(&mut rt, Key::Enter);
    press(&mut rt, Key::Text("A saved body".into()));
    click(&mut rt, "peek-close");
    assert!(
        rt.state
            .tab()
            .unwrap()
            .records
            .iter()
            .any(|r| r.id == id && r.title == "A new note")
    );
    assert_eq!(
        rt.state
            .tab()
            .unwrap()
            .gallery_previews
            .get(&id)
            .map(String::as_str),
        Some("A saved body")
    );
    let record = rt.state.store.get_record(&id).unwrap().unwrap();
    assert_eq!(record.title, "A new note");
    assert_eq!(
        rt.state
            .store
            .get_blocks(record.page_id.as_deref().unwrap())
            .unwrap()[0]
            .text,
        "A saved body"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_today_returns_to_current_month() {
    let (mut rt, path) = database_runtime(2);
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    rt.invalidate();
    rt.frame(1. / 120.);
    click(&mut rt, "calendar-today");
    let today = chrono::Local::now().date_naive();
    assert_eq!(
        rt.state.tab().unwrap().calendar_month,
        chrono::Datelike::with_day(&today, 1).unwrap()
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_and_timeline_no_date_lists_search_and_open_records() {
    for (view, prefix) in [(2, "calendar"), (3, "timeline")] {
        let (mut rt, path) = database_runtime(view);
        for name in ["Find me", "Other"] {
            rt.state.create_inline(Default::default(), false);
            rt.state.cell.replace_text(name);
            assert!(rt.state.finish_cell(false));
        }
        rt.invalidate();
        rt.frame(1. / 120.);
        if view == 2 { click(&mut rt, "db-view-settings"); }
        click(&mut rt, &format!("{prefix}-no-date"));
        assert!(rt.find(&format!("{prefix}-no-date-search")).is_some());
        press(&mut rt, Key::Text("Find".into()));
        let index = rt
            .state
            .tab()
            .unwrap()
            .records
            .iter()
            .position(|r| r.title == "Find me")
            .unwrap();
        let other = rt
            .state
            .tab()
            .unwrap()
            .records
            .iter()
            .position(|r| r.title == "Other")
            .unwrap();
        assert!(rt.find(&format!("{prefix}-undated-open-{index}")).is_some());
        assert!(rt.find(&format!("{prefix}-undated-open-{other}")).is_none());
        click(&mut rt, &format!("{prefix}-undated-open-{index}"));
        assert!(!rt.state.undated_open, "{prefix} open after click");
        assert_eq!(
            rt.state.record_peek.as_ref().unwrap().record.title,
            "Find me"
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn board_and_list_render_shared_property_values() {
    for view in [1, 5] {
        let (mut rt, path) = database_runtime(view);
        rt.state.create_inline(Default::default(), false);
        assert!(rt.state.finish_cell(false));
        let record_id = rt.state.tab().unwrap().records[0].id.clone();
        let mut record = rt
            .state
            .store
            .rename_record(&record_id, "Linked target")
            .unwrap();
        let mut db = rt.state.tab().unwrap().database.clone().unwrap();
        let date = db
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Date)
            .unwrap()
            .id
            .clone();
        let relation = new_id();
        db.properties.push(Property {
            id: relation.clone(),
            name: "Related".into(),
            kind: PropertyKind::Relation,
            config: json!({"target_database_id": db.id}),
        });
        rt.state.store.save_database(&db).unwrap();
        rt.state.tabs[rt.state.active].database = Some(db);
        record.values.insert(date.clone(), json!("2026-11-07"));
        record.values.insert(relation.clone(), json!([record_id]));
        rt.state.store.save_record(&record).unwrap();
        assert!(rt.state.query_records());
        rt.invalidate();
        rt.frame(1. / 120.);
        let prefix = if view == 1 {
            format!("board-value-{record_id}")
        } else {
            "list-value-0".into()
        };
        assert!(rt.find(&format!("{prefix}-{date}")).is_some());
        assert!(rt.find(&format!("{prefix}-{relation}")).is_some());
        assert!(
            rt.elements()
                .iter()
                .any(|element| element.text.as_deref() == Some("Linked target"))
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

include!("test_modules.rs");
