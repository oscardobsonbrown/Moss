use rotion_core::{PageFont, PageSettings};
use rune::{Family, TextStyle};

pub const SANS: &[u8] = include_bytes!("../assets/fonts/Geist.ttf");
pub const MONO: &[u8] = include_bytes!("../assets/fonts/GeistMono.ttf");
pub const SERIF: &[u8] = include_bytes!("../assets/fonts/GeistSerif.ttf");
pub const FONTS: &[&[u8]] = &[
    SANS,
    include_bytes!("../assets/fonts/Geist-Italic.ttf"),
    MONO,
    include_bytes!("../assets/fonts/GeistMono-Italic.ttf"),
    SERIF,
];
pub const FAMILIES: (&str, &str) = ("Geist", "Geist Mono");

pub fn family(font: PageFont) -> Family {
    match font {
        PageFont::Default => Family::Sans,
        PageFont::Serif => Family::Named("Geist Serif"),
        PageFont::Mono => Family::Mono,
    }
}
pub fn scale(settings: PageSettings) -> f32 {
    if settings.small_text { 0.875 } else { 1. }
}
pub fn page_style(settings: PageSettings, size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size * scale(settings), weight).family(family(settings.font))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renderer_uses_the_bundled_fonts_instead_of_fallbacks() {
        let mut system = rune::text::TextSystem::new(FONTS, FAMILIES);
        for (font, bytes) in [
            (PageFont::Default, SANS),
            (PageFont::Serif, SERIF),
            (PageFont::Mono, MONO),
        ] {
            let layout = system.layout_rich(
                "Ag Reading 0123",
                &[],
                page_style(
                    PageSettings {
                        font,
                        ..Default::default()
                    },
                    16.,
                    400.,
                ),
                1.,
                None,
            );
            let mut count = 0;
            for line in layout.lines() {
                for run in line.runs() {
                    assert_eq!(run.font().data.as_ref(), bytes, "wrong font for {font:?}");
                    count += 1;
                }
            }
            assert!(count > 0);
        }
        let serif = page_style(
            PageSettings {
                font: PageFont::Serif,
                ..Default::default()
            },
            16.,
            400.,
        );
        let inline = system.layout_rich(
            "code",
            &[rune::Span {
                start: 0,
                end: 4,
                mono: true,
                ..Default::default()
            }],
            serif,
            1.,
            None,
        );
        let block = system.layout_rich("code", &[], serif.mono(), 1., None);
        for layout in [inline, block] {
            for line in layout.lines() {
                for run in line.runs() {
                    assert_eq!(
                        run.font().data.as_ref(),
                        MONO,
                        "code must use Geist Mono even on a Serif page"
                    );
                }
            }
        }
    }
}
