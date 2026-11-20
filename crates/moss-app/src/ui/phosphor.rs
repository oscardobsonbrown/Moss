// Phosphor Regular. Vendored SVGs and license are in assets/phosphor.
use rune::Icon;

pub const HEXAGON_FILL: Icon = Icon {
    name: "phosphor-hexagon-fill",
    view: 256.,
    body: include_str!("../../assets/phosphor/fill/hexagon-fill.svg"),
};

macro_rules! icons {
    ($($id:ident => $name:literal),+ $(,)?) => {
        $(pub const $id: Icon = Icon {
            name: concat!("phosphor-", $name, "-regular"),
            view: 256.,
            body: include_str!(concat!("../../assets/phosphor/regular/", $name, ".svg")),
        };)+
        #[cfg(test)]
        const ALL: &[&Icon] = &[$(&$id),+];
    };
}

icons! {
    AIRPLANE_TILT => "airplane-tilt",
    ARROW_CLOCKWISE => "arrow-clockwise",
    ARROW_DOWN => "arrow-down",
    ARROW_LEFT => "arrow-left",
    ARROW_LINE_LEFT => "arrow-line-left",
    ARROW_LINE_RIGHT => "arrow-line-right",
    ARROW_RIGHT => "arrow-right",
    ARROW_UP => "arrow-up",
    ARROW_UP_RIGHT => "arrow-up-right",
    ARROWS_LEFT_RIGHT => "arrows-left-right",
    ARROWS_IN_LINE_VERTICAL => "arrows-in-line-vertical",
    ARROWS_OUT_LINE_HORIZONTAL => "arrows-out-line-horizontal",
    ARROWS_OUT_LINE_VERTICAL => "arrows-out-line-vertical",
    ARROWS_OUT_SIMPLE => "arrows-out-simple",
    BASKETBALL => "basketball",
    CALENDAR_BLANK => "calendar-blank",
    CARDS => "cards",
    CARET_DOWN => "caret-down",
    CARET_LEFT => "caret-left",
    CARET_RIGHT => "caret-right",
    CHART_BAR => "chart-bar",
    CHECK => "check",
    CHECK_SQUARE => "check-square",
    CIRCLE_HALF => "circle-half",
    CLOCK => "clock",
    CLOCK_COUNTER_CLOCKWISE => "clock-counter-clockwise",
    COLUMNS => "columns",
    CORNERS_OUT => "corners-out",
    COPY => "copy",
    DOTS_SIX_VERTICAL => "dots-six-vertical",
    DOTS_THREE => "dots-three",
    DOTS_THREE_VERTICAL => "dots-three-vertical",
    ENVELOPE => "envelope",
    EYE => "eye",
    EYE_SLASH => "eye-slash",
    FILE => "file",
    FILE_TEXT => "file-text",
    FLAG => "flag",
    FUNCTION => "function",
    FUNNEL_SIMPLE => "funnel-simple",
    GEAR_SIX => "gear-six",
    HASH => "hash",
    HEART => "heart",
    HOUSE => "house",
    IDENTIFICATION_CARD => "identification-card",
    IMAGES_SQUARE => "images-square",
    KANBAN => "kanban",
    LEAF => "leaf",
    LIGHTBULB => "lightbulb",
    LINK => "link",
    LIST_BULLETS => "list-bullets",
    LIST_NUMBERS => "list-numbers",
    MAGNIFYING_GLASS => "magnifying-glass",
    MAP_PIN => "map-pin",
    NOTE_PENCIL => "note-pencil",
    ORANGE_SLICE => "orange-slice",
    PHONE => "phone",
    PLUS => "plus",
    ROWS => "rows",
    SIDEBAR_SIMPLE => "sidebar-simple",
    SIGMA => "sigma",
    SLIDERS_HORIZONTAL => "sliders-horizontal",
    SMILEY => "smiley",
    SORT_ASCENDING => "sort-ascending",
    SQUARE => "square",
    SQUARES_FOUR => "squares-four",
    TABLE => "table",
    TAG => "tag",
    TEXT_AA => "text-aa",
    TEXT_ALIGN_LEFT => "text-align-left",
    TOGGLE_LEFT => "toggle-left",
    TRASH => "trash",
    USER => "user",
    X => "x",
    STAR => "star",
    PUSH_PIN => "push-pin",
    LIST_CHECKS => "list-checks",
    TEXT_H_ONE => "text-h-one",
    TEXT_H_TWO => "text-h-two",
    TEXT_H_THREE => "text-h-three",
    TEXT_T => "text-t",
    QUOTES => "quotes",
    CODE => "code",
    MINUS => "minus",

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_icons_render_at_control_sizes_and_both_scales() {
        for icon in ALL {
            for size in [12, 14, 15, 16, 18, 20, 24] {
                for scale in [1, 2] {
                    let px = size * scale;
                    let pixels = rune::text::icon_coverage(icon, px);
                    assert_eq!(pixels.len(), (px * px) as usize);
                    assert!(
                        pixels.iter().any(|alpha| *alpha > 0),
                        "{} at {px}px is blank",
                        icon.name
                    );
                    assert!(
                        pixels.iter().any(|alpha| *alpha == 0),
                        "{} at {px}px fills its box",
                        icon.name
                    );
                }
            }
        }
    }

    #[test]
    fn record_file_pair_keeps_exactly_two_body_lines() {
        let blank = rune::text::icon_coverage(&FILE, 256);
        let content = rune::text::icon_coverage(&FILE_TEXT, 256);
        let mut bands = 0;
        let mut ink = false;
        for y in 112..200 {
            assert_eq!(blank[y * 256 + 128], 0);
            let next = content[y * 256 + 128] > 127;
            if next && !ink {
                bands += 1;
            }
            ink = next;
        }
        assert_eq!(bands, 2);
    }
}
