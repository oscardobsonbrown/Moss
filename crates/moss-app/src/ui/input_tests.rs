use super::super::theme::Theme;
use super::*;
use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime, TextEdit, TextStyle, col};

struct Fixture {
    edit: TextEdit,
    dark: bool,
    multiline: bool,
}

fn view(s: &Fixture, _: &mut Cx<Fixture>) -> El {
    let t = Theme::new(s.dark);
    let input = if s.multiline {
        text(&s.edit, TextStyle::new(14., 400.).lh(20.), t).key("input")
    } else {
        field(&s.edit, "input", "Type here", t)
    };
    col().w_full().h_full().padding(20.).child(
        col()
            .key("container")
            .w(120.)
            .child(input.w_full().on_edit(|s: &mut Fixture| &mut s.edit)),
    )
}

fn runtime(dark: bool, scale: f32, multiline: bool) -> Runtime<Fixture> {
    let mut edit = TextEdit::new("");
    edit.focused = true;
    let mut rt = Runtime::with_text(
        Fixture {
            edit,
            dark,
            multiline,
        },
        view,
        (300., 300.),
        Env {
            system_dark: dark,
            scale,
        },
        rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
    );
    rt.frame(0.);
    rt
}

fn element(rt: &Runtime<Fixture>, key: &str) -> rune::runtime::ElementInfo {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap()
}

fn key(rt: &mut Runtime<Fixture>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(1. / 60.);
}

#[test]
fn field_caret_stays_inside_centered_field_during_utf8_edit_and_selection() {
    for dark in [false, true] {
        for scale in [1., 2.] {
            let mut rt = runtime(dark, scale, false);
            let check_caret = |rt: &Runtime<Fixture>| {
                let field = element(rt, "input");
                let caret = field.caret.expect("focused field has a caret");
                let [x, y, w, h] = field.rect;
                assert!((h - 32.).abs() < 0.1, "field: {:?}", field.rect);
                assert!(
                    caret[0] >= x && caret[0] + caret[2] <= x + w + 0.1,
                    "caret {caret:?}, field {:?}",
                    field.rect
                );
                let top = caret[1] - y;
                let bottom = y + h - caret[1] - caret[3];
                assert!(
                    top >= 0. && bottom >= 0.,
                    "caret {caret:?}, field {:?}",
                    field.rect
                );
                assert!((top - bottom).abs() <= 2., "caret insets {top}, {bottom}");
                caret
            };
            let start = check_caret(&rt);
            let value = "Long UTF-8 text: café 東京 🌿 repeated to overflow";
            key(&mut rt, Key::Text(value.into()), Mods::default());
            assert_eq!(rt.state.edit.text, value);
            let end = check_caret(&rt);
            assert!(
                end[0] > start[0] + 30.,
                "caret follows text to the visible right edge"
            );
            key(
                &mut rt,
                Key::Command('a'),
                Mods {
                    cmd: true,
                    ..Mods::default()
                },
            );
            assert_eq!(rt.state.edit.anchor.min(rt.state.edit.focus), 0);
            assert_eq!(rt.state.edit.anchor.max(rt.state.edit.focus), value.len());
            key(&mut rt, Key::Text("é".into()), Mods::default());
            assert_eq!(rt.state.edit.text, "é");
            check_caret(&rt);
        }
    }
}

#[test]
fn multiline_text_wraps_and_grows_and_keyboard_replaces_selection() {
    for dark in [false, true] {
        for scale in [1., 2.] {
            let mut rt = runtime(dark, scale, true);
            let initial = element(&rt, "input").target;
            let value =
                "A long document title with café and several words to wrap onto multiple lines.";
            key(&mut rt, Key::Text(value.into()), Mods::default());
            let wrapped = element(&rt, "input").target;
            assert!(wrapped[2] <= 120.1, "wrapped field: {wrapped:?}");
            assert!(
                wrapped[3] > initial[3] + 20.,
                "initial {initial:?}, wrapped {wrapped:?}"
            );
            assert_eq!(rt.state.edit.text, value);
            key(
                &mut rt,
                Key::Command('a'),
                Mods {
                    cmd: true,
                    ..Mods::default()
                },
            );
            assert_eq!(rt.state.edit.anchor.max(rt.state.edit.focus), value.len());
            key(&mut rt, Key::Text("New text".into()), Mods::default());
            assert_eq!(rt.state.edit.text, "New text");
        }
    }
}

#[test]
fn unboxed_line_stays_one_line_when_text_overflows() {
    fn line_view(s: &Fixture, _: &mut Cx<Fixture>) -> El {
        col().padding(20.).child(
            line(
                &s.edit,
                TextStyle::new(14., 400.).lh(20.),
                Theme::new(s.dark),
            )
            .key("input")
            .w(120.)
            .on_edit(|s: &mut Fixture| &mut s.edit),
        )
    }
    for dark in [false, true] {
        for scale in [1., 2.] {
            let mut edit = TextEdit::new("");
            edit.focused = true;
            let mut rt = Runtime::with_text(
                Fixture {
                    edit,
                    dark,
                    multiline: false,
                },
                line_view,
                (300., 300.),
                Env {
                    system_dark: dark,
                    scale,
                },
                rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
            );
            rt.frame(0.);
            key(
                &mut rt,
                Key::Text("A long single line with café and more words".into()),
                Mods::default(),
            );
            let input = element(&rt, "input");
            assert!(
                (input.target[3] - 20.).abs() < 0.1,
                "line: {:?}",
                input.target
            );
            let caret = input.caret.expect("focused line has a caret");
            assert!(
                caret[0] >= input.rect[0]
                    && caret[0] + caret[2] <= input.rect[0] + input.rect[2] + 0.1
            );
        }
    }
}
