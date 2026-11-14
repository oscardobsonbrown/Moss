use rune::*;
#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub menu: Color,
    pub family: Family,
    pub text_scale: f32,
    pub caret_visible: bool,
    pub sidebar: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub line: Color,
    pub hover: Color,
    pub blue: Color,
    pub selected: Color,
    pub error: Color,
    pub input_hint: Color,
    pub title_hint: Color,
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
    pub fn label(self, s: impl Into<String>, color: Color) -> El {
        text(s)
            .font(TextStyle::new(14., 400.).family(self.family))
            .color(color)
    }

    pub fn sized_label(self, s: impl Into<String>, size: f32, color: Color) -> El {
        text(s)
            .font(TextStyle::new(size * self.text_scale, 400.).family(self.family))
            .color(color)
    }

    pub fn link_label(self, s: impl Into<String>, size: f32) -> El {
        let s = s.into();
        let underline = Span::new(0, s.len()).underline(self.muted);
        rich(s, vec![underline])
            .font(TextStyle::new(size * self.text_scale, 400.).family(self.family))
            .color(self.text)
    }

    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                bg: hex(0x191919),
                menu: hex(0x252525),
                family: Family::Sans,
                text_scale: 1.,
                caret_visible: true,
                sidebar: hex(0x202020),
                text: hex(0xD4D4D4),
                muted: hex(0x9B9B9B),
                faint: hex(0x6E6E6E),
                line: hex(0x303030),
                hover: hex(0x2C2C2C),
                blue: hex(0x2784E0),
                selected: hex(0x2C2C2C),
                error: hex(0xE16259),
                input_hint: hex(0x7A7A7A),
                title_hint: hex(0x4A4A4A),
            }
        } else {
            Self {
                bg: Color::WHITE,
                menu: Color::WHITE,
                family: Family::Sans,
                text_scale: 1.,
                caret_visible: true,
                sidebar: hex(0xF7F7F5),
                text: hex(0x37352F),
                muted: hex(0x787774),
                faint: hex(0x9B9A97),
                line: hex(0xE9E9E7),
                hover: hex(0xEFEFED),
                blue: hex(0x2784E0),
                selected: hex(0xE8E8E5),
                error: hex(0xC43C35),
                input_hint: hex(0x8F8E8A),
                title_hint: hex(0xB8B7B3),
            }
        }
    }
}
pub fn label(s: impl Into<String>, color: Color) -> El {
    sized_label(s, 14., color)
}
pub fn sized_label(s: impl Into<String>, size: f32, color: Color) -> El {
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
        .child(t.label(title, t.text))
}
pub fn link_button(key: impl Into<String>, title: impl Into<String>, t: Theme) -> El {
    let title = title.into();
    row()
        .key(key)
        .role("link")
        .label(&title)
        .focusable(true)
        .h(30.)
        .px(9.)
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .center_y()
        .child(t.link_label(title, 14.))
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
