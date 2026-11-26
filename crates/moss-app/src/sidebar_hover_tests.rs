use super::*;

fn item(rt: &Runtime<State>, key: &str) -> rune::ElementInfo {
    rt.elements()
        .into_iter()
        .find(|el| el.key.as_deref() == Some(key))
        .unwrap()
}

#[test]
fn sidebar_actions_reveal_on_row_hover_and_strengthen_on_direct_hover() {
    for dark in [false, true] {
        for scale in [1., 2.] {
            let (mut state, path) = fixture();
            let page = state
                .store
                .create_page(
                    &state.workspace.id,
                    None,
                    "Hover page",
                    "📄",
                    PageKind::Document,
                )
                .unwrap();
            state
                .store
                .save_blocks(&page.id, &[moss_core::Block::paragraph(&page.id, "", 0)])
                .unwrap();
            state.refresh_pages();
            state.open_page(&page.id, true);
            state.toggle_sidebar_favorite(&page.id);
            let section = state.create_sidebar_section("Study").unwrap();
            state.add_page_to_sidebar_section(&section, &page.id);
            let mut rt = Runtime::new(
                state,
                ui::view,
                (1100., 900.),
                Env {
                    system_dark: dark,
                    scale,
                },
            );
            rt.frame(0.);
            for prefix in [
                "page".to_owned(),
                "favorite".into(),
                "recent".into(),
                format!("section-{section}"),
            ] {
                let action = format!("{prefix}-actions-{}", page.id);
                let plus = format!("{prefix}-add-{}", page.id);
                rt.pointer_move(1000., 800.);
                rt.frame(0.);
                assert!(item(&rt, &format!("{action}-glyph")).opacity < 0.01);
                assert!(item(&rt, &format!("{plus}-glyph")).opacity < 0.01);
                let (x, y) = rt.find(&format!("{prefix}-open-{}", page.id)).unwrap();
                rt.pointer_move(x, y);
                rt.frame(0.);
                assert!(item(&rt, &format!("{action}-glyph")).opacity > 0.99);
                assert!(item(&rt, &format!("{plus}-glyph")).opacity > 0.99);
                for control in [&action, &plus] {
                    let visual = format!("{control}-visual");
                    assert!(item(&rt, &visual).hover < 0.01);
                    let (x, y) = rt.find(control).unwrap();
                    rt.pointer_move(x, y);
                    rt.frame(0.);
                    assert!(item(&rt, &visual).hover > 0.99);
                    let (x, y) = rt.find(&format!("{prefix}-open-{}", page.id)).unwrap();
                    rt.pointer_move(x, y);
                    rt.frame(0.);
                    assert!(item(&rt, &visual).hover < 0.01);
                }
            }
            for section in ["recents", "favorites", "private", &section] {
                let action = format!("section-actions-{section}");
                rt.pointer_move(1000., 800.);
                rt.frame(0.);
                assert!(item(&rt, &format!("{action}-glyph")).opacity < 0.01);
                let (x, y) = rt.find(&format!("{section}-heading")).unwrap();
                rt.pointer_move(x, y);
                rt.frame(0.);
                assert!(item(&rt, &format!("{action}-glyph")).opacity > 0.99);
                assert!(item(&rt, &format!("{action}-visual")).hover < 0.01);
                let (x, y) = rt.find(&action).unwrap();
                rt.pointer_move(x, y);
                rt.frame(0.);
                assert!(item(&rt, &format!("{action}-visual")).hover > 0.99);
            }
            let title = format!("title-{}", page.id);
            click(&mut rt, &title);
            rt.pointer_move(1000., 800.);
            rt.frame(0.);
            rt.set_full_keyboard_access(true);
            let action = format!("page-actions-{}", page.id);
            let mut reached = false;
            for _ in 0..100 {
                press(&mut rt, Key::Tab);
                if rt
                    .elements()
                    .iter()
                    .any(|node| node.focused && node.key.as_deref() == Some(action.as_str()))
                {
                    reached = true;
                    break;
                }
            }
            assert!(reached, "page actions must remain reachable with Tab");
            assert!(rt.state.sidebar_keyboard_controls);
            assert!(item(&rt, &format!("page-actions-{}-glyph", page.id)).opacity > 0.99);
            rt.pointer_move(1001., 800.);
            rt.frame(0.);
            assert!(item(&rt, &format!("page-actions-{}-glyph", page.id)).opacity < 0.01);
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}

#[test]
fn sidebar_hover_uses_separate_full_width_rows_for_nested_pages() {
    let (mut state, path) = fixture();
    let parent = state
        .store
        .create_page(
            &state.workspace.id,
            None,
            "Parent",
            "📄",
            PageKind::Document,
        )
        .unwrap();
    let child = state
        .store
        .create_page(
            &state.workspace.id,
            Some(&parent.id),
            "Child",
            "📄",
            PageKind::Document,
        )
        .unwrap();
    state.refresh_pages();
    state.toggle_sidebar_page(&parent.id);
    state.open_page(&parent.id, true);
    let mut rt = Runtime::new(
        state,
        ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.frame(0.);
    let parent_key = format!("page-highlight-{}", parent.id);
    let child_key = format!("page-highlight-{}", child.id);
    let parent_row = item(&rt, &parent_key);
    let child_row = item(&rt, &child_key);
    assert_eq!(parent_row.rect[0], child_row.rect[0]);
    assert_eq!(parent_row.rect[2], child_row.rect[2]);
    assert_eq!(parent_row.rect[3], 28.);
    assert_eq!(child_row.rect[3], 28.);
    assert!(child_row.rect[1] >= parent_row.rect[1] + parent_row.rect[3] + 2.);
    let selected_fill = parent_row.fill;
    let idle_fill = child_row.fill;
    assert_ne!(selected_fill, idle_fill);
    assert_eq!(parent_row.radius, 7.);
    assert_eq!(child_row.radius, 7.);
    let (x, y) = rt.find(&format!("page-open-{}", child.id)).unwrap();
    rt.pointer_move(x, y);
    rt.frame(0.);
    assert_eq!(item(&rt, &child_key).hover, 1.);
    assert_eq!(item(&rt, &parent_key).hover, 0.);
    let (x, y) = rt.find(&format!("page-open-{}", parent.id)).unwrap();
    rt.pointer_move(x, y);
    rt.frame(0.);
    assert_eq!(item(&rt, &parent_key).hover, 1.);
    assert_eq!(item(&rt, &child_key).hover, 0.);
    let (x, y) = rt.find(&format!("page-open-{}", child.id)).unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.pointer_move(1000., 800.);
    rt.frame(0.);
    assert_eq!(rt.state.tab().unwrap().page.id, child.id);
    assert_eq!(item(&rt, &parent_key).fill, idle_fill);
    assert_eq!(item(&rt, &child_key).fill, selected_fill);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn sidebar_click_press_effect_highlights_row_and_clears_on_release_or_drag() {
    for dark in [false, true] {
        for scale in [1., 2.] {
            let (mut state, path) = fixture();
            let first = state
                .store
                .create_page(
                    &state.workspace.id,
                    None,
                    "Command Centre",
                    "🕹️",
                    PageKind::Document,
                )
                .unwrap();
            let second = state
                .store
                .create_page(
                    &state.workspace.id,
                    None,
                    "Wishlist",
                    "🎁",
                    PageKind::Document,
                )
                .unwrap();
            state
                .store
                .create_page(
                    &state.workspace.id,
                    Some(&first.id),
                    "Subpage",
                    "📄",
                    PageKind::Document,
                )
                .unwrap();
            state.refresh_pages();
            state.toggle_sidebar_favorite(&first.id);
            state.toggle_sidebar_favorite(&second.id);
            state.open_page(&first.id, false);
            let mut rt = Runtime::new(
                state,
                ui::view,
                (1200., 800.),
                Env {
                    system_dark: dark,
                    scale,
                },
            );
            rt.frame(0.);

            let fav_second_highlight = format!("favorite-highlight-{}", second.id);
            let fav_second_press = format!("{fav_second_highlight}-press");
            assert!(item(&rt, &fav_second_press).opacity < 0.01);

            // Pressing down on a favorite row illuminates the full row pill via live_only without rebuilding view.
            let live_before = rt.live_only_events();
            let (x, y) = rt.find(&format!("favorite-open-{}", second.id)).unwrap();
            rt.pointer_move(x, y);
            rt.frame(0.);
            rt.pointer_down(x, y);
            rt.frame(0.);
            let press_layer = item(&rt, &fav_second_press);
            let row_box = item(&rt, &fav_second_highlight);
            assert_eq!(press_layer.rect, row_box.rect);
            assert!(press_layer.opacity > 0.99);
            assert!(press_layer.fill.a > 0.01);
            assert!(
                rt.live_only_events() > live_before,
                "pointer_down press highlight must stay on the live-only fast path"
            );

            // Releasing activates Wishlist and clears the press overlay.
            rt.pointer_up(x, y);
            rt.frame(0.);
            assert_eq!(rt.state.tab().unwrap().page.id, second.id);
            assert!(item(&rt, &fav_second_press).opacity < 0.01);

            // Pressing an already-active row still applies the press overlay on top of active+hover.
            let (x, y) = rt.find(&format!("favorite-open-{}", second.id)).unwrap();
            rt.pointer_move(x, y);
            rt.frame(0.);
            rt.pointer_down(x, y);
            rt.frame(0.);
            assert!(item(&rt, &fav_second_press).opacity > 0.99);
            rt.pointer_up(x, y);
            rt.frame(0.);
            assert!(item(&rt, &fav_second_press).opacity < 0.01);

            // Pressing an inner disclosure button illuminates both the row and the disclosure control.
            let fav_first_highlight = format!("favorite-highlight-{}", first.id);
            let fav_first_press = format!("{fav_first_highlight}-press");
            let disclosure_key = format!("favorite-disclosure-{}", first.id);
            let (dx, dy) = rt.find(&disclosure_key).unwrap();
            rt.pointer_move(dx, dy);
            rt.frame(0.);
            rt.pointer_down(dx, dy);
            for _ in 0..15 {
                rt.frame(1. / 60.);
            }
            assert!(item(&rt, &fav_first_press).opacity > 0.99);
            let dp = item(&rt, &disclosure_key).press;
            assert!(dp > 0.9, "expected disclosure press > 0.9, got {dp}");
            rt.pointer_up(dx, dy);
            rt.frame(0.);
            assert!(item(&rt, &fav_first_press).opacity < 0.01);

            // Pressing the trailing actions button illuminates both the row and the button.
            let actions_key = format!("favorite-actions-{}", second.id);
            let (ax, ay) = rt.find(&actions_key).unwrap();
            rt.pointer_move(ax, ay);
            rt.frame(0.);
            rt.pointer_down(ax, ay);
            for _ in 0..15 {
                rt.frame(1. / 60.);
            }
            assert!(item(&rt, &fav_second_press).opacity > 0.99);
            let ap = item(&rt, &actions_key).press;
            assert!(ap > 0.9, "expected actions press > 0.9, got {ap}");
            rt.pointer_up(ax, ay);
            rt.frame(0.);
            assert!(item(&rt, &fav_second_press).opacity < 0.01);
            assert!(rt.state.sidebar_page_menu.is_some());
            rt.state.close_sidebar_page_menu();
            rt.invalidate();
            rt.frame(0.);

            // Dragging past the threshold clears the press overlay cleanly.
            let (px, py) = rt.find(&format!("page-open-{}", second.id)).unwrap();
            let page_press = format!("page-highlight-{}-press", second.id);
            rt.pointer_down(px, py);
            rt.frame(0.);
            assert!(item(&rt, &page_press).opacity > 0.99);
            rt.pointer_move(px, py + 12.);
            rt.frame(0.);
            assert!(item(&rt, &page_press).opacity < 0.01);
            rt.pointer_up(px, py + 12.);
            rt.frame(0.);
            assert!(
                rt.live_only_warnings().is_empty(),
                "unexpected live-only warnings: {:?}",
                rt.live_only_warnings()
            );

            // Active page title uses brighter ink than inactive page titles in both sections.
            let active_fav_title = item(&rt, &format!("favorite-title-{}", second.id));
            let inactive_fav_title = item(&rt, &format!("favorite-title-{}", first.id));
            let active_priv_title = item(&rt, &format!("page-title-{}", second.id));
            assert_eq!(active_fav_title.text_color, active_priv_title.text_color);
            assert_ne!(active_fav_title.text_color, inactive_fav_title.text_color);

            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}
