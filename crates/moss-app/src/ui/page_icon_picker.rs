use super::theme::{Theme, button};
use crate::{emoji_catalog, state::State};
use rune::*;

const CATEGORY_ICONS: [&Icon; 10] = [
    &super::phosphor::SQUARES_FOUR,
    &super::phosphor::SMILEY,
    &super::phosphor::USER,
    &super::phosphor::LEAF,
    &super::phosphor::ORANGE_SLICE,
    &super::phosphor::AIRPLANE_TILT,
    &super::phosphor::BASKETBALL,
    &super::phosphor::LIGHTBULB,
    &super::phosphor::HEART,
    &super::phosphor::FLAG,
];

pub fn popup(s: &State, t: Theme) -> Option<El> {
    let picker = s.page_icon_picker.as_ref()?;
    let results = emoji_catalog::matches(&s.page_icon_search.text, picker.category);
    let page_count = results.len().div_ceil(emoji_catalog::PAGE_SIZE).max(1);
    let page_start = picker.page * emoji_catalog::PAGE_SIZE;
    let input = super::inputs::field(&s.page_icon_search, "page-icon-search", "Search emoji…", t)
        .label("Search emoji")
        .on_edit(|s: &mut State| &mut s.page_icon_search)
        .on_input_change(|s: &mut State, tx| {
            if let Some(p) = &mut s.page_icon_picker {
                p.selected = 0;
                p.page = 0;
            }
            tx.instant();
        })
        .on_input_key(|s: &mut State, event, tx| {
            if event.key == Key::Escape {
                s.dismiss_page_icon_picker();
                tx.instant();
                return true;
            }
            if matches!(event.key, Key::Up | Key::Down | Key::Left | Key::Right) {
                let len = s
                    .page_icon_picker
                    .as_ref()
                    .map(|p| {
                        emoji_catalog::matches(&s.page_icon_search.text, p.category)
                            .len()
                            .saturating_sub(p.page * emoji_catalog::PAGE_SIZE)
                            .min(emoji_catalog::PAGE_SIZE)
                    })
                    .unwrap_or(0);
                if let Some(p) = &mut s.page_icon_picker {
                    p.selected = match event.key {
                        Key::Up => p.selected.saturating_sub(8),
                        Key::Down => p.selected.saturating_add(8).min(len.saturating_sub(1)),
                        Key::Left => p.selected.saturating_sub(1),
                        _ => p.selected.saturating_add(1).min(len.saturating_sub(1)),
                    };
                    tx.scroll_into_view(format!("page-icon-option-{}", p.selected));
                }
                tx.instant();
                return true;
            }
            false
        })
        .on_submit(|s: &mut State, tx| {
            s.submit_page_icon_search();
            tx.instant();
        });
    let mut content = col().w_full().gap(4.);
    if s.page_icon_search.text.trim().is_empty() && !s.recent_page_icons.is_empty() {
        content = content.child(t.label("Recent", t.muted).px(3.).pt(4.));
        let mut recent = row().w_full().flex_wrap();
        for glyph in &s.recent_page_icons {
            let value = glyph.clone();
            recent = recent.child(
                row()
                    .size(35., 35.)
                    .center()
                    .radius(5.)
                    .hover(t.hover)
                    .role("button")
                    .label(format!("Recent emoji {glyph}"))
                    .focusable(true)
                    .child(t.sized_label(glyph, 20., t.text))
                    .on_press(move |s: &mut State| s.choose_page_icon(&value)),
            );
        }
        content = content.child(recent);
    }
    content = content.child(
        t.label(picker.category.unwrap_or("Emoji"), t.muted)
            .px(3.)
            .pt(5.),
    );
    let mut grid = row().w_full().flex_wrap();
    for (index, emoji) in results
        .iter()
        .skip(page_start)
        .take(emoji_catalog::PAGE_SIZE)
        .enumerate()
    {
        let glyph = emoji.glyph;
        grid = grid.child(
            row()
                .key(format!("page-icon-option-{index}"))
                .size(35., 35.)
                .center()
                .radius(5.)
                .fill(if picker.selected == index {
                    t.selected
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .role("button")
                .label(emoji.name)
                .focusable(true)
                .child(t.sized_label(glyph, 20., t.text))
                .on_press(move |s: &mut State| s.choose_page_icon(glyph)),
        );
    }
    if !results.is_empty() {
        content = content.child(grid);
    }
    if results.is_empty() {
        content = content.child(t.label("No emoji found.", t.muted).padding(8.));
    }
    let mut panel = col()
        .key("page-icon-picker")
        .w(300.)
        .max_h(420.)
        .padding(8.)
        .gap(6.)
        .fill(t.menu)
        .border(1., t.line)
        .radius(8.)
        .shadow(4., 16., Color::BLACK.alpha(0.15))
        .role("dialog")
        .label("Choose page icon")
        .focus_group()
        .focus_return(picker.anchor.clone())
        .modal(true, |s: &mut State| s.dismiss_page_icon_picker())
        .transition(Transition::opacity())
        .child(
            row()
                .w_full()
                .center_y()
                .child(t.label("Page icon", t.text).grow(1.))
                .child(
                    button("page-icon-random", "Random", t)
                        .on_press(|s: &mut State| s.choose_random_page_icon()),
                )
                .child(
                    button("page-icon-remove", "Remove", t)
                        .on_press(|s: &mut State| s.choose_page_icon("")),
                ),
        )
        .child(input);
    let mut categories = row().w_full();
    for (index, category) in std::iter::once(None)
        .chain(emoji_catalog::CATEGORIES.iter().copied().map(Some))
        .enumerate()
    {
        let label = category.unwrap_or("All");
        categories = categories.child(
            row()
                .key(format!("page-icon-category-{label}"))
                .size(28., 28.)
                .center()
                .radius(4.)
                .hover(t.hover)
                .role("button")
                .label(label)
                .focusable(true)
                .fill(if picker.category == category {
                    t.selected
                } else {
                    Color::TRANSPARENT
                })
                .child(
                    row()
                        .size(20., 20.)
                        .center()
                        .child(rune::icon(CATEGORY_ICONS[index], 16.).color(t.muted)),
                )
                .on_press(move |s: &mut State| {
                    if let Some(p) = &mut s.page_icon_picker {
                        p.category = category;
                        p.page = 0;
                        p.selected = 0;
                    }
                }),
        );
    }
    panel = panel.child(categories);
    if let Some(error) = &picker.error {
        panel = panel.child(t.label(error, t.error).wrap()).child(
            row()
                .gap(4.)
                .child(
                    button("page-icon-reload", "Reload icon", t)
                        .on_press(|s: &mut State| s.reload_page_icon_picker()),
                )
                .child(
                    button("page-icon-discard", "Discard", t)
                        .on_press(|s: &mut State| s.page_icon_picker = None),
                ),
        );
    }
    if page_count > 1 {
        panel = panel.child(
            row()
                .w_full()
                .center_y()
                .children((picker.page > 0).then(|| {
                    button("page-icon-previous", "Previous", t).on_press(|s: &mut State| {
                        if let Some(p) = &mut s.page_icon_picker {
                            p.page = p.page.saturating_sub(1);
                            p.selected = 0;
                        }
                    })
                }))
                .child(
                    t.label(format!("{} / {}", picker.page + 1, page_count), t.muted)
                        .grow(1.)
                        .center(),
                )
                .children((picker.page + 1 < page_count).then(|| {
                    button("page-icon-next", "Next", t).on_press(|s: &mut State| {
                        if let Some(p) = &mut s.page_icon_picker {
                            p.page += 1;
                            p.selected = 0;
                        }
                    })
                })),
        );
    }
    Some(
        panel
            .child(
                content
                    .max_h(300.)
                    .min_h(0.)
                    .shrink()
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .instant_changes(),
            )
            .anchored_fit(picker.anchor.clone(), Side::Bottom, Align::Start, 4.),
    )
}
