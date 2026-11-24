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
fn banner_height(surface: &str) -> f32 {
    if surface == "database" { 240. } else { 360. }
}

fn fade_geometry(height: f32, dark: bool) -> (f32, usize) {
    let fade = if dark { height * 0.8 } else { height * (2. / 3.) };
    let step = if fade % 12. == 0. { 12. } else { 10. };
    (fade, (fade / step) as usize)
}

fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    0.5 * (t * t * (3. - 2. * t) + t * t * t * (t * (t * 6. - 15.) + 10.))
}

fn lerp_color_srgb(a: Color, b: Color, t: f32) -> Color {
    let mix = |ca, cb| {
        let (sa, sb) = (rune::color::srgb_encode(ca), rune::color::srgb_encode(cb));
        rune::color::srgb_decode(sa + (sb - sa) * t)
    };
    Color {
        r: mix(a.r, b.r),
        g: mix(a.g, b.g),
        b: mix(a.b, b.b),
        a: a.a + (b.a - a.a) * t,
    }
}

fn blend_linear(fg: Color, bg: Color, alpha: f32) -> Color {
    let inv = 1. - alpha;
    Color {
        r: fg.r * inv + bg.r * alpha,
        g: fg.g * inv + bg.g * alpha,
        b: fg.b * inv + bg.b * alpha,
        a: 1.,
    }
}

fn banner_fade_alpha(t: Theme, pos: f32) -> f32 {
    if pos <= 0. {
        return 0.;
    }
    if pos >= 1. {
        return 1.;
    }
    let bg_lin = t.bg.r.clamp(0., 1.);
    let (ref_lin, target_lin, eased) = if t.dark {
        // Oklab lightness (cbrt) calibrated for bright covers prevents a steep dark-mode shelf.
        let ref_lin = rune::color::srgb_decode(0.80);
        let eased = 0.5 * (1. - (std::f32::consts::PI * pos).cos());
        let l = ref_lin.cbrt() + (bg_lin.cbrt() - ref_lin.cbrt()) * eased;
        (ref_lin, l * l * l, eased)
    } else {
        let (bg_srgb, ref_srgb) = (rune::color::srgb_encode(bg_lin), 0.45);
        let eased = ease_in_out(pos);
        (
            rune::color::srgb_decode(ref_srgb),
            rune::color::srgb_decode(ref_srgb + (bg_srgb - ref_srgb) * eased),
            eased,
        )
    };
    let denom = bg_lin - ref_lin;
    if denom.abs() > 1e-5 {
        ((target_lin - ref_lin) / denom).clamp(0., 1.)
    } else {
        eased
    }
}

fn vertical_bands(height: f32, bands: usize, sample: impl Fn(f32) -> Color) -> El {
    let band_height = height / bands as f32;
    let mut layer = col().inert();
    let mut prev = sample(0.);
    for band in 0..bands {
        let next = sample((band + 1) as f32 / bands as f32);
        layer = layer.child(el().w_full().h(band_height).gradient(prev, next));
        prev = next;
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
    let height = banner_height(surface);
    let (fade_height, fade_bands) = fade_geometry(height, t.dark);
    let fade_top = height - fade_height;
    let fade_key = format!("cover-banner-fade-{}-{surface}", page.id);
    let width_key = format!("{}:{surface}", page.id);
    let layout_key = width_key.clone();
    let mut banner = el()
        .key(format!("cover-banner-{}-{surface}", page.id))
        .w_full()
        .h(height)
        .clip_rounded(0.)
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
    let mut needs_alpha_fade = false;
    match cover {
        PageCover::Color { color } => {
            if let Some(color) = parse_color(color) {
                banner = banner.child(
                    rune::memo(
                        format!("cover-color-memo-{}-{surface}", page.id),
                        (color.r.to_bits(), color.g.to_bits(), color.b.to_bits(), height.to_bits(), t.dark, t.bg.r.to_bits()),
                        || {
                            col()
                                .w_full()
                                .h(height)
                                .child(el().w_full().h(fade_top).fill(color))
                                .child(
                                    vertical_bands(fade_height, fade_bands, |p| {
                                        blend_linear(color, t.bg, banner_fade_alpha(t, p))
                                    })
                                    .key(fade_key.clone()),
                                )
                        },
                    )
                    .pin(Some(0.), Some(0.), Some(0.), Some(0.))
                    .inert(),
                );
            } else {
                banner = banner.fill(t.sidebar);
                needs_alpha_fade = true;
            }
        }
        PageCover::Gradient { start, end } => {
            if let (Some(a), Some(b)) = (parse_color(start), parse_color(end)) {
                banner = banner.child(
                    rune::memo(
                        format!("cover-grad-memo-{}-{surface}", page.id),
                        (
                            a.r.to_bits(), a.g.to_bits(), a.b.to_bits(),
                            b.r.to_bits(), b.g.to_bits(), b.b.to_bits(),
                            height.to_bits(), t.dark, t.bg.r.to_bits(),
                        ),
                        || {
                            let grad_at = |y: f32| {
                                lerp_color_srgb(a, b, ease_in_out(((y / height) / 0.72).clamp(0., 1.)))
                            };
                            let top_bands = ((fade_top / 12.).round() as usize).max(1);
                            col()
                                .w_full()
                                .h(height)
                                .child(vertical_bands(fade_top, top_bands, |p| grad_at(p * fade_top)))
                                .child(
                                    vertical_bands(fade_height, fade_bands, |p| {
                                        blend_linear(grad_at(fade_top + p * fade_height), t.bg, banner_fade_alpha(t, p))
                                    })
                                    .key(fade_key.clone()),
                                )
                        },
                    )
                    .pin(Some(0.), Some(0.), Some(0.), Some(0.))
                    .inert(),
                );
            } else {
                banner = banner.fill(t.sidebar);
                needs_alpha_fade = true;
            }
        }
        PageCover::Image { asset, position } => {
            banner = banner.fill(t.sidebar);
            needs_alpha_fade = true;
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
                    let scale = (width / w as f32).max(height / h as f32);
                    let image_width = w as f32 * scale;
                    let image_height = h as f32 * scale;
                    crop_overflow = (image_height - height).max(0.);
                    banner = banner.child(
                        el().absolute(
                            (width - image_width) / 2.,
                            -(image_height - height) * position.clamp(0., 1.),
                        )
                        .size(image_width, image_height)
                        .pixels(image.pixels()),
                    );
                }
            }
        }
    }
    if needs_alpha_fade {
        banner = banner.child(
            rune::memo(
                fade_key.clone(),
                (height.to_bits(), t.dark, t.bg.r.to_bits()),
                || vertical_bands(fade_height, fade_bands, |p| t.bg.alpha(banner_fade_alpha(t, p))),
            )
            .pin(Some(0.), Some(fade_top), Some(0.), Some(0.))
            .inert(),
        );
    }
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
    fn banner_fade_eases_monotonically_in_both_themes() {
        for dark in [false, true] {
            let t = Theme::new(dark);
            assert_eq!(banner_fade_alpha(t, 0.), 0.);
            assert_eq!(banner_fade_alpha(t, 1.), 1.);
            let samples: Vec<f32> = (0..=20).map(|i| banner_fade_alpha(t, i as f32 / 20.)).collect();
            assert!(samples.windows(2).all(|w| w[0] < w[1]));
            assert!(banner_fade_alpha(t, 0.10) < 0.08);
            assert!(banner_fade_alpha(t, 0.90) > 0.96);
        }
    }

    #[test]
    fn rendered_banner_fades_smoothly_without_edges_or_banding() {
        use rune::render::{Gpu, OFFSCREEN_FORMAT, Offscreen, Renderer};
        let gpu = Gpu::headless();
        let size = (1000u32, 760u32);
        for dark in [false, true] {
            let covers = [
                ("color", PageCover::Color { color: "#5b6ea8".into() }),
                ("gradient", PageCover::Gradient { start: "#6b66b8".into(), end: "#eab59b".into() }),
                ("image", PageCover::Image { asset: "0".repeat(64), position: 0.5 }),
            ];
            for (kind, cover) in covers {
                let path = std::env::temp_dir().join(format!("moss-banner-{}", moss_core::new_id()));
                let mut state = State::open(path.clone(), None).unwrap();
                state.prefs.dark = Some(dark);
                state.new_page(false, None);
                if let PageCover::Image { asset, .. } = &cover {
                    let img = rune::ImageCache::new(std::sync::Arc::new(|| {}))
                        .rgba(16, 16, [110, 100, 180, 255].repeat(256));
                    img.wait().unwrap();
                    state.page_cover.images.insert(asset.clone(), img);
                }
                state.tabs[state.active].page.cover = Some(cover);
                let page_id = state.tabs[state.active].page.id.clone();
                let mut rt = Runtime::new(state, crate::ui::view, (size.0 as f32, size.1 as f32), Env::new(dark, 1.));
                rt.frame(0.);
                rt.frame(1. / 60.);
                let banner = rt.presented(&format!("cover-banner-{page_id}-document")).unwrap().0;
                let fade = rt.presented(&format!("cover-banner-fade-{page_id}-document")).unwrap().0;
                assert!((banner[3] - banner_height("document")).abs() < 0.5);
                assert!((fade[1] + fade[3] - (banner[1] + banner[3])).abs() < 0.5);

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

                let x = (banner[0] + banner[2] * 0.5) as u32;
                let px = |y: u32| -> [u8; 4] { rgba[((y * size.0 + x) * 4) as usize..][..4].try_into().unwrap() };
                let (top_y, fade_y, bot_y) = (banner[1].round() as u32 + 10, fade[1].round() as u32, (banner[1] + banner[3]).round() as u32 - 1);
                let (top, bot, bg) = (px(top_y), px(bot_y), px(bot_y + 4));
                assert!((0..3).all(|c| bot[c].abs_diff(bg[c]) <= 1), "dark={dark} {kind}: {bot:?} != {bg:?}");
                assert!((0..3).map(|c| top[c].abs_diff(bg[c]) as i32).sum::<i32>() > 60);

                if kind == "color" {
                    assert!((0..3).all(|c| px(fade_y - 2)[c].abs_diff(px(fade_y + 12)[c]) <= 4));
                    assert!((0..3).all(|c| px(bot_y - 12)[c].abs_diff(bot[c]) <= 4));
                }
                let mut prev = None::<[i32; 3]>;
                for y in (fade_y..=bot_y - 4).step_by(4) {
                    let (a, b) = (px(y), px(y + 4));
                    let d = [b[0] as i32 - a[0] as i32, b[1] as i32 - a[1] as i32, b[2] as i32 - a[2] as i32];
                    if let Some(p) = prev {
                        assert!((0..3).all(|c| (d[c] - p[c]).abs() <= 2), "dark={dark} {kind} kink at y={y}: {p:?} -> {d:?}");
                    }
                    prev = Some(d);
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
        let asset = "0".repeat(64);
        state.tabs[state.active].page.cover = Some(PageCover::Image { asset: asset.clone(), position: 0.5 });
        let page = state.tabs[state.active].page.clone();
        state.open_page_cover(&page, &format!("page-cover-open-{page_id}-document"));
        state.page_cover.editor.as_mut().unwrap().reposition = true;
        let img = rune::ImageCache::new(std::sync::Arc::new(|| {})).rgba(400, 400, vec![120; 400 * 400 * 4]);
        img.wait().unwrap();
        state.page_cover.images.insert(asset, img);
        let mut rt = Runtime::new(state, crate::ui::view, (1000., 800.), Env::default());
        rt.frame(0.);
        rt.frame(1. / 60.);
        let fade = rt.presented(&format!("cover-banner-fade-{page_id}-document")).unwrap().0;
        let (x, y) = (fade[0] + fade[2] * 0.5, fade[1] + fade[3] * 0.5);
        rt.pointer_down(x, y);
        rt.pointer_move(x, y - 40.);
        rt.pointer_up(x, y - 40.);
        rt.frame(1. / 60.);
        assert!(matches!(
            rt.state.page_cover.editor.as_ref().unwrap().draft,
            Some(PageCover::Image { position, .. }) if (position - 0.5).abs() > 0.05
        ));
        drop(rt);
        let _ = std::fs::remove_dir_all(path);
    }
}
