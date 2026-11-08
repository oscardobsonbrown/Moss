use super::*;

fn overlap(a: [f32; 4], b: [f32; 4]) -> Option<(f32, f32)> {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    (x1 > x0 + 2. && y1 > y0 + 2.).then_some(((x0 + x1) / 2., (y0 + y1) / 2.))
}

#[test]
fn table_overlays_receive_pointer_input_above_sticky_header_in_both_themes() {
    for dark in [false, true] {
        let (mut s, path) = fixture();
        s.new_page(true, None);
        let title = s.tab().unwrap().database.as_ref().unwrap().properties[0]
            .id
            .clone();
        s.open_database_property(Some(&title));
        s.set_table_config("freeze_through", json!(title));
        s.close_database_property();
        let mut rt = Runtime::new(
            s,
            ui::view,
            (850., 700.),
            Env {
                system_dark: dark,
                scale: 1.,
            },
        );
        rt.on_key = Some(key);
        rt.frame(0.);
        click(&mut rt, "db-views");
        rt.frame(1.);
        let header = rt.presented("table-header").unwrap().0;
        let input = rt.presented("view-search").unwrap().0;
        let point = overlap(input, header).expect("view search must overlap the sticky header");
        rt.pointer_down(point.0, point.1);
        rt.pointer_up(point.0, point.1);
        rt.frame(0.);
        press(&mut rt, Key::Text("Board".into()));
        assert_eq!(rt.state.view_search.text, "Board");
        click(&mut rt, "select-view-1");
        assert_eq!(rt.state.tab().unwrap().view, 1);
        assert!(rt.state.modal.is_none());
        // Return to Table, then check a New-menu action at the same overlap.
        rt.state.select_database_view(0);
        rt.invalidate();
        rt.frame(0.);
        click(&mut rt, "db-new-options");
        rt.frame(1.);
        let header = rt.presented("table-header").unwrap().0;
        let (action, point) = [
            "db-new-empty",
            "db-new-mode-side",
            "db-new-mode-center",
            "db-new-mode-full",
        ]
        .into_iter()
        .find_map(|key| {
            rt.presented(key)
                .and_then(|(rect, _)| overlap(rect, header).map(|p| (key, p)))
        })
        .expect("a New menu action must overlap the sticky header");
        rt.pointer_down(point.0, point.1);
        rt.pointer_up(point.0, point.1);
        rt.frame(0.);
        assert!(
            !rt.state.database_new_open,
            "{action} did not receive the click"
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
