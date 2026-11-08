use super::*;
use moss_core::simple_table::SimpleTable;

#[test]
fn slash_table_edits_and_persists_at_both_scales() {
    for scale in [1., 2.] {
        let (mut s, path) = fixture();
        s.new_page(false, None);
        let id = s.tab().unwrap().blocks[0].block.id.clone();
        let page = s.tab().unwrap().page.id.clone();
        let mut rt = Runtime::with_text(
            s,
            ui::view,
            (1100., 800.),
            Env {
                system_dark: false,
                scale,
            },
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.on_key = Some(key);
        rt.frame(0.);
        click(&mut rt, &format!("block-input-{id}"));
        press(&mut rt, Key::Text("/".into()));
        press(&mut rt, Key::Text("table".into()));
        press(&mut rt, Key::Enter);
        assert_eq!(
            rt.state.tab().unwrap().blocks[0].block.kind,
            BlockKind::SimpleTable
        );
        assert!(rt.find("simple-table-input").is_some());
        press(&mut rt, Key::Text("Alpha".into()));
        press(&mut rt, Key::Tab);
        press(&mut rt, Key::Text("Beta".into()));
        press(&mut rt, Key::Enter);
        let blocks = rt.state.store.get_blocks(&page).unwrap();
        let table = SimpleTable::read(&blocks[0]).unwrap();
        assert_eq!((&*table.rows[0][0], &*table.rows[0][1]), ("Alpha", "Beta"));
        click(&mut rt, &format!("simple-insert-row-{id}-0"));
        let table = SimpleTable::read(&rt.state.store.get_blocks(&page).unwrap()[0]).unwrap();
        assert_eq!(table.rows.len(), 4);
        click(&mut rt, &format!("simple-delete-row-{id}-1"));
        assert!(rt.find("simple-delete-confirm").is_some());
        click(&mut rt, "simple-delete-cancel");
        assert_eq!(
            SimpleTable::read(&rt.state.store.get_blocks(&page).unwrap()[0])
                .unwrap()
                .rows
                .len(),
            4
        );
        click(&mut rt, &format!("simple-delete-row-{id}-1"));
        click(&mut rt, "simple-delete-confirm");
        assert_eq!(
            SimpleTable::read(&rt.state.store.get_blocks(&page).unwrap()[0])
                .unwrap()
                .rows
                .len(),
            3
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn markdown_paste_into_empty_editor_paragraph_creates_table() {
    let (mut s, path) = fixture();
    s.new_page(false, None);
    let id = s.tab().unwrap().blocks[0].block.id.clone();
    let page = s.tab().unwrap().page.id.clone();
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (1100., 800.),
        Env {
            system_dark: false,
            scale: 2.,
        },
        rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, &format!("block-input-{id}"));
    rt.paste("| A | B |\n| --- | --- |\n| C | D |");
    rt.frame(1. / 60.);
    let blocks = rt.state.store.get_blocks(&page).unwrap();
    assert_eq!(blocks[0].kind, BlockKind::SimpleTable);
    assert_eq!(SimpleTable::read(&blocks[0]).unwrap().rows[1][1], "D");
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
