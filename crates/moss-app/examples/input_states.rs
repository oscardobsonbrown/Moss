//! Render with: cargo run -p moss --example input_states -- --snap /tmp/inputs.png --theme dark --scale 2

#[allow(dead_code)]
#[path = "../src/fonts.rs"]
mod fonts;
#[allow(dead_code)]
#[allow(unused_imports)]
#[path = "../src/ui/icons.rs"]
mod icons;
#[allow(dead_code)]
#[path = "../src/ui/inputs.rs"]
mod inputs;
#[allow(dead_code)]
#[path = "../src/ui/phosphor.rs"]
mod phosphor;
#[allow(dead_code)]
#[path = "../src/ui/theme.rs"]
mod theme;

use rune::{AppSpec, Cx, El, TextEdit, TextStyle, col, row};

struct State {
    edits: Vec<TextEdit>,
}

fn init() -> State {
    let mut edits: Vec<_> = [
        "",
        "A filled input",
        "Focused input",
        "A long input with café and 東京 that overflows the field width",
        "Invalid value",
        "Unavailable input",
        "Table cell text",
        "A page title that wraps to a second line at this width",
        "A body paragraph that wraps across multiple lines. Edit this text to inspect the caret and selection.",
        "fn main() {\n    println!(\"Hello, world!\");\n}",
    ]
    .into_iter()
    .map(TextEdit::new)
    .collect();
    edits[2].focused = true;
    State { edits }
}

fn sample(label: &str, input: El, t: theme::Theme) -> El {
    col().gap(4.).child(t.label(label, t.muted)).child(input)
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let t = theme::Theme::new(cx.system_dark());
    let mut fields = col().w(340.).gap(12.);
    for (i, label) in [
        "Empty",
        "Filled",
        "Focused",
        "Long / selected",
        "Error",
        "Disabled",
    ]
    .into_iter()
    .enumerate()
    {
        let field = inputs::field(&s.edits[i], format!("field-{i}"), "Type here", t)
            .on_edit(move |s: &mut State| &mut s.edits[i])
            .disabled(i == 5)
            .opacity(if i == 5 { 0.45 } else { 1. });
        fields = fields.child(sample(label, inputs::invalid(field, i == 4, t), t));
    }
    let style = TextStyle::new(14., 400.).lh(20.);
    let line = inputs::line(&s.edits[6], style, t)
        .key("table-line")
        .w_full()
        .on_edit(|s: &mut State| &mut s.edits[6]);
    let title = inputs::text(&s.edits[7], TextStyle::new(28., 600.).lh(34.), t)
        .key("title")
        .w_full()
        .on_edit(|s: &mut State| &mut s.edits[7]);
    let body = inputs::text(&s.edits[8], TextStyle::new(16., 400.).lh(24.), t)
        .key("body")
        .w_full()
        .on_edit(|s: &mut State| &mut s.edits[8]);
    let code = inputs::text(&s.edits[9], style.mono(), t)
        .key("code")
        .w_full()
        .on_edit(|s: &mut State| &mut s.edits[9]);
    let examples = col()
        .w(360.)
        .gap(16.)
        .child(sample("Unboxed table line", line, t))
        .child(sample("Wrapped title", title, t))
        .child(sample("Wrapped body", body, t))
        .child(sample("Code", code, t));
    col()
        .w_full()
        .h_full()
        .padding(28.)
        .gap(20.)
        .fill(t.bg)
        .child(t.label("Shared input states", t.text))
        .child(row().gap(40.).child(fields).child(examples))
}

fn main() {
    rune::run(AppSpec {
        fonts: fonts::all(),
        families: fonts::FAMILIES,
        scenes: &[("selected", |s: &mut State| {
            s.edits[2].focused = false;
            let edit = &mut s.edits[3];
            edit.focused = true;
            edit.anchor = 0;
            edit.focus = edit.text.len();
        })],
        ..AppSpec::new("Input states", (800., 600.), init, view)
    });
}
