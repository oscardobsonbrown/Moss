use super::*;
use crate::settings::{SettingsDropdown, SettingsSection};
use moss_core::new_id;
use std::path::PathBuf;

fn runtime(size: (f32, f32), dark: bool, scale: f32) -> (Runtime<State>, PathBuf) {
    let path = std::env::temp_dir().join(format!("moss-settings-ui-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    state.prefs.dark = Some(dark);
    let mut rt = Runtime::with_text(
        state,
        ui::view,
        size,
        Env {
            system_dark: dark,
            scale,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    (rt, path)
}

fn click(rt: &mut Runtime<State>, name: &str) {
    let (x, y) = rt.find(name).unwrap_or_else(|| panic!("missing {name}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..60 {
        rt.frame(1. / 120.);
    }
}

fn info(rt: &Runtime<State>, name: &str) -> rune::runtime::ElementInfo {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(name) && !e.exiting)
        .unwrap_or_else(|| panic!("missing {name}"))
}

#[test]
fn settings_modal_uses_two_pane_layout_within_bounds_across_themes_and_sizes() {
    for dark in [false, true] {
        for (size, scale) in [((1440., 900.), 2.), ((850., 600.), 1.)] {
            let (mut rt, path) = runtime(size, dark, scale);
            click(&mut rt, "settings-button");
            settle(&mut rt);

            let modal = info(&rt, "modal").target;
            let sidebar = info(&rt, "settings-sidebar").target;
            let content = info(&rt, "settings-content").target;
            let close = info(&rt, "close-modal").target;

            assert!(
                modal[0] >= 0.
                    && modal[1] >= 0.
                    && modal[0] + modal[2] <= size.0 + 0.5
                    && modal[1] + modal[3] <= size.1 + 0.5,
                "modal out of bounds for {size:?}: {modal:?}"
            );
            assert!(
                sidebar[2] >= 200. && sidebar[2] <= 240.,
                "unexpected sidebar width: {sidebar:?}"
            );
            assert!(
                content[0] >= sidebar[0] + sidebar[2],
                "content pane must sit to the right of sidebar: {sidebar:?} vs {content:?}"
            );
            assert!(
                (sidebar[3] - content[3]).abs() <= 2.,
                "sidebar and content should share full dialog height: {sidebar:?} vs {content:?}"
            );
            assert!(
                close[0] + close[2] <= modal[0] + modal[2],
                "close button must sit inside modal: {close:?} vs {modal:?}"
            );
            assert!(rt.find("settings-page-preferences").is_some());

            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}

#[test]
fn settings_pointer_open_fades_and_command_comma_opens_instantly() {
    let (mut rt, path) = runtime((1200., 800.), false, 1.);

    click(&mut rt, "settings-button");
    let pointer_opacity = info(&rt, "modal").opacity;
    assert!(
        pointer_opacity < 1.,
        "pointer open should fade: {pointer_opacity}"
    );
    settle(&mut rt);

    press(&mut rt, Key::Escape);
    settle(&mut rt);
    assert!(rt.state.modal.is_none());

    rt.key(KeyEvent {
        key: Key::Command(','),
        mods: Mods::default(),
    });
    rt.frame(1. / 120.);
    assert_eq!(rt.state.modal, Some(state::Modal::Settings));
    let keyboard_opacity = info(&rt, "modal").opacity;
    assert!(
        keyboard_opacity >= 0.99,
        "Cmd+, should open instantly: {keyboard_opacity}"
    );

    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn appearance_dropdown_switches_theme_and_escape_dismisses_dropdown_first() {
    let (mut rt, path) = runtime((1200., 800.), false, 1.);
    rt.state.open_settings();
    rt.invalidate();
    rt.frame(0.3);

    click(&mut rt, "settings-appearance-trigger");
    assert_eq!(rt.state.settings.dropdown, Some(SettingsDropdown::Appearance));
    assert!(rt.find("settings-appearance-menu").is_some());

    // First Escape closes only the dropdown, keeping Settings open.
    press(&mut rt, Key::Escape);
    assert_eq!(rt.state.settings.dropdown, None);
    assert_eq!(rt.state.modal, Some(state::Modal::Settings));

    click(&mut rt, "settings-appearance-trigger");
    click(&mut rt, "theme-Dark");
    assert_eq!(rt.state.prefs.dark, Some(true));
    assert_eq!(rt.state.settings.dropdown, None);

    click(&mut rt, "settings-appearance-trigger");
    click(&mut rt, "theme-System");
    assert_eq!(rt.state.prefs.dark, None);

    // Escape with no dropdown closes Settings.
    press(&mut rt, Key::Escape);
    assert_eq!(rt.state.modal, None);

    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn general_section_renames_workspace_updates_icon_and_copies_details() {
    let (mut rt, path) = runtime((1200., 800.), false, 1.);
    rt.state.open_settings();
    rt.invalidate();
    rt.frame(0.3);

    click(&mut rt, "settings-nav-general");
    assert_eq!(rt.state.settings.section, SettingsSection::General);
    assert!(rt.find("settings-page-general").is_some());

    // Empty name is rejected with an inline error.
    rt.state.settings.workspace_name.replace_text("   ");
    assert!(!rt.state.save_workspace_name());
    rt.invalidate();
    rt.frame(0.01);
    assert!(rt.find("settings-workspace-name-error").is_some());

    // Valid name saves and persists.
    rt.state.settings.workspace_name.replace_text("Design Studio");
    rt.invalidate();
    rt.frame(0.01);
    click(&mut rt, "settings-workspace-save");
    assert_eq!(rt.state.workspace.name, "Design Studio");
    assert!(rt.state.settings.workspace_name_error.is_none());

    // Workspace icon picker updates icon.
    click(&mut rt, "settings-workspace-icon");
    assert_eq!(
        rt.state.settings.dropdown,
        Some(SettingsDropdown::WorkspaceIcon)
    );
    assert!(rt.find("settings-workspace-icon-picker").is_some());
    click(&mut rt, "settings-icon-choice-0-0");
    assert_eq!(rt.state.settings.dropdown, None);
    let chosen_icon = rt.state.workspace.icon.clone();
    assert!(!chosen_icon.is_empty());

    // Copy workspace ID copies exact ID.
    click(&mut rt, "settings-copy-workspace-id");
    let copied = rt.copy_request.take().expect("expected copy request");
    assert_eq!(copied.content().text(), rt.state.workspace.id);

    // Persists across reopen.
    let saved_id = rt.state.workspace.id.clone();
    drop(rt);
    let reopened = State::open(path.clone(), Some(&saved_id)).unwrap();
    assert_eq!(reopened.workspace.name, "Design Studio");
    assert_eq!(reopened.workspace.icon, chosen_icon);
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn connections_and_import_export_sections_render_all_controls() {
    let (mut rt, path) = runtime((1200., 800.), true, 1.);
    rt.state.open_settings();
    rt.invalidate();
    rt.frame(0.3);

    click(&mut rt, "settings-nav-connections");
    assert!(rt.find("settings-page-connections").is_some());
    assert!(rt.find("mcp-enabled").is_some());
    assert!(rt.find("agent-setup-copy-codex").is_some());
    assert!(rt.find("agent-setup-copy-json").is_some());
    let card = info(&rt, "agent-setup-copy-codex-card").target;
    let code_box = info(&rt, "agent-setup-copy-codex-code-box").target;
    assert!(
        code_box[0] >= card[0] && code_box[0] + code_box[2] <= card[0] + card[2] + 0.5,
        "code box must stay inside card bounds: {code_box:?} vs {card:?}"
    );

    click(&mut rt, "settings-nav-import-export");
    assert!(rt.find("settings-page-import-export").is_some());
    assert!(rt.find("export-workspace").is_some());
    assert!(rt.find("import-workspace").is_some());

    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn connections_scroll_emits_no_extra_layers_and_pointer_motion_avoids_recommits() {
    let (mut rt, path) = runtime((1440., 900.), true, 2.);
    let closed_layers = rt
        .instances
        .iter()
        .filter(|inst| inst.params[1] == rune::render::KIND_LAYER)
        .count();

    rt.state.open_settings();
    rt.state.select_settings_section(SettingsSection::Connections);
    rt.invalidate();
    for _ in 0..240 {
        rt.frame(1. / 120.);
    }

    let open_layers: Vec<_> = rt
        .instances
        .iter()
        .filter(|inst| inst.params[1] == rune::render::KIND_LAYER)
        .map(|inst| (inst.rect, inst.fill[3], inst.params))
        .collect();
    assert_eq!(
        open_layers.len(),
        closed_layers,
        "Settings dialog after settle should not keep offscreen GPU layers (closed={closed_layers}, open={open_layers:?})"
    );

    let (cx, cy) = rt
        .find("agent-setup-copy-codex-code-box")
        .expect("missing codex code box");
    let commits_before = rt.stats.commits;
    rt.pointer_move(cx, cy);
    rt.frame(1. / 120.);
    rt.pointer_move(cx + 12., cy + 4.);
    rt.frame(1. / 120.);
    assert_eq!(
        rt.stats.commits,
        commits_before,
        "pointer motion over Settings should not force view recommits when sidebar is expanded"
    );

    for step in 0..20 {
        rt.wheel_event(
            WheelEvent::new(-18., 0.)
                .at(cx, cy)
                .phase(if step == 0 {
                    GesturePhase::Began
                } else {
                    GesturePhase::Ended
                }),
        );
        rt.frame(1. / 120.);
    }
    rt.wheel(cx, cy, -30.);
    rt.frame(1. / 120.);

    let scroll_layers = rt
        .instances
        .iter()
        .filter(|inst| inst.params[1] == rune::render::KIND_LAYER)
        .count();
    assert_eq!(
        scroll_layers, closed_layers,
        "scrolling Settings should not allocate extra offscreen GPU layers (closed={closed_layers}, scroll={scroll_layers})"
    );
    assert_eq!(
        rt.stats.commits,
        commits_before,
        "scrolling Settings should not trigger full view recommits"
    );

    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
#[ignore = "renders visual review PNGs to /tmp/moss-settings-review"]
fn render_settings_review_snapshots() {
    use rune::render::{Offscreen, Renderer, write_png};
    let out = std::path::Path::new("/tmp/moss-settings-review");
    std::fs::create_dir_all(out).unwrap();
    let gpu = rune::render::Gpu::headless();
    let mut renderer = Renderer::new(&gpu.device, rune::render::OFFSCREEN_FORMAT);
    let cases = [
        ("preferences-light", false, SettingsSection::Preferences, None),
        (
            "preferences-dropdown-light",
            false,
            SettingsSection::Preferences,
            Some(SettingsDropdown::Appearance),
        ),
        ("preferences-dark", true, SettingsSection::Preferences, None),
        ("connections-dark", true, SettingsSection::Connections, None),
        ("general-light", false, SettingsSection::General, None),
        ("import-export-light", false, SettingsSection::ImportExport, None),
    ];
    for (name, dark, section, dropdown) in cases {
        let (mut rt, path) = runtime((1440., 900.), dark, 2.);
        rt.state.open_settings();
        rt.state.select_settings_section(section);
        if let Some(d) = dropdown {
            rt.state.toggle_settings_dropdown(d);
        }
        rt.invalidate();
        settle(&mut rt);
        let target = Offscreen::new(&gpu.device, 2880, 1800);
        renderer.upload_images(&gpu.device, &gpu.queue, &mut rt.images);
        renderer.prepare(
            &gpu.device,
            &gpu.queue,
            &mut rt.text.atlas,
            &rt.instances,
            (2880, 1800),
        );
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        renderer.draw(&mut encoder, &target.view, rt.clear_color());
        target.copy(&mut encoder);
        gpu.queue.submit(Some(encoder.finish()));
        write_png(
            &out.join(format!("{name}.png")),
            2880,
            1800,
            &target.read(&gpu.device),
        )
        .unwrap();
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
