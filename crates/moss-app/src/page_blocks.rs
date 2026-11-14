use crate::{
    block_commands::BlockCommandMode,
    state::{EditableBlock, State},
};
use moss_core::{Block, BlockKind, Page, Store};
use std::collections::BTreeMap;

pub fn load_links(
    store: &Store,
    workspace: &str,
    blocks: &[Block],
) -> Result<BTreeMap<String, Page>, Box<dyn std::error::Error>> {
    let mut links = BTreeMap::new();
    for block in blocks.iter().filter(|b| b.kind == BlockKind::ChildPage) {
        if let Some(id) = block.config["page_id"].as_str() {
            if let Some(page) = store
                .get_page(id)?
                .filter(|p| p.workspace_id == workspace && p.deleted_at.is_none())
            {
                links.insert(id.into(), page);
            }
        }
    }
    Ok(links)
}

impl State {
    pub fn focus_first_page_block(&mut self, peek: bool) {
        let target = if peek {
            self.record_peek.as_ref().map(|p| (&p.page, &p.blocks))
        } else {
            self.tab().map(|t| (&t.page, &t.blocks))
        }
        .and_then(|(p, blocks)| {
            blocks
                .iter()
                .position(|b| !matches!(b.block.kind, BlockKind::Divider | BlockKind::ChildPage))
                .map(|i| {
                    (
                        p.id.clone(),
                        blocks[i].block.id.clone(),
                        blocks[i].block.kind,
                        i,
                    )
                })
        });
        if let Some((page, block, kind, index)) = target {
            if kind == BlockKind::SimpleTable {
                self.start_simple_table_cell(&page, &block, 0, 0, peek);
            } else {
                let block = if peek {
                    &mut self.record_peek.as_mut().unwrap().blocks[index]
                } else {
                    &mut self.tabs[self.active].blocks[index]
                };
                block.edit.focused = true;
                block.edit.set_caret(0);
            }
        }
    }
    pub fn insert_child_page_block(&mut self) {
        let Some(menu) = &self.block_commands else {
            return;
        };
        if menu.error.is_some() {
            return;
        }
        let (parent, block, mode) = (menu.page_id.clone(), menu.block_id.clone(), menu.mode);
        if !self.tab().is_some_and(|t| t.page.id == parent) || !self.flush_current() {
            return;
        }
        let result = (|| -> Result<_, Box<dyn std::error::Error>> {
            let expected = self.store.get_blocks(&parent)?;
            Ok(self.store.create_child_page_block(
                &self.workspace.id,
                &parent,
                &expected,
                &block,
                mode == BlockCommandMode::InsertAfter,
            )?)
        })();
        match result {
            Ok((child, blocks)) => {
                self.track_new_child_page(&child, &blocks);
                let tab = &mut self.tabs[self.active];
                tab.blocks = blocks
                    .into_iter()
                    .map(|block| {
                        let edit = crate::rich_text::load(&block);
                        EditableBlock { block, edit }
                    })
                    .collect();
                tab.page_links.insert(child.id.clone(), child.clone());
                tab.dirty_blocks = false;
                self.block_commands = None;
                self.block_command_search.focused = false;
                self.refresh_child_pages(&parent);
                self.refresh_pages();
                self.open_page(&child.id, false);
            }
            Err(error) => {
                if let Some(menu) = &mut self.block_commands {
                    menu.error = Some(format!("Page is not created: {error}"));
                }
            }
        }
    }
    pub fn refresh_page_links(&mut self) {
        for tab in &mut self.tabs {
            let blocks: Vec<_> = tab.blocks.iter().map(|b| b.block.clone()).collect();
            match load_links(&self.store, &self.workspace.id, &blocks) {
                Ok(links) => tab.page_links = links,
                Err(error) => self.error = Some(format!("Page links are not loaded: {error}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_commands::BlockCommandMode;
    use moss_core::new_id;
    use rune::{Env, Runtime};
    #[test]
    fn slash_page_opens_child_and_renders_one_link_at_the_original_position() {
        let path = std::env::temp_dir().join(format!("moss-page-block-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(false, None);
        let parent = s.tab().unwrap().page.id.clone();
        let block = s.tab().unwrap().blocks[0].block.id.clone();
        s.tabs[s.active].blocks[0].edit.focused = true;
        s.open_block_commands(
            &block,
            &format!("block-input-{block}"),
            BlockCommandMode::Slash,
        );
        s.block_command_search.replace_text("PAGE");
        s.submit_block_command();
        let child = s.tab().unwrap().page.id.clone();
        assert_ne!(child, parent);
        assert_eq!(
            s.tab().unwrap().page.parent_id.as_deref(),
            Some(parent.as_str())
        );
        s.tabs[s.active].title.replace_text("Project notes");
        s.save_title();
        s.open_page(&parent, false);
        assert_eq!(
            s.tab()
                .unwrap()
                .blocks
                .iter()
                .filter(|b| b.block.kind == BlockKind::ChildPage)
                .count(),
            1
        );
        assert_eq!(s.tab().unwrap().page_links[&child].title, "Project notes");
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (1100., 800.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.frame(0.);
        let links = rt
            .elements()
            .iter()
            .filter(|e| {
                e.key
                    .as_deref()
                    .is_some_and(|k| k.starts_with("page-link-"))
            })
            .count();
        assert_eq!(links, 1);
        assert!(
            !rt.elements()
                .iter()
                .any(|e| e.key.as_deref() == Some(&format!("child-page-{child}")))
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
