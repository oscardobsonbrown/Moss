use super::*;

fn sample_document() -> (State, PathBuf, Vec<String>) {
    let (mut state, path) = fixture();
    state.new_page(false, None);
    let active = state.active;
    let page_id = state.tabs[active].page.id.clone();
    state.tabs[active].page.icon = "🌱".into();
    let kinds = [
        BlockKind::Paragraph,
        BlockKind::Heading1,
        BlockKind::Heading2,
        BlockKind::Heading3,
        BlockKind::BulletedList,
        BlockKind::NumberedList,
        BlockKind::Todo,
        BlockKind::Callout,
        BlockKind::Code,
        BlockKind::Divider,
    ];
    let blocks: Vec<_> = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            let mut block = moss_core::Block::paragraph(
                &page_id,
                format!("Sample block {index}"),
                index as i64,
            );
            block.kind = kind;
            let edit = TextEdit::new(&block.text);
            state::EditableBlock { block, edit }
        })
        .collect();
    let ids = blocks.iter().map(|b| b.block.id.clone()).collect();
    state.tabs[active].blocks = blocks;
    (state, path, ids)
}

fn runtime(state: State, width: f32, dark: bool, scale: f32) -> Runtime<State> {
    let text = rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES);
    let mut rt = Runtime::with_text(
        state,
        ui::view,
        (width, 1400.),
        Env {
            system_dark: dark,
            scale,
        },
        text,
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    rt
}

fn info(rt: &Runtime<State>, key: &str) -> rune::runtime::ElementInfo {
    rt.elements()
        .into_iter()
        .find(|node| node.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("missing {key}"))
}

#[test]
fn empty_block_hint_requires_hover_or_focus_and_typing_stays_visible() {
    for dark in [false, true] {
        for scale in [1., 2.] {
            let (mut state, path) = fixture();
            state.new_page(false, None);
            let page = state.tab().unwrap().page.id.clone();
            let id = state.tab().unwrap().blocks[0].block.id.clone();
            let input = format!("block-input-{id}");
            let hint = format!("block-hint-{id}");
            let mut rt = runtime(state, 850., dark, scale);
            click(&mut rt, &format!("title-{page}"));
            assert!(rt.state.tab().unwrap().title.focused);
            rt.pointer_move(840., 1390.);
            rt.frame(0.);
            assert!(info(&rt, &hint).opacity < 0.01);
            let rect = info(&rt, &input).rect;
            rt.pointer_move(rect[0] + 4., rect[1] + rect[3] / 2.);
            rt.frame(0.);
            assert!(info(&rt, &hint).opacity > 0.99);
            rt.pointer_move(840., 1390.);
            rt.frame(0.);
            assert!(info(&rt, &hint).opacity < 0.01);
            click(&mut rt, &input);
            rt.pointer_move(840., 1390.);
            rt.frame(0.);
            assert!(rt.state.tab().unwrap().blocks[0].edit.focused);
            assert!(info(&rt, &hint).opacity > 0.99);
            press(&mut rt, Key::Text("A block".into()));
            click(&mut rt, &format!("title-{page}"));
            rt.pointer_move(840., 1390.);
            rt.frame(0.);
            assert_eq!(rt.state.tab().unwrap().blocks[0].edit.text, "A block");
            assert!(rt.find(&hint).is_none());
            assert!(info(&rt, &input).opacity > 0.99);
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}

#[test]
fn page_content_edges_remain_aligned_across_window_and_font_modes() {
    for width in [850., 1440.] {
        for dark in [false, true] {
            for scale in [1., 2.] {
                for full_width in [false, true] {
                    for font in [PageFont::Default, PageFont::Serif, PageFont::Mono] {
                        let (mut state, path, ids) = sample_document();
                        let page = state.tab().unwrap().page.id.clone();
                        let active = state.active;
                        state.tabs[active].page.settings.full_width = full_width;
                        state.tabs[active].page.settings.font = font;
                        let rt = runtime(state, width, dark, scale);
                        let title = info(&rt, &format!("title-{page}")).rect;
                        let icon = info(&rt, &format!("page-header-icon-{page}")).rect;
                        assert!(
                            (icon[0] - title[0]).abs() <= 0.6,
                            "icon/title: {icon:?} {title:?}"
                        );
                        for id in &ids[..4] {
                            let input = info(&rt, &format!("block-input-{id}")).rect;
                            assert!(
                                (input[0] - title[0]).abs() <= 0.6,
                                "paragraph/heading: {input:?} {title:?}"
                            );
                        }
                        for (id, expected_inset) in ids[4..9].iter().zip([36., 36., 36., 48., 14.])
                        {
                            let input = info(&rt, &format!("block-input-{id}")).rect;
                            assert!(
                                (input[0] - title[0] - expected_inset).abs() <= 0.6,
                                "list, callout or code inset: {input:?} {title:?}; expected {expected_inset}"
                            );
                        }
                        for (index, id) in ids[4..7].iter().enumerate() {
                            let input = info(&rt, &format!("block-input-{id}")).rect;
                            let marker = if index == 2 {
                                info(&rt, &format!("todo-box-{id}")).rect
                            } else {
                                info(&rt, &format!("list-marker-{id}")).rect
                            };
                            assert!(
                                (marker[1] + marker[3] / 2. - input[1] - 13.).abs() <= 0.6,
                                "list marker must stay centered on the first line: {marker:?} {input:?}"
                            );
                        }
                        let divider = info(&rt, &format!("block-{}", ids[9])).rect;
                        assert!(
                            (divider[0] - title[0]).abs() <= 0.6,
                            "divider: {divider:?} {title:?}"
                        );
                        drop(rt);
                        std::fs::remove_dir_all(path).unwrap();
                    }
                }
            }
        }
    }
}

#[test]
fn block_gutter_controls_remain_reachable_and_keep_text_selection() {
    let (state, path, ids) = sample_document();
    let mut rt = runtime(state, 850., false, 2.);
    let input_key = format!("block-input-{}", ids[0]);
    click(&mut rt, &input_key);
    let active = rt.state.active;
    rt.state.tabs[active].blocks[0].edit.set_selection(0, 6);
    rt.frame(0.);
    let input = info(&rt, &input_key).rect;
    let plus_key = format!("newblock-add-{}", ids[0]);
    let plus = info(&rt, &plus_key).rect;
    let grip = info(&rt, "block-menu-0").rect;
    for (name, rect) in [("plus", plus), ("grip", grip)] {
        assert!(
            (rect[2] - 20.).abs() <= 0.1 && (rect[3] - 24.).abs() <= 0.1,
            "{name}: {rect:?}"
        );
    }
    let y = grip[1] + grip[3] / 2.;
    for x in [
        input[0] + 2.,
        input[0] - 2.,
        grip[0] + grip[2] / 2.,
        plus[0] + plus[2] / 2.,
    ] {
        rt.pointer_move(x, y);
        rt.frame(1. / 60.);
        assert!(info(&rt, &plus_key).opacity > 0.9, "plus hidden at {x}");
        assert!(
            info(&rt, "block-menu-0").opacity > 0.9,
            "grip hidden at {x}"
        );
        assert_eq!(rt.state.tab().unwrap().blocks[0].edit.range(), (0, 6));
        assert!(rt.state.tab().unwrap().blocks[0].edit.focused);
    }
    click(&mut rt, "block-menu-0");
    assert_eq!(rt.state.tab().unwrap().blocks[0].edit.range(), (0, 6));
    assert_eq!(
        rt.state.block_commands.as_ref().unwrap().mode,
        crate::block_commands::BlockCommandMode::Change
    );
    press(&mut rt, Key::Escape);
    click(&mut rt, &plus_key);
    assert_eq!(
        rt.state.block_commands.as_ref().unwrap().mode,
        crate::block_commands::BlockCommandMode::InsertAfter
    );
    assert_eq!(rt.state.tab().unwrap().blocks[0].edit.range(), (0, 6));
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn unfocused_gutter_stays_visible_between_text_and_both_controls() {
    let (state, path, ids) = sample_document();
    let page = state.tab().unwrap().page.id.clone();
    let mut rt = runtime(state, 850., false, 1.);
    click(&mut rt, &format!("title-{page}"));
    assert!(!rt.state.tab().unwrap().blocks[0].edit.focused);
    let input = info(&rt, &format!("block-input-{}", ids[0])).rect;
    let plus_key = format!("newblock-add-{}", ids[0]);
    let plus = info(&rt, &plus_key).rect;
    let grip = info(&rt, "block-menu-0").rect;
    let y = grip[1] + grip[3] / 2.;
    rt.pointer_move(input[0] + 2., y);
    for _ in 0..20 {
        rt.frame(1. / 60.);
    }
    let end = plus[0] + plus[2] / 2.;
    let mut x = input[0];
    while x >= end {
        rt.pointer_move(x, y);
        rt.frame(1. / 60.);
        assert!(info(&rt, &plus_key).opacity > 0.9, "plus hidden at {x}");
        assert!(
            info(&rt, "block-menu-0").opacity > 0.9,
            "grip hidden at {x}"
        );
        assert!(!rt.state.tab().unwrap().blocks[0].edit.focused);
        x -= 2.;
    }
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn page_customization_controls_align_and_show_hover_and_focus() {
    for dark in [false, true] {
        for scale in [1., 2.] {
            let (mut state, path) = fixture();
            state.new_page(false, None);
            state.tabs[state.active].page.icon.clear();
            state.tabs[state.active].title.focused = false;
            let page = state.tab().unwrap().page.id.clone();
            let icon = format!("page-header-icon-{page}");
            let cover = format!("page-cover-open-{page}-document");
            let mut rt = runtime(state, 850., dark, scale);
            rt.pointer_move(840., 1390.);
            rt.frame(0.3);
            assert!(info(&rt, &cover).opacity < 0.01);
            let title = info(&rt, &format!("title-{page}")).rect;
            let icon_label = info(&rt, &format!("{icon}-label"));
            let cover_label = info(&rt, &format!("{cover}-label"));
            assert!(
                (icon_label.rect[0] - title[0]).abs() < 1.,
                "{icon_label:?} {title:?}"
            );
            assert!((icon_label.rect[1] - cover_label.rect[1]).abs() < 1.);
            assert_eq!(icon_label.text_color, cover_label.text_color);
            let button = info(&rt, &icon).rect;
            rt.pointer_move(button[0] + button[2] / 2., button[1] + button[3] / 2.);
            rt.frame(0.);
            assert!(info(&rt, &icon).hover > 0.99);
            assert!(info(&rt, &cover).opacity > 0.99);
            click(&mut rt, &format!("title-{page}"));
            rt.pointer_move(840., 1390.);
            rt.frame(0.2);
            assert!(info(&rt, &cover).opacity > 0.99);
            let block_id = rt.state.tab().unwrap().blocks[0].block.id.clone();
            click(&mut rt, &format!("block-input-{block_id}"));
            rt.set_full_keyboard_access(true);
            for _ in 0..100 {
                if info(&rt, &cover).focused {
                    break;
                }
                press(&mut rt, Key::Tab);
            }
            let focused = info(&rt, &cover);
            assert!(focused.focused && focused.focus_visible, "{focused:?}");
            assert!(focused.opacity > 0.99, "{focused:?}");
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}

#[test]
fn record_page_customization_controls_align_with_title() {
    let (mut rt, path) = database_runtime(0);
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    rt.state.open_record(0);
    rt.invalidate();
    rt.frame(0.3);
    let page = rt.state.record_peek.as_ref().unwrap().page.id.clone();
    rt.state.record_peek.as_mut().unwrap().title.focused = false;
    rt.invalidate();
    rt.pointer_move(5., 5.);
    rt.frame(0.3);
    let cover_key = format!("page-cover-open-{page}-peek");
    assert!(info(&rt, &cover_key).opacity < 0.01);
    let title = info(&rt, &format!("peek-title-{page}")).rect;
    let icon = info(&rt, &format!("peek-icon-{page}-label")).rect;
    let cover = info(&rt, &format!("page-cover-open-{page}-peek-label")).rect;
    assert!((icon[0] - title[0]).abs() < 1., "{icon:?} {title:?}");
    assert!((icon[1] - cover[1]).abs() < 1.);
    rt.pointer_move(title[0] + 2., title[1] + title[3] / 2.);
    rt.frame(0.3);
    assert!(info(&rt, &cover_key).opacity > 0.99);
    click(&mut rt, &format!("peek-title-{page}"));
    rt.pointer_move(5., 5.);
    rt.frame(0.3);
    assert!(info(&rt, &cover_key).opacity > 0.99);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn record_metadata_icons_and_values_align_in_both_peek_modes() {
    for (mode, width, scale) in [("center", 1440., 1.), ("side", 850., 2.)] {
        let (mut rt, path) = database_runtime(0);
        rt.set_viewport((width, 1000.), scale);
        rt.state.create_inline(Default::default(), false);
        assert!(rt.state.finish_cell(false));
        let db = rt.state.tab().unwrap().database.clone().unwrap();
        let record = rt.state.tab().unwrap().records[0].id.clone();
        rt.state.open_database_record(&db.id, &record, mode);
        rt.invalidate();
        rt.frame(0.3);
        rt.frame(0.3);
        let props: Vec<_> = db
            .properties
            .iter()
            .filter(|p| p.kind != PropertyKind::Title)
            .collect();
        let mut previous: Option<f32> = None;
        for prop in props {
            let name = info(&rt, &format!("peek-property-name-{}", prop.id)).rect;
            let icon = info(&rt, &format!("peek-property-icon-{}", prop.id)).rect;
            let row = info(&rt, &format!("peek-property-row-{}", prop.id)).rect;
            let value = info(&rt, &format!("peek-property-{}", prop.id)).rect;
            assert!(
                (name[0] - icon[0]).abs() < 1. && (icon[2] - 16.).abs() < 1.,
                "{name:?} {icon:?}"
            );
            assert!(
                (value[0] - name[0] - name[2]).abs() < 1.,
                "{name:?} {value:?}"
            );
            assert!(
                value[2] >= 64.,
                "values must remain usable in a narrow peek: {value:?}"
            );
            if let Some(last) = previous {
                assert!(
                    (row[1] - last - 38.).abs() < 1.,
                    "metadata rows must have even spacing: {row:?}"
                );
            }
            previous = Some(row[1]);
        }
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
