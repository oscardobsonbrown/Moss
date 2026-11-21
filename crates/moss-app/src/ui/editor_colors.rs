use super::{
    phosphor,
    theme::{Theme, button},
};
use crate::{
    block_commands::BlockCommands,
    state::{EditableBlock, State},
    text_colors::{self, Channel, Palette},
};
use moss_core::{Block, BlockKind};
use rune::*;

fn target<'a>(s: &'a State, menu: &BlockCommands) -> Option<&'a EditableBlock> {
    let blocks = if menu.peek {
        &s.record_peek
            .as_ref()
            .filter(|p| p.page.id == menu.page_id)?
            .blocks
    } else {
        &s.tab().filter(|p| p.page.id == menu.page_id)?.blocks
    };
    blocks.iter().find(|b| b.block.id == menu.block_id)
}

pub fn can_color(s: &State, menu: &BlockCommands) -> bool {
    target(s, menu).is_some_and(|b| {
        !matches!(
            b.block.kind,
            BlockKind::Divider
                | BlockKind::SimpleTable
                | BlockKind::Image
                | BlockKind::Bookmark
                | BlockKind::ChildPage
        )
    })
}

pub fn block_paint(block: &Block, channel: Channel, t: Theme) -> Option<Color> {
    text_colors::block_color(block, channel).map(|color| Palette::paint(Some(color), channel, t.dark))
}

pub fn popup(
    s: &State,
    menu: &BlockCommands,
    selection: Option<(usize, usize)>,
    t: Theme,
) -> Option<El> {
    let block = &target(s, menu)?.block;
    let colors: Vec<_> = Palette::choices().collect();
    let groups = [
        (
            Channel::Text,
            if selection.is_some() {
                "Text color"
            } else {
                "Block text color"
            },
            "text",
        ),
        (
            Channel::Background,
            if selection.is_some() {
                "Highlight"
            } else {
                "Block background"
            },
            "background",
        ),
    ];
    let input = if menu.peek {
        "peek-block-"
    } else {
        "block-input-"
    };
    Some(
        col()
            .key("editor-color-picker")
            .label(if selection.is_some() {
                "Color selected text"
            } else {
                "Color block"
            })
            .role("dialog")
            .focus_group()
            .focus_return(format!("{input}{}", menu.block_id))
            .modal(true, |s: &mut State| s.dismiss_block_commands())
            .w(206.)
            .padding(10.)
            .gap(10.)
            .radius(8.)
            .border(1., t.line)
            .fill(t.menu)
            .shadow(4., 16., Color::BLACK.alpha(0.15))
            .anchored_fit(&menu.anchor, Side::Bottom, Align::Start, 4.)
            .children(groups.into_iter().map(|(channel, title, key)| {
                let active = text_colors::active_color(
                    block,
                    selection.map(|(start, end)| start..end),
                    channel,
                );
                let rows = colors.chunks(5).map(|colors| {
                    row()
                        .gap(5.)
                        .children(colors.iter().copied().map(|(color, name)| {
                            let current = color.is_some() && color == active;
                            let ink = Palette::paint(color, Channel::Text, t.dark);
                            let fill = Palette::paint(color, Channel::Background, t.dark);
                            let label = format!("{title}: {name}");
                            row()
                                .key(format!("editor-color-{key}-{name}"))
                                .size(32., 28.)
                                .center()
                                .radius(5.)
                                .fill(if channel == Channel::Background {
                                    fill
                                } else {
                                    t.menu
                                })
                                .border(1., if current { t.blue } else { t.line })
                                .hover(if channel == Channel::Background {
                                    fill.mix(t.hover, 0.25)
                                } else {
                                    t.hover
                                })
                                .label(&label)
                                .help(label)
                                .role("button")
                                .focusable(menu.error.is_none())
                                .child(
                                    icon(
                                        if color.is_some() {
                                            &phosphor::TEXT_T
                                        } else {
                                            &phosphor::X
                                        },
                                        13.,
                                    )
                                    .color(
                                        if channel == Channel::Text {
                                            ink
                                        } else {
                                            t.text
                                        },
                                    ),
                                )
                                .on_press_tx(move |s: &mut State, tx| {
                                    s.apply_editor_color(channel, color);
                                    tx.instant();
                                })
                        }))
                });
                col()
                    .gap(5.)
                    .child(t.sized_label(title, 12., t.muted))
                    .children(rows)
            }))
            .children(menu.error.as_ref().map(|error| {
                col()
                    .gap(4.)
                    .child(t.label(error, t.error).wrap())
                    .child(
                        button("editor-color-retry", "Retry", t)
                            .on_press(|s: &mut State| s.retry_block_command()),
                    )
                    .child(
                        button("editor-color-keep-draft", "Keep draft", t)
                            .on_press(|s: &mut State| s.keep_block_command_draft()),
                    )
            })),
    )
}
