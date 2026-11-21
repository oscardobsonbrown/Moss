use crate::state::{EditableBlock, State};
use moss_core::{Block, Store};
use rune::Tx;
use std::time::Duration;

fn refresh_blocks(
    store: &Store,
    page_id: &str,
    blocks: &mut Vec<EditableBlock>,
    saved: &mut Vec<Block>,
    dirty: bool,
) -> Result<(bool, bool), Box<dyn std::error::Error>> {
    if dirty {
        return Ok((false, true));
    }
    let current = store.get_blocks(page_id)?;
    if current == *saved {
        return Ok((false, false));
    }
    let same_structure = current.len() == saved.len()
        && current.len() == blocks.len()
        && current
            .iter()
            .zip(saved.iter())
            .zip(blocks.iter())
            .all(|((a, b), c)| a.id == b.id && b.id == c.block.id);
    if same_structure {
        let mut changed = false;
        let mut pending = false;
        for ((new, old), editable) in current.iter().zip(saved.iter_mut()).zip(blocks.iter_mut()) {
            if new == old {
                continue;
            }
            if editable.edit.focused || editable.edit.preedit.is_some() {
                pending = true;
                continue;
            }
            *editable = EditableBlock {
                block: new.clone(),
                edit: crate::rich_text::load(new),
            };
            *old = new.clone();
            changed = true;
        }
        return Ok((changed, pending));
    }
    if blocks
        .iter()
        .any(|block| block.edit.focused || block.edit.preedit.is_some())
    {
        return Ok((false, true));
    }
    *saved = current.clone();
    *blocks = if current.is_empty() {
        let block = Block::paragraph(page_id, "", 0);
        vec![EditableBlock {
            edit: crate::rich_text::load(&block),
            block,
        }]
    } else {
        current
            .into_iter()
            .map(|block| EditableBlock {
                edit: crate::rich_text::load(&block),
                block,
            })
            .collect()
    };
    Ok((true, false))
}

pub fn poll(s: &mut State, time: f64, tx: &mut Tx) -> bool {
    if !s.collaboration_live && !s.external_refresh_pending {
        return false;
    }
    if s.tabs.is_empty() && s.record_peek.is_none() {
        return false;
    }
    tx.wake_after(Duration::from_millis(250));
    if time < s.external_check_at {
        return false;
    }
    s.external_check_at = time + 0.25;
    let version = match s.store.external_data_version() {
        Ok(version) => version,
        Err(error) => {
            s.error = Some(format!("Changes could not be checked: {error}"));
            return false;
        }
    };
    let external_changed = version != s.external_data_version;
    if !external_changed && !s.external_refresh_pending {
        return false;
    }
    s.external_data_version = version;
    let mut changed = false;
    let mut pending = false;
    for tab in &mut s.tabs {
        match refresh_blocks(
            &s.store,
            &tab.page.id,
            &mut tab.blocks,
            &mut tab.saved_blocks,
            tab.dirty_blocks,
        ) {
            Ok((updated, deferred)) => {
                changed |= updated;
                pending |= deferred;
            }
            Err(error) => s.error = Some(format!("Page changes could not be loaded: {error}")),
        }
    }
    if let Some(peek) = &mut s.record_peek {
        match refresh_blocks(
            &s.store,
            &peek.page.id,
            &mut peek.blocks,
            &mut peek.saved_blocks,
            peek.dirty_blocks,
        ) {
            Ok((updated, deferred)) => {
                changed |= updated;
                pending |= deferred;
            }
            Err(error) => s.error = Some(format!("Page changes could not be loaded: {error}")),
        }
    }
    s.external_refresh_pending = pending;
    if external_changed {
        s.computed_change_count = s.store.local_change_count().wrapping_add(1);
        let mut pages = Vec::new();
        if let Some(tab) = s.tab() {
            pages.push(tab.page.id.clone());
        }
        if let Some(peek) = &s.record_peek {
            if !pages.contains(&peek.page.id) {
                pages.push(peek.page.id.clone());
            }
        }
        for page_id in pages {
            s.load_comments(&page_id);
        }
    }
    changed || external_changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::PageKind;

    #[test]
    fn refreshes_other_blocks_without_replacing_the_focused_editor() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Shared", "").unwrap();
        let page = store
            .create_page(&workspace.id, None, "Page", "", PageKind::Document)
            .unwrap();
        let first = Block::paragraph(&page.id, "first", 0);
        let second = Block::paragraph(&page.id, "second", 1);
        let mut saved = vec![first.clone(), second.clone()];
        store.save_blocks(&page.id, &saved).unwrap();
        let mut blocks = saved
            .iter()
            .cloned()
            .map(|block| EditableBlock {
                edit: crate::rich_text::load(&block),
                block,
            })
            .collect::<Vec<_>>();
        blocks[0].edit.focused = true;
        blocks[0].edit.set_caret(2);
        let mut remote = saved.clone();
        remote[1].text = "changed remotely".into();
        store.save_blocks(&page.id, &remote).unwrap();
        assert_eq!(
            refresh_blocks(&store, &page.id, &mut blocks, &mut saved, false).unwrap(),
            (true, false)
        );
        assert!(blocks[0].edit.focused);
        assert_eq!(blocks[0].edit.focus, 2);
        assert_eq!(blocks[1].edit.text, "changed remotely");
        assert_eq!(saved, remote);
        remote[0].text = "conflicting change".into();
        store.save_blocks(&page.id, &remote).unwrap();
        assert_eq!(
            refresh_blocks(&store, &page.id, &mut blocks, &mut saved, false).unwrap(),
            (false, true)
        );
        assert_eq!(blocks[0].edit.text, "first");
        blocks[0].edit.focused = false;
        assert_eq!(
            refresh_blocks(&store, &page.id, &mut blocks, &mut saved, false).unwrap(),
            (true, false)
        );
        assert_eq!(blocks[0].edit.text, "conflicting change");
    }
}
