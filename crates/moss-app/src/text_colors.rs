use moss_core::Block;
use rune::{Color, Span, TextEdit, color::hex};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::ops::Range;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Palette {
    Gray,
    Brown,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
    Red,
}

impl Palette {
    const ALL: [(Self, &'static str); 9] = [
        (Self::Gray, "Gray"),
        (Self::Brown, "Brown"),
        (Self::Orange, "Orange"),
        (Self::Yellow, "Yellow"),
        (Self::Green, "Green"),
        (Self::Blue, "Blue"),
        (Self::Purple, "Purple"),
        (Self::Pink, "Pink"),
        (Self::Red, "Red"),
    ];

    pub fn choices() -> impl Iterator<Item = (Option<Self>, &'static str)> {
        std::iter::once((None, "Default"))
            .chain(Self::ALL.into_iter().map(|(color, name)| (Some(color), name)))
    }

    // Text, background and icon, from https://matthiasfrank.de/en/notion-colors/
    fn tokens(color: Option<Self>, dark: bool) -> [&'static str; 3] {
        use Palette::*;
        match (color, dark) {
            (None, false) => ["#373530", "#FFFFFF", "#55534E"],
            (Some(Gray), false) => ["#787774", "#F1F1EF", "#A6A299"],
            (Some(Brown), false) => ["#976D57", "#F3EEEE", "#9F6B53"],
            (Some(Orange), false) => ["#CC782F", "#F8ECDF", "#D87620"],
            (Some(Yellow), false) => ["#C29343", "#FAF3DD", "#CB912F"],
            (Some(Green), false) => ["#548164", "#EEF3ED", "#448361"],
            (Some(Blue), false) => ["#487CA5", "#E9F3F7", "#337EA9"],
            (Some(Purple), false) => ["#8A67AB", "#F6F3F8", "#9065B0"],
            (Some(Pink), false) => ["#B35488", "#F9F2F5", "#C14C8A"],
            (Some(Red), false) => ["#C4554D", "#FAECEC", "#D44C47"],
            (None, true) => ["#D4D4D4", "#191919", "#D3D3D3"],
            (Some(Gray), true) => ["#9B9B9B", "#252525", "#7F7F7F"],
            (Some(Brown), true) => ["#A27763", "#2E2724", "#AA755F"],
            (Some(Orange), true) => ["#CB7B37", "#36291F", "#D9730D"],
            (Some(Yellow), true) => ["#C19138", "#372E20", "#CA8E1B"],
            (Some(Green), true) => ["#4F9768", "#242B26", "#2D9964"],
            (Some(Blue), true) => ["#447ACB", "#1F282D", "#2E7CD1"],
            (Some(Purple), true) => ["#865DBB", "#2A2430", "#8D5BC1"],
            (Some(Pink), true) => ["#BA4A78", "#2E2328", "#C94079"],
            (Some(Red), true) => ["#BE524B", "#332523", "#CD4945"],
        }
    }

    pub fn paint(color: Option<Self>, channel: Channel, dark: bool) -> Color {
        let [text, background, _] = Self::tokens(color, dark);
        hex(match channel {
            Channel::Text => text,
            Channel::Background => background,
        })
        .unwrap()
    }

    pub fn icon_paint(color: Option<Self>, dark: bool) -> Color {
        hex(Self::tokens(color, dark)[2]).unwrap()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Text,
    Background,
}

impl Channel {
    fn block_key(self) -> &'static str {
        match self {
            Self::Text => "block_text_color",
            Self::Background => "block_background_color",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Run {
    start: usize,
    end: usize,
    channel: Channel,
    color: Palette,
}

fn runs(block: &Block) -> Vec<Run> {
    block.config["inline_colors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| serde_json::from_value::<Run>(value.clone()).ok())
        .filter(|r| {
            r.start < r.end
                && block.text.is_char_boundary(r.start)
                && block.text.is_char_boundary(r.end)
        })
        .collect()
}

fn write_runs(block: &mut Block, mut colors: Vec<Run>) {
    colors.sort_by_key(|r| (r.channel as u8, r.start));
    let mut merged: Vec<Run> = Vec::with_capacity(colors.len());
    for run in colors {
        if let Some(last) = merged.last_mut()
            && last.channel == run.channel
            && last.color == run.color
            && last.end == run.start
        {
            last.end = run.end;
        } else {
            merged.push(run);
        }
    }
    if merged.is_empty() {
        if let Some(config) = block.config.as_object_mut() {
            config.remove("inline_colors");
        }
    } else {
        block.config["inline_colors"] = json!(merged);
    }
}

pub fn apply_inline(
    block: &mut Block,
    range: Range<usize>,
    channel: Channel,
    color: Option<Palette>,
) {
    let mut next = Vec::new();
    for run in runs(block) {
        if run.channel != channel || run.end <= range.start || range.end <= run.start {
            next.push(run);
        } else {
            if run.start < range.start {
                next.push(Run {
                    end: range.start,
                    ..run
                });
            }
            if range.end < run.end {
                next.push(Run {
                    start: range.end,
                    ..run
                });
            }
        }
    }
    if let Some(color) = color {
        next.push(Run {
            start: range.start,
            end: range.end,
            channel,
            color,
        });
    }
    write_runs(block, next);
}

pub fn block_color(block: &Block, channel: Channel) -> Option<Palette> {
    serde_json::from_value(block.config.get(channel.block_key())?.clone()).ok()
}

pub fn set_block_color(block: &mut Block, channel: Channel, color: Option<Palette>) {
    if let Some(color) = color {
        block.config[channel.block_key()] = json!(color);
    } else if let Some(config) = block.config.as_object_mut() {
        config.remove(channel.block_key());
    }
}

pub fn active_color(
    block: &Block,
    range: Option<Range<usize>>,
    channel: Channel,
) -> Option<Palette> {
    match range {
        Some(range) => runs(block)
            .into_iter()
            .find(|r| r.channel == channel && r.start <= range.start && range.end <= r.end)
            .map(|r| r.color),
        None => block_color(block, channel),
    }
}

pub fn spans(block: &Block, dark: bool) -> impl Iterator<Item = Span> {
    runs(block).into_iter().map(move |r| {
        let span = Span::new(r.start, r.end);
        let color = Palette::paint(Some(r.color), r.channel, dark);
        match r.channel {
            Channel::Text => span.color(color),
            Channel::Background => span.bg(color, 0.),
        }
    })
}

// shortcut: Track one replacement per save. Use Rune marks when available for native undo and clipboard support.
pub fn sync(block: &mut Block, edit: &TextEdit) {
    if block.config["inline_colors"].is_null() || block.text == edit.text {
        return;
    }
    let old = &block.text;
    let new = &edit.text;
    let prefix = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .map(|(ch, _)| ch.len_utf8())
        .sum::<usize>();
    let caret = edit.focus.min(new.len());
    let (start, old_end, new_end) = if let Some(tail) = new.get(caret..)
        && let Some(head) = old.strip_suffix(tail)
    {
        (prefix.min(head.len()).min(caret), head.len(), caret)
    } else {
        let suffix = old[prefix..]
            .chars()
            .rev()
            .zip(new[prefix..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(ch, _)| ch.len_utf8())
            .sum::<usize>();
        (prefix, old.len() - suffix, new.len() - suffix)
    };
    let colors = runs(block)
        .into_iter()
        .filter_map(|mut run| {
            if run.end > start {
                if run.start >= old_end {
                    run.start = new_end + (run.start - old_end);
                    run.end = new_end + (run.end - old_end);
                } else {
                    if run.start > start {
                        run.start = new_end;
                    }
                    run.end = if run.end >= old_end {
                        new_end + (run.end - old_end)
                    } else {
                        start
                    };
                }
            }
            (run.start < run.end).then_some(run)
        })
        .collect();
    write_runs(block, colors);
}

pub fn copy_suffix(source: &Block, suffix: &mut Block, start: usize) {
    let colors = runs(source)
        .into_iter()
        .filter(|r| r.end > start)
        .map(|r| Run {
            start: r.start.saturating_sub(start),
            end: r.end - start,
            ..r
        })
        .collect();
    write_runs(suffix, colors);
}

pub fn append_runs(target: &mut Block, source: &Block, offset: usize) {
    let source_runs = runs(source);
    if source_runs.is_empty() {
        return;
    }
    let mut combined = runs(target);
    combined.extend(source_runs.into_iter().map(|r| Run {
        start: r.start + offset,
        end: r.end + offset,
        ..r
    }));
    write_runs(target, combined);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orange_uses_separate_text_background_and_icon_colors_in_each_theme() {
        let orange = Some(Palette::Orange);
        for (dark, [text, background, icon]) in [
            (false, ["#CC782F", "#F8ECDF", "#D87620"]),
            (true, ["#CB7B37", "#36291F", "#D9730D"]),
        ] {
            assert_eq!(Palette::paint(orange, Channel::Text, dark), hex(text).unwrap());
            assert_eq!(Palette::paint(orange, Channel::Background, dark), hex(background).unwrap());
            assert_eq!(Palette::icon_paint(orange, dark), hex(icon).unwrap());
        }
    }

    #[test]
    fn inline_colors_follow_edits_and_split_with_the_text() {
        let mut block = Block::paragraph("page", "aaa café", 0);
        apply_inline(&mut block, 1..2, Channel::Background, Some(Palette::Yellow));
        apply_inline(&mut block, 4..9, Channel::Text, Some(Palette::Blue));
        let mut edit = crate::rich_text::load(&block);
        edit.set_caret(0);
        edit.insert("a");
        crate::rich_text::save(&mut block, &edit);
        assert_eq!(
            active_color(&block, Some(2..3), Channel::Background),
            Some(Palette::Yellow)
        );
        assert_eq!(
            active_color(&block, Some(5..10), Channel::Text),
            Some(Palette::Blue)
        );

        apply_inline(&mut block, 6..8, Channel::Text, None);
        let mut suffix = Block::paragraph("page", "café", 1);
        copy_suffix(&block, &mut suffix, 5);
        let mut suffix_edit = crate::rich_text::load(&suffix);
        suffix_edit.set_caret(3);
        suffix_edit.insert("x");
        crate::rich_text::save(&mut suffix, &suffix_edit);
        assert_eq!(
            active_color(&suffix, Some(0..1), Channel::Text),
            Some(Palette::Blue)
        );
        assert_eq!(active_color(&suffix, Some(1..4), Channel::Text), None);
        assert_eq!(
            active_color(&suffix, Some(4..6), Channel::Text),
            Some(Palette::Blue)
        );
        assert_eq!(active_color(&suffix, Some(0..1), Channel::Background), None);
    }
}
