use super::{
    menu, phosphor,
    theme::{Theme, button},
};
use crate::state::State;
use moss_core::{Page, PageCover};
use rune::*;

pub(super) fn parse_color(value: &str) -> Option<Color> {
    let value = u32::from_str_radix(value.strip_prefix('#')?, 16).ok()?;
    Some(Color {
        r: rune::color::srgb_decode(((value >> 16) & 255) as f32 / 255.),
        g: rune::color::srgb_decode(((value >> 8) & 255) as f32 / 255.),
        b: rune::color::srgb_decode((value & 255) as f32 / 255.),
        a: 1.,
    })
}
fn reposition(start: f32, delta: f32, overflow: f32, cancelled: bool) -> f32 {
    if cancelled || overflow <= 0. {
        start
    } else {
        (start - delta / overflow).clamp(0., 1.)
    }
}
const BANNER_HEIGHT: f32 = 360.;
const LIGHT_FADE_HEIGHT: f32 = 240.;
const DARK_FADE_HEIGHT: f32 = 288.;
const LIGHT_FADE_BANDS: usize = 12;
const DARK_FADE_BANDS: usize = 16;
const GRADIENT_BANDS: usize = 9;

fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    let cubic = t * t * (3. - 2. * t);
    let quintic = t * t * t * (t * (t * 6. - 15.) + 10.);
    0.5 * (cubic + quintic)
}

fn cosine_ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    0.5 * (1. - (std::f32::consts::PI * t).cos())
}

fn fade_geometry(t: Theme) -> (f32, usize) {
    if t.dark {
        (DARK_FADE_HEIGHT, DARK_FADE_BANDS)
    } else {
        (LIGHT_FADE_HEIGHT, LIGHT_FADE_BANDS)
    }
}

fn lerp_color_srgb(a: Color, b: Color, t: f32) -> Color {
    let mix = |ca: f32, cb: f32| {
        let sa = rune::color::srgb_encode(ca);
        let sb = rune::color::srgb_encode(cb);
        rune::color::srgb_decode(sa + (sb - sa) * t)
    };
    Color {
        r: mix(a.r, b.r),
        g: mix(a.g, b.g),
        b: mix(a.b, b.b),
        a: a.a + (b.a - a.a) * t,
    }
}

fn banner_gradient_color(start: Color, end: Color, t: f32) -> Color {
    let u = (t / 0.72).clamp(0., 1.);
    lerp_color_srgb(start, end, ease_in_out(u))
}

fn banner_gradient_band_stops(start: Color, end: Color, band: usize) -> [(f32, Color); 8] {
    let t0 = band as f32 / GRADIENT_BANDS as f32;
    let t1 = (band + 1) as f32 / GRADIENT_BANDS as f32;
    std::array::from_fn(|i| {
        let local = i as f32 / 7.;
        let global = t0 + (t1 - t0) * local;
        (local, banner_gradient_color(start, end, global))
    })
}

fn banner_fade_alpha(t: Theme, pos: f32) -> f32 {
    if pos <= 0. {
        return 0.;
    }
    if pos >= 1. {
        return 1.;
    }
    if t.dark {
        // Use Oklab lightness (cube root of linear luminance) calibrated for bright
        // covers so dark-mode gradients roll off gradually instead of forming a steep
        // shelf above the dark background.
        let bg_lin = t.bg.r.clamp(0., 1.);
        let ref_lin = rune::color::srgb_decode(0.80_f32);
        let l_bg = bg_lin.cbrt();
        let l_ref = ref_lin.cbrt();
        let eased = cosine_ease_in_out(pos);
        let l_target = l_ref + (l_bg - l_ref) * eased;
        let target_lin = l_target * l_target * l_target;
        let denom = bg_lin - ref_lin;
        if denom.abs() > 1e-5 {
            ((target_lin - ref_lin) / denom).clamp(0., 1.)
        } else {
            eased
        }
    } else {
        let bg_srgb = rune::color::srgb_encode(t.bg.r);
        let ref_srgb = 0.45_f32;
        let bg_lin = rune::color::srgb_decode(bg_srgb);
        let ref_lin = rune::color::srgb_decode(ref_srgb);
        let denom = bg_lin - ref_lin;
        let eased = ease_in_out(pos);
        let target_srgb = ref_srgb + (bg_srgb - ref_srgb) * eased;
        let target_lin = rune::color::srgb_decode(target_srgb);
        if denom.abs() > 1e-5 {
            ((target_lin - ref_lin) / denom).clamp(0., 1.)
        } else {
            eased
        }
    }
}

fn banner_fade_band_stops(t: Theme, band: usize, bands: usize) -> [(f32, Color); 8] {
    let t0 = band as f32 / bands as f32;
    let t1 = (band + 1) as f32 / bands as f32;
    std::array::from_fn(|i| {
        let local = i as f32 / 7.;
        let global = t0 + (t1 - t0) * local;
        (local, t.bg.alpha(banner_fade_alpha(t, global)))
    })
}

fn banner_gradient_layer(start: Color, end: Color) -> El {
    let band_height = BANNER_HEIGHT / GRADIENT_BANDS as f32;
    let mut layer = col()
        .pin(Some(0.), Some(0.), Some(0.), Some(0.))
        .inert();
    for band in 0..GRADIENT_BANDS {
        layer = layer.child(
            el().w_full()
                .h(band_height)
                .linear_gradient(std::f32::consts::PI, banner_gradient_band_stops(start, end, band)),
        );
    }
    layer
}

fn banner_fade_layer(key: String, t: Theme) -> El {
    let (fade_height, bands) = fade_geometry(t);
    let band_height = fade_height / bands as f32;
    let mut layer = col()
        .key(key)
        .pin(Some(0.), Some(BANNER_HEIGHT - fade_height), Some(0.), Some(0.))
        .inert();
    for band in 0..bands {
        layer = layer.child(
            el().w_full()
                .h(band_height)
                .linear_gradient(std::f32::consts::PI, banner_fade_band_stops(t, band, bands)),
        );
    }
    layer
}

pub fn view(s: &State, page: &Page, t: Theme, surface: &str) -> El {
    let editor = s
        .page_cover
        .editor
        .as_ref()
        .filter(|e| e.page_id == page.id);
    let cover = editor
        .filter(|e| e.reposition)
        .and_then(|e| e.draft.as_ref())
        .or(page.cover.as_ref());
    let Some(cover) = cover else {
        return el().h(0.);
    };
    let width_key = format!("{}:{surface}", page.id);
    let layout_key = width_key.clone();
    let mut banner = el()
        .key(format!("cover-banner-{}-{surface}", page.id))
        .w_full()
        .h(BANNER_HEIGHT)
        .clip_rounded(0.)
        .fill(t.sidebar)
        .on_layout_tx(move |s: &mut State, rect, _| {
            if !s.page_cover.widths.contains_key(&layout_key) && s.page_cover.widths.len() >= 32 {
                if let Some(key) = s.page_cover.widths.keys().next().cloned() {
                    s.page_cover.widths.remove(&key);
                }
            }
            s.page_cover.widths.insert(layout_key.clone(), rect[2]);
        });
    let mut crop_overflow = 0.;
    let mut retry_button = None;
    match cover {
        PageCover::Color { color } => {
            if let Some(color) = parse_color(color) {
                banner = banner.fill(color);
            }
        }
        PageCover::Gradient { start, end } => {
            if let (Some(a), Some(b)) = (parse_color(start), parse_color(end)) {
                banner = banner.fill(a).child(banner_gradient_layer(a, b));
            }
        }
        PageCover::Image { asset, position } => {
            if let Some(error) = s.page_cover.image_errors.get(asset) {
                let asset = asset.clone();
                retry_button = Some(
                    button(
                        format!("cover-image-retry-{}-{surface}", page.id),
                        "Retry image",
                        t,
                    )
                    .label(format!("Cover image failed: {error}. Retry."))
                    .on_press(move |s: &mut State| s.retry_cover_image(&asset)),
                );
            }
            if let Some(image) = s.page_cover.images.get(asset) {
                if let Some([w, h]) = image.size() {
                    let width = s
                        .page_cover
                        .widths
                        .get(&width_key)
                        .copied()
                        .unwrap_or(1.)
                        .max(1.);
                    let scale = (width / w as f32).max(BANNER_HEIGHT / h as f32);
                    let image_width = w as f32 * scale;
                    let image_height = h as f32 * scale;
                    crop_overflow = (image_height - BANNER_HEIGHT).max(0.);
                    banner = banner.child(
                        el().absolute(
                            (width - image_width) / 2.,
                            -(image_height - BANNER_HEIGHT) * position.clamp(0., 1.),
                        )
                        .size(image_width, image_height)
                        .pixels(image.pixels()),
                    );
                }
            }
        }
    }
    banner = banner.child(banner_fade_layer(
        format!("cover-banner-fade-{}-{surface}", page.id),
        t,
    ));
    if let Some(retry) = retry_button {
        banner = banner.child(retry);
    }
    if editor.is_some_and(|e| e.reposition) {
        banner = banner.on_drag(move |s: &mut State, event, tx| {
            if let Some(editor) = &mut s.page_cover.editor {
                if let Some(PageCover::Image { position, .. }) = &mut editor.draft {
                    if event.phase == Phase::Down {
                        event.data = *position;
                    } else {
                        *position =
                            reposition(event.data, event.dy, crop_overflow, event.cancelled());
                    }
                }
            }
            tx.instant();
        });
    }
    banner
}

pub(super) fn header_action_visibility(s: &State, page_id: &str, title_focused: bool) -> f32 {
    if title_focused
        || s.sidebar_keyboard_controls
        || s.page_cover
            .editor
            .as_ref()
            .is_some_and(|editor| editor.page_id == page_id)
        || s.page_icon_picker
            .as_ref()
            .is_some_and(|picker| picker.page_id == page_id)
    {
        1.
    } else {
        0.
    }
}

pub fn controls(s: &State, page: &Page, t: Theme, surface: &str, title_focused: bool) -> El {
    let page_copy = page.clone();
    let key = format!("page-cover-open-{}-{surface}", page.id);
    let anchor = key.clone();
    header_action(
        key,
        if page.cover.is_some() {
            "Change cover"
        } else {
            "Add cover"
        },
        t,
    )
    .group_opacity(header_action_visibility(s, &page.id, title_focused), 1.)
    .on_press(move |s: &mut State| s.open_page_cover(&page_copy, &anchor))
}

pub(super) fn header_action(key: String, title: &str, t: Theme) -> El {
    row()
        .key(key.clone())
        .role("button")
        .label(title)
        .focusable(true)
        .h(30.)
        .px(6.)
        .radius(4.)
        .center_y()
        .hover_group()
        .hover(t.hover)
        .press(t.selected)
        .child(
            t.label(title, t.muted)
                .key(format!("{key}-label"))
                .group_color(t.text),
        )
}

const GRADIENT_PRESETS: [(&str, &str, &str); 8] = [
    ("Dawn", "#acc7df", "#efd3c2"),
    ("Sunset", "#e96443", "#904e95"),
    ("Midnight", "#2b5876", "#4e4376"),
    ("Emerald", "#11998e", "#38ef7d"),
    ("Amber", "#f7971e", "#ffd200"),
    ("Rose", "#ee9ca7", "#ffdde1"),
    ("Indigo", "#4568dc", "#b06ab3"),
    ("Mist", "#d3cce3", "#e9e4f0"),
];

const START_SWATCHES: [&str; 8] = [
    "#acc7df", "#3b82f6", "#8b5cf6", "#ec4899", "#ef4444", "#f59e0b", "#10b981", "#374151",
];

const END_SWATCHES: [&str; 8] = [
    "#efd3c2", "#60a5fa", "#c084fc", "#f472b6", "#fb923c", "#fde047", "#34d399", "#1f2937",
];

fn option(key: &str, label: &str, icon: &'static Icon, t: Theme) -> El {
    menu::row_item(key, label, rune::icon(icon, 16.).color(t.muted), t)
}

fn gradient_swatch_row(
    prefix: &str,
    swatches: &[&str],
    active: &str,
    is_start: bool,
    t: Theme,
) -> El {
    let mut strip = row().w_full().gap(4.);
    for hex in swatches {
        let selected = active.eq_ignore_ascii_case(hex);
        let color_val = parse_color(hex).unwrap_or(t.selected);
        let hex_owned = (*hex).to_owned();
        strip = strip.child(
            el().key(format!("{prefix}-{hex}"))
                .grow(1.)
                .h(22.)
                .radius(4.)
                .fill(color_val)
                .border(if selected { 2. } else { 1. }, if selected { t.blue } else { t.line })
                .role("button")
                .label(format!(
                    "{} color {hex}",
                    if is_start { "Start" } else { "End" }
                ))
                .focusable(true)
                .on_press_tx(move |s: &mut State, tx| {
                    s.set_gradient_stop(is_start, &hex_owned);
                    tx.instant();
                }),
        );
    }
    strip
}

pub fn popup(s: &State, t: Theme) -> Option<El> {
    let editor = s.page_cover.editor.as_ref()?;
    let mut choices = col().gap(6.);
    if editor.gradient_picker {
        let (cur_start, cur_end) = match &editor.draft {
            Some(PageCover::Gradient { start, end }) => (start.as_str(), end.as_str()),
            _ => crate::page_covers::DEFAULT_GRADIENT,
        };
        let preview_start = parse_color(cur_start).unwrap_or(t.selected);
        let preview_end = parse_color(cur_end).unwrap_or(t.selected);
        let mut presets = col().w_full().gap(4.);
        for (chunk_idx, chunk) in GRADIENT_PRESETS.chunks(4).enumerate() {
            let mut line = row().w_full().gap(4.);
            for (offset, (name, start, end)) in chunk.iter().enumerate() {
                let idx = chunk_idx * 4 + offset;
                let active =
                    cur_start.eq_ignore_ascii_case(start) && cur_end.eq_ignore_ascii_case(end);
                let start_s = (*start).to_owned();
                let end_s = (*end).to_owned();
                line = line.child(
                    el().key(format!("cover-gradient-preset-{idx}"))
                        .grow(1.)
                        .h(26.)
                        .radius(5.)
                        .gradient(parse_color(start).unwrap(), parse_color(end).unwrap())
                        .border(if active { 2. } else { 1. }, if active { t.blue } else { t.line })
                        .role("button")
                        .label(format!("{name} gradient"))
                        .help(*name)
                        .focusable(true)
                        .on_press_tx(move |s: &mut State, tx| {
                            s.set_gradient_colors(&start_s, &end_s);
                            tx.instant();
                        }),
                );
            }
            presets = presets.child(line);
        }
        choices = choices
            .child(
                option("cover-gradient-back", "Gradient", &phosphor::CARET_LEFT, t).on_press_tx(
                    |s: &mut State, tx| {
                        if let Some(e) = &mut s.page_cover.editor {
                            e.gradient_picker = false;
                            e.gradient_error = None;
                        }
                        tx.instant();
                    },
                ),
            )
            .child(el().w_full().h(1.).fill(t.line))
            .child(
                el().key("cover-gradient-preview")
                    .w_full()
                    .h(40.)
                    .radius(6.)
                    .gradient(preview_start, preview_end)
                    .border(1., t.line),
            )
            .child(t.sized_label("Presets", 12., t.muted))
            .child(presets)
            .child(
                row()
                    .w_full()
                    .center_y()
                    .child(t.sized_label("Start color", 12., t.muted).grow(1.))
                    .child(
                        row()
                            .key("cover-gradient-swap")
                            .h(22.)
                            .px(6.)
                            .gap(4.)
                            .center_y()
                            .radius(4.)
                            .hover(t.hover)
                            .role("button")
                            .label("Swap colors")
                            .focusable(true)
                            .child(rune::icon(&phosphor::ARROWS_LEFT_RIGHT, 12.).color(t.muted))
                            .child(t.sized_label("Swap", 12., t.muted))
                            .on_press_tx(|s: &mut State, tx| {
                                s.swap_gradient_colors();
                                tx.instant();
                            }),
                    ),
            )
            .child(gradient_swatch_row(
                "cover-gradient-start",
                &START_SWATCHES,
                cur_start,
                true,
                t,
            ))
            .child(
                super::inputs::field(
                    &editor.gradient_start,
                    "cover-gradient-start-input",
                    "#acc7df",
                    t,
                )
                .label("Start hex color")
                .on_edit(|s: &mut State| &mut s.page_cover.editor.as_mut().unwrap().gradient_start)
                .on_input_change(|s: &mut State, tx| {
                    s.apply_gradient_inputs(false);
                    tx.instant();
                })
                .on_submit(|s: &mut State, tx| {
                    s.apply_gradient_inputs(true);
                    tx.instant();
                }),
            )
            .child(t.sized_label("End color", 12., t.muted))
            .child(gradient_swatch_row(
                "cover-gradient-end",
                &END_SWATCHES,
                cur_end,
                false,
                t,
            ))
            .child(
                super::inputs::field(
                    &editor.gradient_end,
                    "cover-gradient-end-input",
                    "#efd3c2",
                    t,
                )
                .label("End hex color")
                .on_edit(|s: &mut State| &mut s.page_cover.editor.as_mut().unwrap().gradient_end)
                .on_input_change(|s: &mut State, tx| {
                    s.apply_gradient_inputs(false);
                    tx.instant();
                })
                .on_submit(|s: &mut State, tx| {
                    s.apply_gradient_inputs(true);
                    tx.instant();
                }),
            )
            .children(
                editor
                    .gradient_error
                    .as_ref()
                    .map(|error| t.label(error, t.error).wrap()),
            );
    } else {
        let mut colors = row().gap(4.);
        for color in ["#b8c5d6", "#d7bfae", "#b9ccb8", "#c9bed4"] {
            colors = colors.child(
                button(format!("cover-color-{color}"), " ", t)
                    .fill(parse_color(color).unwrap())
                    .label(format!("Cover color {color}"))
                    .on_press(move |s: &mut State| {
                        s.set_cover_draft(Some(PageCover::Color {
                            color: color.into(),
                        }))
                    }),
            );
        }
        let target_page = editor.page_id.clone();
        let target_generation = editor.generation;
        choices = choices
            .child(colors)
            .child(
                option("cover-gradient", "Gradient", &phosphor::CIRCLE_HALF, t)
                    .child(menu::end(
                        "cover-gradient-chevron",
                        Some(&phosphor::CARET_RIGHT),
                        t,
                    ))
                    .on_press_tx(|s: &mut State, tx| {
                        s.open_gradient_picker();
                        tx.instant();
                    }),
            )
            .child(
                option("cover-upload", "Upload image", &phosphor::IMAGES_SQUARE, t).on_press_tx(
                    move |_: &mut State, tx| {
                        let target_page = target_page.clone();
                        tx.open_file(
                            FileDialogOptions {
                                title: "Choose a cover image".into(),
                                ..Default::default()
                            }
                            .filter("Images", &["png", "jpg", "jpeg", "webp"]),
                            move |s: &mut State, result, _| {
                                if !s.page_cover.editor.as_ref().is_some_and(|e| {
                                    e.page_id == target_page && e.generation == target_generation
                                }) {
                                    return;
                                }
                                match result {
                                    Ok(Some(path)) => {
                                        s.upload_page_cover(&target_page, target_generation, path)
                                    }
                                    Ok(None) => {}
                                    Err(error) => {
                                        if let Some(editor) = &mut s.page_cover.editor {
                                            editor.error = Some(error.to_string());
                                        }
                                    }
                                }
                            },
                        );
                    },
                ),
            );
        if editor.reposition {
            choices = choices
                .child(
                    option("cover-position-save", "Save position", &phosphor::CHECK, t)
                        .on_press(|s: &mut State| s.save_cover_draft()),
                )
                .child(
                    option("cover-position-cancel", "Cancel", &phosphor::X, t).on_press(
                        |s: &mut State| {
                            if let Some(e) = &mut s.page_cover.editor {
                                e.draft = e.expected.clone();
                                e.reposition = false;
                            }
                        },
                    ),
                );
        } else {
            if matches!(editor.expected, Some(PageCover::Image { .. })) {
                choices = choices.child(
                    option(
                        "cover-reposition",
                        "Reposition",
                        &phosphor::ARROWS_OUT_LINE_VERTICAL,
                        t,
                    )
                    .on_press(|s: &mut State| {
                        if let Some(e) = &mut s.page_cover.editor {
                            e.reposition = true;
                        }
                    }),
                );
            }
            choices = choices.child(
                option("cover-remove", "Remove cover", &phosphor::TRASH, t)
                    .on_press(|s: &mut State| s.set_cover_draft(None)),
            );
        }
    }
    if s.page_cover.loading {
        choices = choices.child(t.label("Importing image…", t.muted));
    }
    if let Some(error) = &editor.error {
        choices = choices
            .child(t.label(error, t.error).wrap())
            .child(
                option("cover-retry", "Retry", &phosphor::ARROW_CLOCKWISE, t)
                    .on_press(|s: &mut State| s.save_cover_draft()),
            )
            .child(
                option("cover-discard", "Discard changes", &phosphor::X, t)
                    .on_press(|s: &mut State| s.discard_cover_draft()),
            );
    }
    choices = choices.child(option("cover-close", "Close", &phosphor::X, t).on_press(
        |s: &mut State| {
            if s.page_cover
                .editor
                .as_ref()
                .is_some_and(|e| e.error.is_none() && !e.reposition)
            {
                s.page_cover.editor = None;
            }
        },
    ));
    let panel = choices
        .key("cover-popup")
        .w(240.)
        .padding(8.)
        .fill(t.menu)
        .radius(10.)
        .border(1., t.line)
        .shadow(6., 20., Color::BLACK.alpha(0.2))
        .role("dialog")
        .label("Cover options")
        .focus_group()
        .focus_return(&editor.anchor);
    let panel = if editor.reposition {
        panel
    } else {
        panel.modal(true, |s: &mut State| {
            if s.page_cover
                .editor
                .as_ref()
                .is_some_and(|e| e.error.is_none())
            {
                s.page_cover.editor = None;
            }
        })
    };
    Some(panel.anchored(&editor.anchor, Side::Bottom, Align::End, 6.))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_uses_crop_overflow_and_cancel_restores_start() {
        assert!((reposition(0.5, 30., 300., false) - 0.4).abs() < 0.0001);
        assert_eq!(reposition(0.5, 30., 300., true), 0.5);
        assert_eq!(reposition(0.5, 30., 0., false), 0.5);
    }

    #[test]
    fn banner_fade_stops_ease_in_and_out_monotonically_in_both_themes() {
        for dark in [false, true] {
            let t = Theme::new(dark);
            let (_, bands) = fade_geometry(t);
            let mut prev_alpha = -1.0_f32;
            for band in 0..bands {
                let stops = banner_fade_band_stops(t, band, bands);
                assert_eq!(stops[0].0, 0.);
                assert_eq!(stops[7].0, 1.);
                if band == 0 {
                    assert_eq!(stops[0].1.a, 0.);
                } else {
                    assert!((stops[0].1.a - prev_alpha).abs() < 1e-6);
                }
                for pair in stops.windows(2) {
                    assert!(pair[0].0 < pair[1].0);
                    assert!(pair[0].1.a < pair[1].1.a);
                    assert!((pair[1].1.r - t.bg.r).abs() < 1e-6);
                    assert!((pair[1].1.g - t.bg.g).abs() < 1e-6);
                    assert!((pair[1].1.b - t.bg.b).abs() < 1e-6);
                }
                prev_alpha = stops[7].1.a;
            }
            assert_eq!(prev_alpha, 1.);
            assert!(banner_fade_alpha(t, 0.10) < 0.08);
            assert!(banner_fade_alpha(t, 0.90) > 0.96);
        }
    }

    #[test]
    fn banner_gradient_stops_ease_from_start_to_end_before_bottom() {
        let a = parse_color("#4568dc").unwrap();
        let b = parse_color("#b06ab3").unwrap();
        let first = banner_gradient_band_stops(a, b, 0);
        let last = banner_gradient_band_stops(a, b, GRADIENT_BANDS - 1);
        assert!((first[0].1.r - a.r).abs() < 1e-5);
        assert!((last[0].1.r - b.r).abs() < 1e-5);
        assert!((last[7].1.r - b.r).abs() < 1e-5);
    }

    #[test]
    fn rendered_banner_fades_gradually_into_page_background_without_edges() {
        use rune::render::{Gpu, OFFSCREEN_FORMAT, Offscreen, Renderer};
        let gpu = Gpu::headless();
        let preview_dir = std::env::var("MOSS_BANNER_PREVIEW_DIR").ok();
        let size = (1280u32, 820u32);
        for (mode, dark) in [("light", false), ("dark", true)] {
            for (kind, cover) in [
                (
                    "color",
                    PageCover::Color {
                        color: "#5b6ea8".into(),
                    },
                ),
                (
                    "gradient",
                    PageCover::Gradient {
                        start: "#6b66b8".into(),
                        end: "#eab59b".into(),
                    },
                ),
                (
                    "image",
                    PageCover::Image {
                        asset: "0".repeat(64),
                        position: 0.5,
                    },
                ),
            ] {
                let path =
                    std::env::temp_dir().join(format!("moss-banner-render-{}", moss_core::new_id()));
                let mut state = State::open(path.clone(), None).unwrap();
                state.prefs.dark = Some(dark);
                state.new_page(false, None);
                state.tabs[state.active].title.replace_text("Moss Page");
                state.tabs[state.active].blocks[0]
                    .edit
                    .replace_text("The banner fades smoothly into the page background.");
                if let PageCover::Image { asset, .. } = &cover {
                    let cache = rune::ImageCache::new(std::sync::Arc::new(|| {}));
                    let (w, h) = (320u32, 160u32);
                    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
                    for y in 0..h {
                        let ty = y as f32 / (h - 1) as f32;
                        for x in 0..w {
                            let check = ((x / 8) + (y / 8)) % 2 == 0;
                            let r = (100. + 135. * ty + if check { 14. } else { -14. }) as u8;
                            let g = (95. + 90. * ty + if check { 10. } else { -10. }) as u8;
                            let b = (180. - 30. * ty + if check { 12. } else { -12. }) as u8;
                            pixels.extend_from_slice(&[r, g, b, 255]);
                        }
                    }
                    let img = cache.rgba(w, h, pixels);
                    img.wait().unwrap();
                    state.page_cover.images.insert(asset.clone(), img);
                }
                state.tabs[state.active].page.cover = Some(cover);
                let page_id = state.tabs[state.active].page.id.clone();
                let mut rt = Runtime::with_text(
                    state,
                    crate::ui::view,
                    (size.0 as f32, size.1 as f32),
                    Env {
                        system_dark: dark,
                        scale: 1.,
                    },
                    rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
                );
                rt.frame(0.);
                rt.frame(1. / 60.);
                let banner_rect = rt
                    .presented(&format!("cover-banner-{page_id}-document"))
                    .expect("banner rect")
                    .0;
                assert!((banner_rect[3] - BANNER_HEIGHT).abs() < 0.5);
                let (expected_fade_height, _) = fade_geometry(Theme::new(dark));
                let fade_rect = rt
                    .presented(&format!("cover-banner-fade-{page_id}-document"))
                    .expect("fade rect")
                    .0;
                assert!((fade_rect[3] - expected_fade_height).abs() < 0.5);
                assert!(
                    (fade_rect[1] + fade_rect[3] - (banner_rect[1] + banner_rect[3])).abs() < 0.5
                );

                let mut renderer = Renderer::new(&gpu.device, OFFSCREEN_FORMAT);
                let target = Offscreen::with_format(&gpu.device, size.0, size.1, OFFSCREEN_FORMAT);
                renderer.upload_images(&gpu.device, &gpu.queue, &mut rt.images);
                renderer.prepare_surfaces(&gpu.device, &gpu.queue, &rt.surfaces, size);
                renderer.prepare(&gpu.device, &gpu.queue, &mut rt.text.atlas, &rt.instances, size);
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                renderer.draw(&mut encoder, &target.view, rt.clear_color());
                target.copy(&mut encoder);
                renderer.submit(&gpu.queue, encoder);
                let rgba = target.read(&gpu.device);

                if let Some(dir) = &preview_dir {
                    let dir = std::path::Path::new(dir);
                    std::fs::create_dir_all(dir).unwrap();
                    let ppm_path = dir.join(format!("banner-{mode}-{kind}.ppm"));
                    let mut ppm = format!("P6\n{} {}\n255\n", size.0, size.1).into_bytes();
                    ppm.reserve((size.0 * size.1 * 3) as usize);
                    for px in rgba.chunks_exact(4) {
                        ppm.extend_from_slice(&px[..3]);
                    }
                    std::fs::write(ppm_path, ppm).unwrap();
                }

                let sample_x = (banner_rect[0] + banner_rect[2] * 0.5) as u32;
                let top_y = banner_rect[1].round() as u32 + 10;
                let fade_start_y = fade_rect[1].round() as u32;
                let bottom_y = (banner_rect[1] + banner_rect[3]).round() as u32 - 1;
                let below_y = bottom_y + 4;
                let px = |y: u32| -> [u8; 4] {
                    let i = ((y * size.0 + sample_x) * 4) as usize;
                    rgba[i..i + 4].try_into().unwrap()
                };
                let bg = px(below_y);
                let bottom = px(bottom_y);
                let top = px(top_y);
                // Bottom edge seamlessly matches page background.
                for c in 0..3 {
                    assert!(
                        bottom[c].abs_diff(bg[c]) <= 1,
                        "{mode}/{kind}: bottom {bottom:?} != bg {bg:?}"
                    );
                }
                // Top is visibly distinct from background.
                let top_diff: i32 = (0..3).map(|c| top[c].abs_diff(bg[c]) as i32).sum();
                assert!(top_diff > 60, "{mode}/{kind}: top {top:?} too close to bg {bg:?}");

                // Measure start edge, end edge, and per-step smoothness across the fade zone
                // on color and gradient covers to confirm zero stepped banding or creases.
                if matches!(kind, "color" | "gradient") {
                    if kind == "color" {
                        let before_start = px(fade_start_y - 2);
                        let after_start = px(fade_start_y + 12);
                        let near_end = px(bottom_y - 12);
                        let start_step: u8 = (0..3)
                            .map(|c| before_start[c].abs_diff(after_start[c]))
                            .max()
                            .unwrap();
                        let end_step: u8 = (0..3)
                            .map(|c| near_end[c].abs_diff(bottom[c]))
                            .max()
                            .unwrap();
                        assert!(
                            start_step <= 4,
                            "{mode}: start of fade has sharp edge: {before_start:?} -> {after_start:?} (delta {start_step})"
                        );
                        assert!(
                            end_step <= 4,
                            "{mode}: end of fade has sharp edge: {near_end:?} -> {bottom:?} (delta {end_step})"
                        );
                    }
                    let mut prev_delta = [0i32; 3];
                    let mut have_prev = false;
                    let mut y = fade_start_y;
                    while y + 4 <= bottom_y {
                        let a = px(y);
                        let b = px(y + 4);
                        let delta = [
                            b[0] as i32 - a[0] as i32,
                            b[1] as i32 - a[1] as i32,
                            b[2] as i32 - a[2] as i32,
                        ];
                        if have_prev {
                            for c in 0..3 {
                                let kink = (delta[c] - prev_delta[c]).abs();
                                assert!(
                                    kink <= 2,
                                    "{mode}/{kind} banding kink at y={y} ch={c}: prev_delta={} delta={}",
                                    prev_delta[c],
                                    delta[c]
                                );
                            }
                        }
                        prev_delta = delta;
                        have_prev = true;
                        y += 4;
                    }
                }

                drop(rt);
                let _ = std::fs::remove_dir_all(path);
            }
        }
    }

    #[test]
    fn fade_overlay_does_not_block_image_reposition_drag() {
        let path = std::env::temp_dir().join(format!("moss-banner-drag-{}", moss_core::new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let page_id = state.tabs[state.active].page.id.clone();
        // Create a tall 10x600 PNG so crop_overflow > 0 on a 1000pt window.
        let png = {
            // Minimal valid 2x400 solid PNG built using a small raw encoder is unnecessary;
            // verify reposition with a valid imported image and direct drag over the fade zone.
            [
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0,
                1, 8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99,
                248, 207, 192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68,
                174, 66, 96, 130,
            ]
        };
        let asset = state.store.import_cover_image(&png).unwrap();
        let cover = PageCover::Image {
            asset: asset.clone(),
            position: 0.5,
        };
        state.tabs[state.active].page.cover = Some(cover.clone());
        let page = state.tabs[state.active].page.clone();
        state.open_page_cover(&page, &format!("page-cover-open-{page_id}-document"));
        state.page_cover.editor.as_mut().unwrap().reposition = true;
        let cache = rune::ImageCache::new(std::sync::Arc::new(|| {}));
        let image = cache.rgba(400, 300, vec![120; 400 * 300 * 4]);
        image.wait().unwrap();
        state.page_cover.images.insert(asset, image);
        let mut rt = Runtime::new(state, crate::ui::view, (1000., 800.), Env::default());
        rt.frame(0.);
        rt.frame(1. / 60.);
        let fade = rt
            .presented(&format!("cover-banner-fade-{page_id}-document"))
            .unwrap()
            .0;
        let x = fade[0] + fade[2] * 0.5;
        let y = fade[1] + fade[3] * 0.5;
        rt.pointer_down(x, y);
        rt.pointer_move(x, y - 40.);
        rt.pointer_up(x, y - 40.);
        rt.frame(1. / 60.);
        match &rt.state.page_cover.editor.as_ref().unwrap().draft {
            Some(PageCover::Image { position, .. }) => {
                assert!(
                    (*position - 0.5).abs() > 0.05,
                    "fade overlay intercepted reposition drag: position={position}"
                );
            }
            other => panic!("unexpected draft: {other:?}"),
        }
        drop(rt);
        let _ = std::fs::remove_dir_all(path);
    }
}
