use super::theme::Theme;
use rune::{El, TextEdit, TextStyle};

pub fn text(edit: &TextEdit, style: TextStyle, t: Theme) -> El {
    rune::text_input(edit, style)
        .min_w(0.)
        .color(t.text)
        .caret(
            if t.caret_visible {
                t.text
            } else {
                t.text.alpha(0.)
            },
            t.blue.alpha(0.25),
        )
        .no_focus_ring()
}

pub fn line(edit: &TextEdit, style: TextStyle, t: Theme) -> El {
    text(edit, style, t).single_line().h(style.line_px())
}

pub fn field(edit: &TextEdit, key: impl Into<String>, placeholder: &str, t: Theme) -> El {
    field_with_style(edit, key, placeholder, TextStyle::new(14., 400.).lh(20.), t)
}

pub fn field_with_style(
    edit: &TextEdit,
    key: impl Into<String>,
    placeholder: &str,
    style: TextStyle,
    t: Theme,
) -> El {
    let line_height = style.line_px();
    let height = (line_height + 12.).max(32.);
    text(edit, style, t)
        .key(key)
        .single_line()
        .w_full()
        .h(height)
        .px(10.)
        .py((height - line_height) / 2.)
        .radius(6.)
        .fill(t.sidebar)
        .border(1., t.line)
        .focus_within_ring(1., -1., t.blue)
        .placeholder(placeholder, t.input_hint)
}

pub fn invalid(field: El, invalid: bool, t: Theme) -> El {
    if invalid {
        field
            .invalid(true)
            .border(1., t.error)
            .focus_within_ring(1., -1., t.error)
    } else {
        field
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
