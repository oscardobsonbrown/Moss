use rune::*;
#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub sidebar: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub line: Color,
    pub hover: Color,
    pub blue: Color,
    pub selected: Color,
}
fn hex(v: u32) -> Color {
    Color {
        r: rune::color::srgb_decode(((v >> 16) & 255) as f32 / 255.),
        g: rune::color::srgb_decode(((v >> 8) & 255) as f32 / 255.),
        b: rune::color::srgb_decode((v & 255) as f32 / 255.),
        a: 1.,
    }
}
impl Theme {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                bg: hex(0x191919),
                sidebar: hex(0x202020),
                text: hex(0xD4D4D4),
                muted: hex(0x9B9B9B),
                faint: hex(0x6E6E6E),
                line: hex(0x303030),
                hover: hex(0x2C2C2C),
                blue: hex(0x529CCA),
                selected: hex(0x2C2C2C),
            }
        } else {
            Self {
                bg: Color::WHITE,
                sidebar: hex(0xF7F7F5),
                text: hex(0x37352F),
                muted: hex(0x787774),
                faint: hex(0x9B9A97),
                line: hex(0xE9E9E7),
                hover: hex(0xEFEFED),
                blue: hex(0x2383E2),
                selected: hex(0xE8E8E5),
            }
        }
    }
}
pub fn label(s: impl Into<String>, size: f32, color: Color) -> El {
    text(s).font(TextStyle::new(size, 400.)).color(color)
}
pub fn button(key: impl Into<String>, title: impl Into<String>, t: Theme) -> El {
    let title = title.into();
    row()
        .key(key)
        .role("button")
        .label(&title)
        .focusable(true)
        .h(30.)
        .px(9.)
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .center_y()
        .child(label(title, 14., t.text))
}
pub fn icon_button(key: impl Into<String>, title: &str, icon: &str, t: Theme) -> El {
    row()
        .key(key)
        .role("button")
        .label(title)
        .focusable(true)
        .size(28., 28.)
        .center()
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .child(super::icons::glyph(icon, 18., t.muted))
}

pub fn primary_button(key: impl Into<String>, title: impl Into<String>, t: Theme) -> El {
    button(
        key,
        title,
        Theme {
            text: Color::WHITE,
            ..t
        },
    )
    .fill(t.blue)
    .hover(t.blue)
    .press(t.blue)
}
