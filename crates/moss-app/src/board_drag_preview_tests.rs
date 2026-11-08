use super::*;

#[test]
fn dragged_board_card_escapes_the_viewport_clip_and_cancels_outside_it() {
    for (dark, scale) in [(false, 1.), (true, 2.)] {
        let mut f = board_fixture(&[("Alpha", json!("Not started"), None)], false, dark, scale);
        let id = f.ids[0].clone();
        let viewport = rect(&f.rt, "db-body");
        let from = title_point(&f.rt, &id);
        let start = rect(&f.rt, &format!("board-card-{id}"));
        let changes = f.rt.state.store.local_change_count();
        for to in [
            [viewport[0] - 60., from[1]],
            [from[0], viewport[1] - 40.],
            [viewport[0] + viewport[2] - 30., from[1]],
            [from[0], viewport[1] + viewport[3] - 30.],
        ] {
            f.rt.pointer_down(from[0], from[1]);
            f.rt.frame(1. / 120.);
            f.rt.pointer_move(to[0], to[1]);
            f.rt.frame(1. / 120.);
            let elements = f.rt.elements();
            let ghost = elements.iter().find(|e| e.key.as_deref() == Some("board-drag-preview")).unwrap();
            assert!(ghost.inert);
            assert!((ghost.rect[0] - (start[0] + to[0] - from[0])).abs() < 1.);
            assert!((ghost.rect[1] - (start[1] + to[1] - from[1])).abs() < 1.);
            for (actual, expected) in ghost.visible.iter().zip(ghost.rect) {
                assert!((actual - expected).abs() < 1., "preview clipped: {:?}, expected {expected}", ghost.visible);
            }
            assert_eq!(elements.iter().find(|e| e.key.as_deref() == Some(&format!("board-card-{id}"))).unwrap().opacity, 0.);
            assert_eq!(rect(&f.rt, &format!("board-card-slot-{id}")), start);
            press(&mut f.rt, Key::Escape);
            f.rt.pointer_up(to[0], to[1]);
            f.rt.frame(1. / 120.);
            assert!(f.rt.find("board-drag-preview").is_none());
            assert!(f.rt.state.database_views.board_ui.drag.is_none());
            assert_eq!(f.rt.state.store.local_change_count(), changes);
        }
        drop(f.rt);
        std::fs::remove_dir_all(f.path).unwrap();
    }
}

#[test]
fn board_footer_follows_cards_and_lifting_does_not_reserve_a_return_slot() {
    for (dark, scale) in [(false, 1.), (true, 2.)] {
        let mut f = board_fixture(&[("Alpha", json!("Not started"), None), ("Beta", json!("Not started"), None)], false, dark, scale);
        let mut ordered = f.ids.clone();
        ordered.sort_by(|a, b| rect(&f.rt, &format!("board-card-{a}"))[1].total_cmp(&rect(&f.rt, &format!("board-card-{b}"))[1]));
        let id = ordered[0].clone();
        let other = ordered[1].clone();
        let header = rect(&f.rt, "board-heading-null-0");
        let empty_header = rect(&f.rt, "board-heading-null-2");
        let empty_footer = rect(&f.rt, "board-new-Done");
        assert!((empty_footer[1] - empty_header[1] - empty_header[3] - 6.).abs() < 1.);
        let before = rect(&f.rt, "board-new-Not started");
        let top = rect(&f.rt, &format!("board-card-{id}"));
        let other_before = rect(&f.rt, &format!("board-card-{other}"));
        let from = title_point(&f.rt, &id);
        let outside = [f.rt.state.database_views.board_ui.viewport[0] - 60., from[1]];
        f.rt.pointer_down(from[0], from[1]);
        f.rt.frame(1. / 120.);
        f.rt.pointer_move(outside[0], outside[1]);
        f.rt.frame(1. / 120.);
        let remaining = rect(&f.rt, &format!("board-card-{other}"));
        let footer = rect(&f.rt, "board-new-Not started");
        assert!((remaining[1] - top[1]).abs() < 1., "source {top:?}, remaining {remaining:?}, before {other_before:?}, footer {footer:?}, bucket {:?}", bucket(&f.rt, json!("Not started"), Value::Null));
        assert!((footer[1] - remaining[1] - remaining[3] - 6.).abs() < 1.);
        assert!(before[1] - footer[1] > top[3]);
        press(&mut f.rt, Key::Escape);
        f.rt.pointer_up(outside[0], outside[1]);
        f.rt.frame(1. / 120.);
        assert_eq!(rect(&f.rt, "board-new-Not started"), before);
        assert_eq!(rect(&f.rt, &format!("board-card-{other}")), other_before);
        drag_to(&mut f.rt, &id, [empty_footer[0] + 40., empty_footer[1] + 12.]);
        wait_board(&mut f.rt, |p| projected_in(p, &id, &json!("Done"), &Value::Null));
        let last = title_point(&f.rt, &other);
        f.rt.pointer_down(last[0], last[1]);
        f.rt.frame(1. / 120.);
        f.rt.pointer_move(outside[0], last[1]);
        f.rt.frame(1. / 120.);
        let footer = rect(&f.rt, "board-new-Not started");
        assert!((footer[1] - header[1] - header[3] - 6.).abs() < 1.);
        press(&mut f.rt, Key::Escape);
        f.rt.pointer_up(outside[0], last[1]);
        f.rt.frame(1. / 120.);
        drop(f.rt);
        std::fs::remove_dir_all(f.path).unwrap();
    }
}
