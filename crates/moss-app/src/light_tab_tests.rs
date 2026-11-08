use super::*;

fn dark_selected() -> Color {
    let v = rune::color::srgb_decode(44. / 255.);
    Color {
        r: v,
        g: v,
        b: v,
        a: 1.,
    }
}

fn element(rt: &Runtime<State>, key: &str) -> rune::runtime::ElementInfo {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
        .unwrap_or_else(|| panic!("missing {key}"))
}

#[test]
fn light_tabs_have_a_raised_active_pill_and_hover_close() {
    for width in [850., 1440.] {
        let (mut state, path) = fixture();
        state
            .new_name
            .replace_text("A second page with a longer title");
        state.new_page(false, None);
        let mut rt = Runtime::new(
            state,
            ui::view,
            (width, 700.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.on_key = Some(key);
        rt.frame(1.);
        let active = element(&rt, "tab-1");
        let inactive = element(&rt, "tab-0");
        let indicator = element(&rt, "tab-indicator-1");
        assert!((active.target[3] - 28.).abs() < 0.1);
        assert!(active.target[2] <= 240. && inactive.target[2] <= 240.);
        assert!(active.target[0] >= rt.state.sidebar_width(width));
        assert_eq!(indicator.fill, Color::WHITE);
        assert!((indicator.radius - 7.).abs() < 0.1);
        assert_eq!(indicator.opacity, 1.);
        assert_eq!(element(&rt, "tab-indicator-0").opacity, 0.);
        assert_eq!(
            rt.presented_pose("tab-indicator-1")
                .unwrap()
                .shadows()
                .len(),
            1
        );
        assert_eq!(element(&rt, "close-tab-1").opacity, 1.);
        assert_eq!(element(&rt, "close-tab-0").opacity, 0.);
        assert!(!element(&rt, "close-tab-0").focusable);
        assert!(element(&rt, "close-tab-1").focusable);
        let [x, y, w, h] = inactive.rect;
        rt.pointer_move(x + w / 2., y + h / 2.);
        rt.frame(0.2);
        assert!(element(&rt, "close-tab-0").opacity > 0.99);
        drop(rt);
        if width == 1440. {
            if let Some(destination) = std::env::var_os("MOSS_TAB_CAPTURE_DIR") {
                std::fs::rename(&path, destination).unwrap();
                continue;
            }
        }
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn dark_tabs_keep_their_existing_indicator_and_close_visibility() {
    let (mut state, path) = fixture();
    state.new_name.replace_text("Second");
    state.new_page(false, None);
    let mut rt = Runtime::new(
        state,
        ui::view,
        (850., 700.),
        Env {
            system_dark: true,
            scale: 1.,
        },
    );
    rt.frame(1.);
    assert_eq!(element(&rt, "tab-indicator-1").fill, dark_selected());
    assert!(
        rt.presented_pose("tab-indicator-1")
            .unwrap()
            .shadows()
            .is_empty()
    );
    assert_eq!(element(&rt, "close-tab-1").opacity, 1.);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
