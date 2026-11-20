use super::{phosphor, theme::Theme, view_icons::catalog_icon};
use crate::{
    block_commands::BlockCommandMode,
    state::{EditableBlock, PeekMode, State},
};
use moss_core::BlockKind;
use rune::{edit::Mark, *};

const HEIGHT: f32 = 36.;

pub fn view(s: &State, cx: &Cx<State>, t: Theme) -> Option<El> {
    if s.modal.is_some() || s.block_commands.is_some() || s.editor_drag.is_some() {
        return None;
    }
    let selected = |block: &&EditableBlock| {
        let edit = &block.edit;
        edit.focused && edit.anchor != edit.focus && edit.preedit.is_none()
    };
    let (page, block, peek) = if let Some(p) = &s.record_peek
        && let Some(b) = p.blocks.iter().find(selected)
    {
        (&p.page.id, b, true)
    } else {
        if s.record_peek
            .as_ref()
            .is_some_and(|p| p.mode == PeekMode::Center)
        {
            return None;
        }
        let tab = s.tab()?;
        (&tab.page.id, tab.blocks.iter().find(selected)?, false)
    };
    let id = &block.block.id;
    let input = input_key(id, peek);
    let [x, y, _, line_height] = cx.caret(&input)?;
    let viewport = if peek {
        "peek-scroll".into()
    } else {
        format!("document-scroll-{page}")
    };
    let [left, top, width, height] = cx.window_layout(&viewport)?;
    if y + line_height <= top || y >= top + height || x < left || x > left + width {
        return None;
    }
    let code_block = block.block.kind == BlockKind::Code;
    let width = if code_block { 160. } else { 343. };
    let [window_width, window_height] = cx.layout_size("root")?;
    let left = (x - width / 2.).clamp(6., (window_width - width - 6.).max(6.));
    let above = y - HEIGHT - 6.;
    let top = if above >= top {
        above
    } else {
        y + line_height + 6.
    };
    let top = top.min(window_height - HEIGHT - 6.).max(0.);
    let marks = [
        ("bold", "Bold", "text-b", Mark::Bold),
        ("italic", "Italic", "text-italic", Mark::Italic),
        ("underline", "Underline", "text-underline", Mark::Underline),
        (
            "strike",
            "Strikethrough",
            "text-strikethrough",
            Mark::Strike,
        ),
        ("code", "Inline code", "code", Mark::Code),
    ];
    let (type_id, link_id, comment_id, comment_page, color_id) =
        (id.clone(), id.clone(), id.clone(), page.clone(), id.clone());
    let selection = block.edit.range();
    Some(
        row()
            .key("text-toolbar")
            .role("toolbar")
            .label("Text formatting")
            .absolute(left, top)
            .h(HEIGHT)
            .px(4.)
            .gap(2.)
            .center_y()
            .fill(t.menu)
            .border(1., t.line)
            .radius(9.)
            .shadow(3., 12., Color::BLACK.alpha(0.16))
            .keeps_text_selection()
            .child(
                control("type", "Turn into", 87., false, t)
                    .gap(5.)
                    .child(t.sized_label("Turn into", 12., t.text))
                    .child(icon(&phosphor::CARET_DOWN, 10.).color(t.muted))
                    .on_press(move |s: &mut State| {
                        open_menu(s, &type_id, peek, BlockCommandMode::TextType);
                    }),
            )
            .child(el().size(1., 17.).fill(t.line))
            .children((!code_block).then(|| {
                let active = block.edit.active_link().is_some();
                control("link", "Link", 28., active, t)
                    .child(icon(&phosphor::LINK, 16.).color(ink(active, t)))
                    .on_press(move |s: &mut State| {
                        open_link(s, &link_id, peek);
                    })
            }))
            .child(
                control("comment", "Add comment", 28., false, t)
                    .child(icon(catalog_icon("chat-circle-text").unwrap(), 16.).color(t.text))
                    .on_press(move |s: &mut State| {
                        s.comment_for_block(&comment_page, &comment_id, peek)
                    }),
            )
            .children((!code_block).then(|| el().size(1., 17.).fill(t.line)))
            .children(
                marks
                    .into_iter()
                    .filter(|(_, _, _, mark)| !code_block && block.edit.allows_mark(mark))
                    .map(|(key, name, symbol, mark)| {
                        let id = id.clone();
                        let active = block.edit.has_mark(&mark);
                        control(key, name, 28., active, t)
                            .child(icon(catalog_icon(symbol).unwrap(), 16.).color(ink(active, t)))
                            .on_press(move |s: &mut State| {
                                toggle(s, &id, peek, mark.clone());
                            })
                    }),
            )
            .child(
                control("color", "Color and highlight", 28., false, t)
                    .child(icon(catalog_icon("palette").unwrap(), 16.).color(t.text))
                    .on_press(move |s: &mut State| {
                        open_menu(
                            s,
                            &color_id,
                            peek,
                            BlockCommandMode::Color {
                                selection: Some(selection),
                            },
                        );
                    }),
            ),
    )
}

fn ink(active: bool, t: Theme) -> Color {
    if active { t.blue } else { t.text }
}

fn control(key: &str, label: &str, width: f32, active: bool, t: Theme) -> El {
    row()
        .key(format!("text-toolbar-{key}"))
        .role("button")
        .label(label)
        .help(label)
        .focusable(true)
        .size(width, 28.)
        .center()
        .radius(5.)
        .fill(if active {
            t.blue.alpha(0.13)
        } else {
            Color::TRANSPARENT
        })
        .hover(t.hover)
        .press(t.selected)
}

fn input_key(id: &str, peek: bool) -> String {
    if peek {
        format!("peek-block-{id}")
    } else {
        format!("block-input-{id}")
    }
}

fn open_menu(s: &mut State, id: &str, peek: bool, mode: BlockCommandMode) {
    let anchor = input_key(id, peek);
    if peek {
        s.open_peek_block_commands(id, &anchor, mode);
    } else {
        s.open_block_commands(id, &anchor, mode);
    }
}

fn open_link(s: &mut State, id: &str, peek: bool) -> bool {
    let Some(block) = focused_block(s, id, peek) else {
        return false;
    };
    let selection = block.edit.range();
    if selection.0 == selection.1 || block.block.kind == BlockKind::Code {
        return false;
    }
    let address = block
        .edit
        .active_link()
        .filter(|url| !url.starts_with("moss-page:"))
        .unwrap_or_default()
        .to_owned();
    let mode = BlockCommandMode::TextLink { selection };
    open_menu(s, id, peek, mode);
    if let Some(menu) = &mut s.block_commands
        && menu.mode == mode
    {
        menu.link_picker = true;
        s.block_command_search.replace_text(&address);
        s.block_command_search.set_selection(0, address.len());
        s.refresh_block_link_results();
    }
    true
}

fn focused_block<'a>(s: &'a mut State, id: &str, peek: bool) -> Option<&'a mut EditableBlock> {
    let blocks = if peek {
        &mut s.record_peek.as_mut()?.blocks
    } else {
        &mut s.tabs.get_mut(s.active)?.blocks
    };
    blocks
        .iter_mut()
        .find(|b| b.block.id == id && b.edit.focused)
}

fn toggle(s: &mut State, id: &str, peek: bool, mark: Mark) -> bool {
    let Some(block) = focused_block(s, id, peek) else {
        return false;
    };
    if block.edit.anchor == block.edit.focus || block.block.kind == BlockKind::Code {
        return false;
    }
    block.edit.toggle_mark(mark);
    if peek {
        s.save_peek_blocks();
    } else {
        s.save_blocks();
    }
    true
}

pub fn shortcut(s: &mut State, id: &str, peek: bool, event: &KeyEvent, tx: &mut Tx) -> bool {
    let handled = match event.key {
        Key::Command('k') if !event.mods.shift => open_link(s, id, peek),
        Key::Command('/') if !event.mods.shift => {
            let Some(block) = focused_block(s, id, peek) else {
                return false;
            };
            if block.edit.anchor == block.edit.focus {
                return false;
            }
            open_menu(s, id, peek, BlockCommandMode::TextType);
            true
        }
        Key::Command('m') if event.mods.shift => {
            let Some(block) = focused_block(s, id, peek) else {
                return false;
            };
            if block.edit.anchor == block.edit.focus {
                return false;
            }
            let page = block.block.page_id.clone();
            s.comment_for_block(&page, id, peek);
            true
        }
        Key::Command('s') if event.mods.shift => toggle(s, id, peek, Mark::Strike),
        Key::Command('e') if !event.mods.shift => toggle(s, id, peek, Mark::Code),
        _ => false,
    };
    if handled {
        tx.instant();
    }
    handled
}
