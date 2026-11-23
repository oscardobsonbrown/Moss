use crate::{
    Block, BlockKind, Store, search,
    store::{decode, encode, invalid},
};
use rusqlite::{OptionalExtension, Result, params};

impl Store {
    /// Read, validate and attach a selected image on a worker thread.
    /// The existing database is opened without schema changes or trash cleanup.
    pub fn import_block_image_file(
        database: impl AsRef<std::path::Path>,
        workspace: &str,
        page_id: &str,
        block_id: &str,
        expected: Option<&str>,
        path: impl AsRef<std::path::Path>,
    ) -> Result<Block> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|e| invalid(&e.to_string()))?
            .take(12 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| invalid(&e.to_string()))?;
        Self::import_block_image_bytes(database, workspace, page_id, block_id, expected, &bytes)
    }
    /// Validate and attach raw image bytes (e.g. from clipboard paste) on a worker thread.
    pub fn import_block_image_bytes(
        database: impl AsRef<std::path::Path>,
        workspace: &str,
        page_id: &str,
        block_id: &str,
        expected: Option<&str>,
        bytes: &[u8],
    ) -> Result<Block> {
        if bytes.len() > 12 * 1024 * 1024 {
            return Err(invalid("image exceeds 12 MiB"));
        }
        let conn = rusqlite::Connection::open_with_flags(
            database,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
        )?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let mut store = Store { conn, history_actor: None, history_coalesce: true };
        let asset = store.import_cover_image(bytes)?;
        store.set_block_image_if_unchanged(workspace, page_id, block_id, expected, &asset)
    }
    /// Attach an imported image without overwriting a newer image or other block changes.
    pub fn set_block_image_if_unchanged(
        &mut self,
        workspace: &str,
        page_id: &str,
        block_id: &str,
        expected: Option<&str>,
        asset: &str,
    ) -> Result<Block> {
        self.read_cover_image(asset)?;
        self.replace_block_image(workspace, page_id, block_id, expected, Some(asset))
    }
    /// Clear an image only when its saved asset still matches `expected`.
    pub fn clear_block_image_if_unchanged(
        &mut self,
        workspace: &str,
        page_id: &str,
        block_id: &str,
        expected: &str,
    ) -> Result<Block> {
        self.replace_block_image(workspace, page_id, block_id, Some(expected), None)
    }
    fn replace_block_image(
        &mut self,
        workspace: &str,
        page_id: &str,
        block_id: &str,
        expected: Option<&str>,
        asset: Option<&str>,
    ) -> Result<Block> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let page = search::page(&tx, page_id)?.ok_or_else(|| invalid("page missing"))?;
        if page.workspace_id != workspace || page.deleted_at.is_some() {
            return Err(invalid("page is not available"));
        }
        let raw: String = tx
            .query_row(
                "SELECT data FROM blocks WHERE id=?1 AND page_id=?2",
                params![block_id, page_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| invalid("block missing"))?;
        let mut block: Block = decode(raw)?;
        if block.kind != BlockKind::Image
            || block
                .config
                .get("asset")
                .and_then(serde_json::Value::as_str)
                != expected
        {
            return Err(invalid("image changed in another editor"));
        }
        if block.config.is_null() {
            block.config = serde_json::json!({});
        }
        let config = block
            .config
            .as_object_mut()
            .ok_or_else(|| invalid("invalid image block settings"))?;
        if let Some(asset) = asset {
            config.insert("asset".into(), serde_json::json!(asset));
        } else {
            config.remove("asset");
        }
        tx.execute(
            "UPDATE blocks SET data=?1 WHERE id=?2",
            params![encode(&block)?, block_id],
        )?;
        tx.commit()?;
        Ok(block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PageKind, new_id};
    #[test]
    fn editor_saves_preserve_concurrent_images_and_allow_explicit_changes() {
        let path = std::env::temp_dir().join(format!("moss-image-editor-race-{}", new_id()));
        std::fs::create_dir_all(&path).unwrap();
        let db = path.join("workspace.sqlite3");
        let mut store = Store::open(&db).unwrap();
        let ws = store.create_workspace("Pictures", "").unwrap();
        let page = store
            .create_page(&ws.id, None, "Page", "", PageKind::Document)
            .unwrap();
        let mut stale = Block::paragraph(&page.id, "before", 0);
        stale.kind = BlockKind::Image;
        stale.config = serde_json::json!({"extra":"keep"});
        let paragraph = Block::paragraph(&page.id, "other", 1);
        store
            .save_editor_blocks(&page.id, &[stale.clone(), paragraph.clone()])
            .unwrap();
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let image = path.join("image.png");
        std::fs::write(&image, bytes.into_inner()).unwrap();
        let attached =
            Store::import_block_image_file(&db, &ws.id, &page.id, &stale.id, None, &image).unwrap();
        let asset = attached.config["asset"].as_str().unwrap().to_owned();
        stale.text = "edited concurrently".into();
        let mut other = paragraph.clone();
        other.text = "also edited".into();
        let saved = store
            .save_editor_blocks(&page.id, &[stale.clone(), other.clone()])
            .unwrap();
        assert_eq!(saved[0].config["asset"], asset);
        assert_eq!(saved[0].config["extra"], "keep");
        assert_eq!(saved[0].text, "edited concurrently");
        assert_eq!(saved[1].text, "also edited");
        assert!(
            Store::import_block_image_file(&db, &ws.id, &page.id, &stale.id, None, &image).is_err()
        );
        let mut second = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(3, 2)
            .write_to(&mut second, image::ImageFormat::Png)
            .unwrap();
        let second = store.import_cover_image(&second.into_inner()).unwrap();
        assert!(
            store
                .set_block_image_if_unchanged(&ws.id, &page.id, &stale.id, None, &second)
                .is_err()
        );
        store
            .set_block_image_if_unchanged(&ws.id, &page.id, &stale.id, Some(&asset), &second)
            .unwrap();
        assert_eq!(
            store
                .save_editor_blocks(&page.id, &[stale.clone(), other.clone()])
                .unwrap()[0]
                .config["asset"],
            second
        );
        assert!(
            store
                .clear_block_image_if_unchanged(&ws.id, &page.id, &stale.id, &asset)
                .is_err()
        );
        store
            .clear_block_image_if_unchanged(&ws.id, &page.id, &stale.id, &second)
            .unwrap();
        assert!(
            store
                .save_editor_blocks(&page.id, &[stale.clone(), other.clone()])
                .unwrap()[0]
                .config
                .get("asset")
                .is_none()
        );
        let mut explicit = stale.clone();
        explicit.config["asset"] = serde_json::json!(asset);
        store
            .save_blocks(&page.id, &[explicit.clone(), other.clone()])
            .unwrap();
        assert_eq!(
            store.get_blocks(&page.id).unwrap()[0].config["asset"],
            asset
        );
        let mut replacement = Block::paragraph(&page.id, "changed kind", 0);
        replacement.id = stale.id.clone();
        store
            .save_editor_blocks(&page.id, &[replacement, other.clone()])
            .unwrap();
        assert_eq!(
            store.get_blocks(&page.id).unwrap()[0].kind,
            BlockKind::Paragraph
        );
        store.save_editor_blocks(&page.id, &[other]).unwrap();
        assert_eq!(store.get_blocks(&page.id).unwrap().len(), 1);
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn image_attach_is_atomic_and_rejects_a_stale_upload() {
        let path = std::env::temp_dir().join(format!("moss-image-atomic-{}", new_id()));
        std::fs::create_dir_all(&path).unwrap();
        let mut store = Store::open(path.join("workspace.sqlite3")).unwrap();
        let ws = store.create_workspace("Pictures", "").unwrap();
        let page = store
            .create_page(&ws.id, None, "Page", "", PageKind::Document)
            .unwrap();
        let mut output = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut output, image::ImageFormat::Png)
            .unwrap();
        let asset = store.import_cover_image(&output.into_inner()).unwrap();
        let mut block = Block::paragraph(&page.id, "caption", 0);
        block.kind = BlockKind::Image;
        block.config = serde_json::json!({"extra": "keep"});
        store.save_blocks(&page.id, &[block.clone()]).unwrap();
        store
            .set_block_image_if_unchanged(&ws.id, &page.id, &block.id, None, &asset)
            .unwrap();
        assert!(
            store
                .set_block_image_if_unchanged(&ws.id, &page.id, &block.id, None, &asset)
                .is_err()
        );
        let saved = store.get_blocks(&page.id).unwrap().remove(0);
        assert_eq!(saved.config["extra"], "keep");
        assert_eq!(saved.config["asset"], asset);
        assert_eq!(saved.text, "caption");
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
    }
}
