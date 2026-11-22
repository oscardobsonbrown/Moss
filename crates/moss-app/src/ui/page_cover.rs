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
        .h(200.)
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
    match cover {
        PageCover::Color { color } => {
            if let Some(color) = parse_color(color) {
                banner = banner.fill(color);
            }
        }
        PageCover::Gradient { start, end } => {
            if let (Some(a), Some(b)) = (parse_color(start), parse_color(end)) {
                banner = banner.gradient(a, b);
            }
        }
        PageCover::Image { asset, position } => {
            if let Some(error) = s.page_cover.image_errors.get(asset) {
                let asset = asset.clone();
                banner = banner.child(
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
                    let scale = (width / w as f32).max(200. / h as f32);
                    let image_width = w as f32 * scale;
                    let image_height = h as f32 * scale;
                    crop_overflow = (image_height - 200.).max(0.);
                    banner = banner.child(
                        el().absolute(
                            (width - image_width) / 2.,
                            -(image_height - 200.) * position.clamp(0., 1.),
                        )
                        .size(image_width, image_height)
                        .pixels(image.pixels()),
                    );
                }
            }
        }
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
}
