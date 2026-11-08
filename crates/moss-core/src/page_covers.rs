use crate::{
    Page, PageCover, PageKind, Store, now, search,
    store::{encode, invalid},
};
use rusqlite::{OptionalExtension, Result, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MAX_IMAGE_BYTES: usize = 12 * 1024 * 1024;
fn io_error(error: impl std::fmt::Display) -> rusqlite::Error {
    invalid(&format!("cover asset: {error}"))
}
fn valid_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}
pub(crate) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(crate) fn valid_cover(cover: &PageCover) -> bool {
    match cover {
        PageCover::Color { color } => valid_color(color),
        PageCover::Gradient { start, end } => valid_color(start) && valid_color(end),
        PageCover::Image { asset, position } => {
            valid_digest(asset) && position.is_finite() && (0. ..=1.).contains(position)
        }
    }
}
pub(crate) fn validate_image(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return false;
    }
    let format = match image::guess_format(bytes) {
        Ok(
            format
            @ (image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP),
        ) => format,
        _ => return false,
    };
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().is_ok()
}

fn asset_directory(conn: &rusqlite::Connection) -> Result<PathBuf> {
    let db = conn
        .path()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| invalid("cover images require a disk workspace"))?;
    Ok(Path::new(db)
        .parent()
        .ok_or_else(|| invalid("workspace path missing"))?
        .join("cover-assets"))
}
pub(crate) fn asset_path(conn: &rusqlite::Connection, digest: &str) -> Result<PathBuf> {
    if !valid_digest(digest) {
        return Err(invalid("invalid cover asset"));
    }
    Ok(asset_directory(conn)?.join(digest))
}
pub(crate) fn save_asset(conn: &rusqlite::Connection, bytes: &[u8]) -> Result<String> {
    if !validate_image(bytes) {
        return Err(invalid(
            "cover must be a PNG, JPEG, or WebP image up to 12 MiB",
        ));
    }
    let digest = format!("{:x}", Sha256::digest(bytes));
    let path = asset_path(conn, &digest)?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(io_error)?;
    if path.exists() {
        use std::io::Read;
        let mut existing = Vec::new();
        std::fs::File::open(&path)
            .map_err(io_error)?
            .take((MAX_IMAGE_BYTES + 1) as u64)
            .read_to_end(&mut existing)
            .map_err(io_error)?;
        if existing != bytes {
            return Err(invalid("cover asset checksum mismatch"));
        }
    } else {
        let temporary = path.with_extension(format!("{}.tmp", crate::new_id()));
        std::fs::write(&temporary, bytes).map_err(io_error)?;
        if let Err(error) = std::fs::rename(&temporary, &path) {
            let _ = std::fs::remove_file(&temporary);
            return Err(io_error(error));
        }
    }
    Ok(digest)
}
impl Store {
    /// Import a local image into the managed cover asset directory.
    pub fn import_cover_image(&self, bytes: &[u8]) -> Result<String> {
        save_asset(&self.conn, bytes)
    }
    /// Read a managed image, checking its content hash before use.
    pub fn read_cover_image(&self, digest: &str) -> Result<Vec<u8>> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(asset_path(&self.conn, digest)?)
            .map_err(io_error)?
            .take((MAX_IMAGE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if !validate_image(&bytes) || format!("{:x}", Sha256::digest(&bytes)) != digest {
            return Err(invalid("cover asset checksum mismatch"));
        }
        Ok(bytes)
    }
    /// Change a cover without replacing newer page metadata.
    pub fn set_page_cover_if_unchanged(
        &mut self,
        workspace_id: &str,
        page_id: &str,
        expected: &Option<PageCover>,
        cover: Option<PageCover>,
    ) -> Result<Page> {
        if let Some(cover) = &cover {
            if !valid_cover(cover) {
                return Err(invalid("invalid page cover"));
            }
            if let PageCover::Image { asset, .. } = cover {
                self.read_cover_image(asset)?;
            }
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut page = search::page(&tx, page_id)?.ok_or_else(|| invalid("page missing"))?;
        if page.workspace_id != workspace_id || page.deleted_at.is_some() {
            return Err(invalid("page is not live in this workspace"));
        }
        if page.kind == PageKind::Record {
            let owner: Option<String> = tx.query_row("SELECT d.page_id FROM records r JOIN databases d ON d.id=r.database_id JOIN pages p ON p.id=d.page_id WHERE r.page_id=?1 AND r.deleted_at IS NULL AND p.deleted_at IS NULL AND p.workspace_id=?2", params![page_id, workspace_id], |r| r.get(0)).optional()?;
            if owner.is_none() || owner.as_deref() != page.parent_id.as_deref() {
                return Err(invalid("record page is not linked to a live database"));
            }
        }
        if &page.cover != expected {
            return Err(invalid("cover changed in another editor"));
        }
        page.cover = cover;
        page.updated_at = now();
        tx.execute(
            "UPDATE pages SET data=?1 WHERE id=?2",
            params![encode(&page)?, page.id],
        )?;
        tx.commit()?;
        Ok(page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PageKind, new_id};
    #[test]
    fn atomic_cover_updates_preserve_other_page_data_and_reject_stale_changes() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Covers", "").unwrap();
        let other = store.create_workspace("Other", "").unwrap();
        let page = store
            .create_page(&workspace.id, None, "Title", "", PageKind::Document)
            .unwrap();
        let color = Some(PageCover::Color {
            color: "#123456".into(),
        });
        let saved = store
            .set_page_cover_if_unchanged(&workspace.id, &page.id, &None, color.clone())
            .unwrap();
        assert_eq!(saved.title, "Title");
        assert!(
            store
                .set_page_cover_if_unchanged(&workspace.id, &page.id, &None, None)
                .is_err()
        );
        assert!(
            store
                .set_page_cover_if_unchanged(&other.id, &page.id, &color, None)
                .is_err()
        );
        assert!(
            store
                .set_page_cover_if_unchanged(
                    &workspace.id,
                    &page.id,
                    &color,
                    Some(PageCover::Gradient {
                        start: "bad".into(),
                        end: "#ffffff".into()
                    })
                )
                .is_err()
        );
        assert_eq!(store.get_page(&page.id).unwrap().unwrap().cover, color);
        assert!(
            store
                .set_page_cover_if_unchanged(&workspace.id, &page.id, &color, None)
                .is_ok()
        );
    }
    #[test]
    fn cover_images_round_trip_with_archive_and_are_content_addressed() {
        let path = std::env::temp_dir().join(format!("moss-cover-{}", new_id()));
        std::fs::create_dir_all(&path).unwrap();
        let mut store = Store::open(path.join("workspace.sqlite3")).unwrap();
        let workspace = store.create_workspace("Covers", "").unwrap();
        let page = store
            .create_page(&workspace.id, None, "Title", "", PageKind::Document)
            .unwrap();
        let mut output = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut output, image::ImageFormat::Png)
            .unwrap();
        let png = output.into_inner();
        let digest = store.import_cover_image(&png).unwrap();
        assert_eq!(store.import_cover_image(&png).unwrap(), digest);
        assert!(store.import_cover_image(b"not an image").is_err());
        let cover = Some(PageCover::Image {
            asset: digest.clone(),
            position: 0.4,
        });
        store
            .set_page_cover_if_unchanged(&workspace.id, &page.id, &None, cover.clone())
            .unwrap();
        let archive = store.export_workspace(&workspace.id).unwrap();
        let destination = path.join("destination");
        std::fs::create_dir_all(&destination).unwrap();
        let mut imported = Store::open(destination.join("workspace.sqlite3")).unwrap();
        let new_workspace = imported.import_workspace(&archive).unwrap();
        let pages = imported.list_pages(&new_workspace.id).unwrap();
        assert_eq!(pages[0].cover, cover);
        assert_eq!(imported.read_cover_image(&digest).unwrap(), png);
        drop(imported);
        drop(store);
        let _ = std::fs::remove_dir_all(path);
    }
}
