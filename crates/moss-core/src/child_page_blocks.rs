use crate::{
    Block, BlockKind, Page, PageKind, Store, new_id, now, search,
    store::{decode, encode, invalid},
};
use rusqlite::{Result, TransactionBehavior, params};

impl Store {
    /// Save the link and child together; a stale parent must not create an orphan.
    pub fn create_child_page_block(
        &mut self,
        workspace_id: &str,
        parent_id: &str,
        expected: &[Block],
        target_id: &str,
        insert_after: bool,
    ) -> Result<(Page, Vec<Block>)> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let parent = search::page(&tx, parent_id)?.ok_or_else(|| invalid("parent missing"))?;
        if parent.workspace_id != workspace_id
            || parent.deleted_at.is_some()
            || parent.kind == PageKind::Database
        {
            return Err(invalid("parent is not a live content page"));
        }
        let mut current: Vec<Block> = tx
            .prepare("SELECT data FROM blocks WHERE page_id=?1 ORDER BY sort_order,id")?
            .query_map([parent_id], |r| r.get::<_, String>(0))?
            .map(|r| decode(r?))
            .collect::<Result<_>>()?;
        if current != expected {
            return Err(invalid("parent content changed in another editor"));
        }
        let index = if let Some(index) = current.iter().position(|b| b.id == target_id) {
            index
        } else if current.is_empty() {
            let mut empty = Block::paragraph(parent_id, "", 0);
            empty.id = target_id.into();
            current.push(empty);
            0
        } else {
            return Err(invalid("target block missing"));
        };
        let mut child = parent.clone();
        child.id = new_id();
        child.parent_id = Some(parent_id.into());
        child.title.clear();
        child.icon.clear();
        child.cover = None;
        child.kind = PageKind::Document;
        child.settings = Default::default();
        child.created_at = now();
        child.updated_at = child.created_at.clone();
        let mut blocks = current;
        let reuse = blocks[index].kind == BlockKind::Paragraph && blocks[index].text.is_empty();
        let position = if insert_after || !reuse {
            index + 1
        } else {
            index
        };
        let mut link = Block::paragraph(parent_id, "", position as i64);
        link.kind = BlockKind::ChildPage;
        link.config = serde_json::json!({"page_id": child.id});
        if position == index {
            blocks[index] = link;
        } else {
            blocks.insert(position, link);
        }
        for (i, block) in blocks.iter_mut().enumerate() {
            block.sort_order = i as i64;
        }
        tx.execute(
            "INSERT INTO pages(id,workspace_id,parent_id,deleted_at,data) VALUES(?1,?2,?3,NULL,?4)",
            params![child.id, workspace_id, parent_id, encode(&child)?],
        )?;
        search::index(&tx, &format!("page:{}", child.id), &child, "")?;
        crate::comments::update_block_anchors(&tx, parent_id, &blocks)?;
        tx.execute("DELETE FROM blocks WHERE page_id=?1", [parent_id])?;
        for block in expected {
            search::remove(&tx, &format!("block:{}", block.id))?;
        }
        for block in &blocks {
            tx.execute(
                "INSERT INTO blocks(id,page_id,sort_order,data) VALUES(?1,?2,?3,?4)",
                params![block.id, parent_id, block.sort_order, encode(block)?],
            )?;
            search::index(
                &tx,
                &format!("block:{}", block.id),
                &parent,
                &format!("{} {}", block.text, block.config),
            )?;
        }
        tx.commit()?;
        Ok((child, blocks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child_and_link_are_atomic_and_archive_references_remap() {
        let mut s = Store::in_memory().unwrap();
        let w = s.create_workspace("Test", "").unwrap();
        let p = s
            .create_page(&w.id, None, "Parent", "", PageKind::Document)
            .unwrap();
        let b = Block::paragraph(&p.id, "", 0);
        s.save_blocks(&p.id, std::slice::from_ref(&b)).unwrap();
        let (child, blocks) = s
            .create_child_page_block(&w.id, &p.id, std::slice::from_ref(&b), &b.id, false)
            .unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].config["page_id"], child.id);
        assert_eq!(child.parent_id.as_deref(), Some(p.id.as_str()));
        assert!(
            s.create_child_page_block(&w.id, &p.id, std::slice::from_ref(&b), &b.id, false)
                .is_err()
        );
        assert_eq!(
            s.page_children(&w.id, Some(&p.id), crate::PagePosition::Start, 50)
                .unwrap()
                .items
                .len(),
            1
        );
        let archive = s.export_workspace(&w.id).unwrap();
        let imported = s.import_workspace(&archive).unwrap();
        let root = s
            .page_children(&imported.id, None, crate::PagePosition::Start, 50)
            .unwrap()
            .items
            .remove(0)
            .page;
        let link = s.get_blocks(&root.id).unwrap().remove(0);
        let linked = s
            .get_page(link.config["page_id"].as_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(linked.parent_id.as_deref(), Some(root.id.as_str()));
        assert_ne!(linked.id, child.id);
    }
}
