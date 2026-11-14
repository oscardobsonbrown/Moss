use super::*;

#[test]
fn sidebar_controls_and_history_work_at_narrow_and_wide_sizes() {
    for dark in [false, true] {
        let (s, path) = fixture();
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
        for width in [850., 1440.] {
            rt.set_viewport((width, 700.), 1.);
            rt.invalidate();
            rt.frame(1. / 120.);
            let header = rt.presented("sidebar-header").unwrap().0;
            let search = rt.presented("search-button").unwrap().0;
            let tabs = rt.presented("tab-strip").unwrap().0;
            let compose = rt.presented("sidebar-compose").unwrap().0;
            let section_action = rt.presented("sidebar-manage-sections").unwrap().0;
            assert!(header[1] + header[3] <= search[1]);
            assert!(tabs[0] >= search[0] + search[2]);
            assert!((compose[0] + compose[2] / 2. - section_action[0] - section_action[2] / 2.).abs() < 1.);
            click(&mut rt, "sidebar-collapse");
            assert!(rt.state.sidebar_collapsed());
            assert!(rt.find("sidebar").is_some());
            assert!(rt.find("sidebar-expand").is_some());
            click(&mut rt, "sidebar-expand");
            assert!(rt.find("sidebar").is_some());
        }
        let first = rt.state.tab().unwrap().page.id.clone();
        click(&mut rt, "sidebar-compose");
        let second = rt.state.tab().unwrap().page.id.clone();
        assert_ne!(first, second);
        click(&mut rt, "history-back");
        assert_eq!(rt.state.tab().unwrap().page.id, first);
        click(&mut rt, "history-forward");
        assert_eq!(rt.state.tab().unwrap().page.id, second);
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
