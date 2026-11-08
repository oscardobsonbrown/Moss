use super::phosphor;
use rune::{Color, El};

pub use super::phosphor::{FILE as PAGE_BLANK, FILE_TEXT as PAGE_CONTENT};

pub fn glyph(name: &str, size: f32, color: Color) -> El {
    let icon = match name {
        "⌕" => &phosphor::MAGNIFYING_GLASS,
        "⌂" => &phosphor::HOUSE,
        "♧" | "⌫" => &phosphor::TRASH,
        "⚙" => &phosphor::GEAR_SIX,
        "+" => &phosphor::PLUS,
        "×" | "✕" => &phosphor::X,
        "✓" => &phosphor::CHECK,
        "⠿" => &phosphor::DOTS_SIX_VERTICAL,
        "⋮" => &phosphor::DOTS_THREE_VERTICAL,
        "…" | "..." => &phosphor::DOTS_THREE,
        "⌄" | "▼" | "▾" => &phosphor::CARET_DOWN,
        "›" | "▶" | "▸" => &phosphor::CARET_RIGHT,
        "‹" => &phosphor::CARET_LEFT,
        "↗" => &phosphor::ARROW_UP_RIGHT,
        "←" => &phosphor::ARROW_LEFT,
        "→" => &phosphor::ARROW_RIGHT,
        "☺" => &phosphor::SMILEY,
        "☆" => &phosphor::STAR,
        "Aa" => &phosphor::TEXT_AA,
        "open-peek" => &phosphor::SIDEBAR_SIMPLE,
        _ => return super::theme::sized_label(name, size, color),
    };
    rune::icon(icon, size).color(color)
}
