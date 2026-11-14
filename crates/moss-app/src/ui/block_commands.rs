use super::{
    menu, phosphor,
    theme::{Theme, button},
};
use crate::{block_commands::BlockCommandMode, state::State};
use moss_core::BlockKind;
use rune::*;

fn icon(kind: BlockKind) -> &'static Icon {
    match kind {
        BlockKind::Paragraph => &phosphor::TEXT_T,
        BlockKind::Heading1 => &phosphor::TEXT_H_ONE,
        BlockKind::Heading2 => &phosphor::TEXT_H_TWO,
        BlockKind::Heading3 => &phosphor::TEXT_H_THREE,
        BlockKind::Todo => &phosphor::LIST_CHECKS,
        BlockKind::BulletedList => &phosphor::LIST_BULLETS,
        BlockKind::NumberedList => &phosphor::LIST_NUMBERS,
        BlockKind::Toggle => &phosphor::CARET_RIGHT,
        BlockKind::Quote => &phosphor::QUOTES,
        BlockKind::Callout => &phosphor::LIGHTBULB,
        BlockKind::Code => &phosphor::CODE,
        BlockKind::Divider => &phosphor::MINUS,
        BlockKind::ChildPage => &phosphor::FILE,
        BlockKind::SimpleTable => &phosphor::TABLE,
        BlockKind::Image => &phosphor::IMAGES_SQUARE,
        BlockKind::Bookmark => &phosphor::LINK,
        _ => &phosphor::TEXT_T,
    }
}

pub fn popup(s: &State, t: Theme) -> Option<El> {
    let current = s.block_commands.as_ref()?;
    if current.link_picker {
        return Some(link_picker(s, t));
    }
    let results = s.block_command_matches();
    let anchor = current.anchor.clone();
    let change = current.mode == BlockCommandMode::Change && !current.peek;
    let empty = results.is_empty();
    let input = super::inputs::field(
        &s.block_command_search,
        "block-command-search",
        "Search blocks…",
        t,
    )
    .label("Search block commands")
    .on_edit(|s: &mut State| &mut s.block_command_search)
    .on_input_change(|s: &mut State, tx| {
        if let Some(menu) = &mut s.block_commands {
            menu.selected = 0;
        }
        tx.instant();
    })
    .on_input_key(|s: &mut State, event, tx| match event.key {
        Key::Escape => {
            s.dismiss_block_commands();
            tx.instant();
            true
        }
        Key::Up | Key::Down => {
            let count = s.block_command_matches().len();
            if let Some(menu) = &mut s.block_commands {
                menu.selected = if event.key == Key::Up {
                    menu.selected.saturating_sub(1)
                } else {
                    menu.selected.saturating_add(1).min(count.saturating_sub(1))
                };
                tx.scroll_into_view(format!("block-command-option-{}", menu.selected));
            }
            tx.instant();
            true
        }
        _ => false,
    })
    .on_submit(|s: &mut State, tx| {
        s.submit_block_command();
        tx.instant();
    });
    let rows = results
        .into_iter()
        .enumerate()
        .map(|(index, (kind, name))| {
            let selected = current.selected == index;
            menu::row_item(
                format!("block-kind-{name}"),
                name,
                rune::icon(icon(kind), 16.).color(t.muted),
                t,
            )
            .key(format!("block-kind-{name}"))
            .fill(if selected {
                t.selected
            } else {
                Color::TRANSPARENT
            })
            .child(
                row()
                    .key(format!("block-command-option-{index}"))
                    .w(36.)
                    .h(20.)
                    .center_y()
                    .child(t.label(
                        match kind {
                            BlockKind::Heading1 => "#",
                            BlockKind::Heading2 => "##",
                            BlockKind::Heading3 => "###",
                            _ => "",
                        },
                        t.faint,
                    )),
            )
            .on_press(move |s: &mut State| s.apply_block_command(kind))
        });
    let list = col()
        .w_full()
        .min_h(0.)
        .max_h(300.)
        .shrink()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .children(rows)
        .children(empty.then(|| t.label("No blocks found.", t.muted).padding(8.)));
    Some(
        col()
            .key("block-commands")
            .w(280.)
            .max_h(460.)
            .padding(8.)
            .gap(6.)
            .fill(t.menu)
            .border(1., t.line)
            .radius(8.)
            .shadow(4., 16., Color::BLACK.alpha(0.15))
            .role("dialog")
            .label("Block commands")
            .focus_group()
            .focus_return(format!(
                "{}{}",
                if current.peek {
                    "peek-block-"
                } else {
                    "block-input-"
                },
                current.block_id
            ))
            .modal(true, |s: &mut State| s.dismiss_block_commands())
            .transition(Transition::opacity())
            .child(input)
            .child(t.label("Basic blocks", t.muted).px(10.))
            .child(list)
            .children((change && current.error.is_none()).then(|| {
                col()
                    .w_full()
                    .child(el().w_full().h(1.).fill(t.line))
                    .child(
                        menu::row_item(
                            "duplicate-block",
                            "Duplicate",
                            rune::icon(&phosphor::COPY, 16.).color(t.muted),
                            t,
                        )
                        .on_press(|s: &mut State| s.block_command_action(true)),
                    )
                    .child(
                        menu::row_item(
                            "delete-block",
                            "Delete",
                            rune::icon(&phosphor::TRASH, 16.).color(t.error),
                            Theme { text: t.error, ..t },
                        )
                        .on_press(|s: &mut State| s.block_command_action(false)),
                    )
            }))
            .children(current.error.as_ref().map(|error| {
                col()
                    .w_full()
                    .gap(4.)
                    .child(t.label(error, t.error).wrap())
                    .child(
                        button("block-command-retry", "Retry", t)
                            .on_press(|s: &mut State| s.retry_block_command()),
                    )
                    .child(
                        button("block-command-keep-draft", "Keep draft and close", t)
                            .on_press(|s: &mut State| s.keep_block_command_draft()),
                    )
            }))
            .child(t.label("Esc to close", t.faint).px(10.))
            .anchored_fit(anchor, Side::Bottom, Align::Start, 4.),
    )
}

fn link_picker(s: &State, t: Theme) -> El {
    let menu = s.block_commands.as_ref().unwrap();
    let anchor = menu.anchor.clone();
    let input = super::inputs::field(
        &s.block_command_search,
        "block-link-search",
        "Search pages or paste a URL…",
        t,
    )
    .label("Search pages or enter a website")
    .on_edit(|s: &mut State| &mut s.block_command_search)
    .on_input_change(|s: &mut State, tx| {
        s.refresh_block_link_results();
        tx.instant();
    })
    .on_input_key(|s: &mut State, event, tx| match event.key {
        Key::Escape => {
            s.dismiss_block_commands();
            tx.instant();
            true
        }
        Key::Up | Key::Down => {
            let count = s.block_link_count();
            if let Some(menu) = &mut s.block_commands {
                menu.selected = if event.key == Key::Up {
                    menu.selected.saturating_sub(1)
                } else {
                    menu.selected.saturating_add(1).min(count.saturating_sub(1))
                };
            }
            tx.instant();
            true
        }
        _ => false,
    })
    .on_submit(|s: &mut State, tx| {
        s.submit_block_link();
        tx.instant();
    });
    let rows = menu.link_results.iter().enumerate().map(|(i, hit)| {
        let id = hit.page_id.clone();
        menu::row_item(
            format!("link-page-{id}"),
            &hit.title,
            rune::icon(&phosphor::FILE, 16.).color(t.muted),
            t,
        )
        .fill(if menu.selected == i {
            t.selected
        } else {
            Color::TRANSPARENT
        })
        .on_press(move |s: &mut State| s.insert_block_link(Some(id.clone()), None))
    });
    let url = s.block_command_search.text.trim().to_owned();
    let website = (url.starts_with("http://") || url.starts_with("https://")).then(|| {
        menu::row_item(
            "link-website",
            &url,
            rune::icon(&phosphor::LINK, 16.).color(t.muted),
            t,
        )
        .fill(if menu.selected == menu.link_results.len() {
            t.selected
        } else {
            Color::TRANSPARENT
        })
        .on_press(move |s: &mut State| s.insert_block_link(None, Some(url.clone())))
    });
    col()
        .key("block-link-picker")
        .w(320.)
        .max_h(400.)
        .padding(8.)
        .gap(6.)
        .fill(t.menu)
        .border(1., t.line)
        .radius(8.)
        .shadow(4., 16., Color::BLACK.alpha(0.15))
        .role("dialog")
        .label("Link to page or website")
        .focus_group()
        .focus_return(format!(
            "{}{}",
            if menu.peek {
                "peek-block-"
            } else {
                "block-input-"
            },
            menu.block_id
        ))
        .modal(true, |s: &mut State| s.dismiss_block_commands())
        .child(input)
        .child(
            col()
                .w_full()
                .min_h(0.)
                .max_h(300.)
                .shrink()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(rows)
                .children(website),
        )
        .children(menu.error.as_ref().map(|error| {
            col()
                .w_full()
                .gap(4.)
                .child(t.label(error, t.error).wrap())
                .child(
                    button("block-link-retry", "Retry", t)
                        .on_press(|s: &mut State| s.retry_block_command()),
                )
                .child(
                    button("block-link-keep-draft", "Keep draft and close", t)
                        .on_press(|s: &mut State| s.keep_block_command_draft()),
                )
        }))
        .anchored_fit(anchor, Side::Bottom, Align::Start, 4.)
}
