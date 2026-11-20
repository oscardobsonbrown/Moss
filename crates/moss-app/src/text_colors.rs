use moss_core::Block;
use rune::{
    Color, Span, TextEdit,
    color::{hex, oklch},
};
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
    pub const ALL: [(Self, &'static str); 9] = [
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

    pub fn paint(self, channel: Channel, surface: Color) -> Color {
        let dark = surface.luminance() < 0.3;
        let (hue, chroma) = match self {
            Self::Gray => (260., 0.012),
            Self::Brown => (57., 0.085),
            Self::Orange | Self::Yellow => {
                let value = match (self, dark, channel) {
                    (Self::Orange, false, Channel::Text) => "#A74703",
                    (Self::Orange, false, Channel::Background) => "#FFE3C9",
                    (Self::Orange, true, Channel::Text) => "#FFAA63",
                    (Self::Orange, true, Channel::Background) => "#633819",
                    (_, false, Channel::Text) => "#765B00",
                    (_, false, Channel::Background) => "#FFF4C5",
                    (_, true, Channel::Text) => "#F4D66D",
                    (_, true, Channel::Background) => "#514717",
                };
                return hex(value).unwrap();
            }
            Self::Green => (150., 0.13),
            Self::Blue => (252., 0.16),
            Self::Purple => (305., 0.16),
            Self::Pink => (350., 0.16),
            Self::Red => (25., 0.18),
        };
        match channel {
            Channel::Text => oklch(if dark { 0.81 } else { 0.43 }, chroma, hue),
            Channel::Background => surface.mix(
                oklch(if dark { 0.57 } else { 0.80 }, chroma, hue),
                if dark { 0.28 } else { 0.30 },
            ),
        }
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

pub fn spans(block: &Block, surface: Color) -> impl Iterator<Item = Span> {
    runs(block).into_iter().map(move |r| {
        let span = Span::new(r.start, r.end);
        let color = r.color.paint(r.channel, surface);
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

#[cfg(test)]
mod tests {
    use super::*;

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
