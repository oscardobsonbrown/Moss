use super::*;

#[test]
fn table_new_remains_compact_across_sidebar_widths() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let mut rt = Runtime::new(
        s,
        ui::view,
        (850., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    let rect = |rt: &Runtime<State>, key: &str| rt.presented(key).unwrap().0;
    let mut widths = Vec::new();
    for (window, sidebar) in [(850., 200.), (850., 480.), (1440., 200.), (1440., 480.)] {
        rt.set_viewport((window, 700.), 1.);
        rt.state.set_sidebar_width(sidebar, window);
        rt.invalidate();
        rt.frame(1. / 120.);
        let footer = rect(&rt, "table-new");
        assert!(footer[2] >= 70. && footer[2] < 150., "footer: {footer:?}");
        widths.push(footer[2]);
    }
    assert!(widths.iter().all(|width| (*width - widths[0]).abs() < 0.1));
    click(&mut rt, "table-new");
    assert!(rt.state.editing_cell.is_some());
    assert!(rt.state.record_peek.is_none());
    assert!(rt.find("table-new-items").is_none());
    let cell = rect(&rt, "cell-0-title");
    let footer = rect(&rt, "table-new");
    assert!(
        (footer[1] - cell[1] - cell[3]).abs() < 1.,
        "row must meet the New page control: {cell:?} {footer:?}"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
