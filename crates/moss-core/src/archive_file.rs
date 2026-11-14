use super::{
    ArchiveError, FORMAT, MAX_ARCHIVE_BYTES, Payload, Store, asset_ids, invalid,
    validate_with_assets,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

const MAGIC: &[u8; 8] = b"MOSSBIN2";
const MAX_METADATA: u64 = 256 * 1024 * 1024;
const MAX_ASSET: u64 = 12 * 1024 * 1024;
const MAX_TOTAL: u64 = 16 * 1024 * 1024 * 1024;
const MAX_ASSETS: usize = 100_000;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    version: u32,
    payload: Payload,
    assets: Vec<String>,
}

struct MetadataWriter<const LIMIT: usize>(Vec<u8>);
impl<const LIMIT: usize> Write for MetadataWriter<LIMIT> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > LIMIT.saturating_sub(self.0.len()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "archive metadata exceeds size limit",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn check_cancelled(cancel: &AtomicBool) -> Result<(), ArchiveError> {
    if cancel.load(Ordering::Relaxed) {
        Err(invalid("archive transfer cancelled"))
    } else {
        Ok(())
    }
}
fn read_u64<R: Read>(reader: &mut R) -> Result<u64, ArchiveError> {
    let mut data = [0; 8];
    reader.read_exact(&mut data)?;
    Ok(u64::from_le_bytes(data))
}
fn temporary_path(path: &Path) -> Result<PathBuf, ArchiveError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| invalid("archive path has no filename"))?
        .to_string_lossy();
    Ok(parent.join(format!(".{name}.{}.tmp", crate::new_id())))
}
fn asset_refs(manifest: &Manifest) -> Result<(), ArchiveError> {
    if manifest.format != FORMAT
        || manifest.version != 2
        || !manifest.payload.cover_assets.is_empty()
    {
        return Err(invalid("unsupported binary archive version"));
    }
    if manifest.assets.len() > MAX_ASSETS || asset_ids(&manifest.payload)? != manifest.assets {
        return Err(invalid("invalid asset manifest"));
    }
    let ids: HashSet<&str> = manifest.assets.iter().map(String::as_str).collect();
    validate_with_assets(&manifest.payload, &ids)
}
fn scan_assets<R: Read>(
    reader: &mut R,
    assets: &[String],
    mut save: impl FnMut(&str, &[u8]) -> Result<(), ArchiveError>,
) -> Result<(), ArchiveError> {
    let mut total = 0u64;
    for digest in assets {
        let size = read_u64(reader)?;
        total = total
            .checked_add(size)
            .ok_or_else(|| invalid("asset size overflow"))?;
        if size == 0 || size > MAX_ASSET || total > MAX_TOTAL {
            return Err(invalid("archive asset exceeds size limit"));
        }
        let mut data = vec![0; size as usize];
        reader.read_exact(&mut data)?;
        if !crate::page_covers::validate_image(&data)
            || format!("{:x}", Sha256::digest(&data)) != *digest
        {
            return Err(invalid("archive image checksum mismatch"));
        }
        save(digest, &data)?;
    }
    let mut trailing = [0];
    if reader.read(&mut trailing)? != 0 {
        return Err(invalid("unexpected trailing archive data"));
    }
    Ok(())
}

impl Store {
    /// Write a version 2 archive without expanding images into JSON arrays.
    /// The temporary file is renamed only after the complete archive is saved.
    pub fn export_workspace_to_file(
        &self,
        workspace_id: &str,
        path: impl AsRef<Path>,
    ) -> Result<(), ArchiveError> {
        self.export_workspace_to_file_cancellable(workspace_id, path, &AtomicBool::new(false))
    }
    /// Cancel an export before it replaces the destination.
    pub fn export_workspace_to_file_cancellable(
        &self,
        workspace_id: &str,
        path: impl AsRef<Path>,
        cancel: &AtomicBool,
    ) -> Result<(), ArchiveError> {
        check_cancelled(cancel)?;
        let target = path.as_ref();
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let canonical = parent.canonicalize()?.join(
            target
                .file_name()
                .ok_or_else(|| invalid("archive path has no filename"))?,
        );
        if let Some(db) = self.conn.path() {
            let db = Path::new(db).canonicalize()?;
            if canonical.starts_with(db.parent().unwrap_or(Path::new("."))) {
                return Err(invalid("archive destination is managed workspace data"));
            }
        }
        let tx = self.conn.unchecked_transaction()?;
        let (payload, assets) = self.collect_archive_payload(workspace_id)?;
        tx.commit()?;
        let manifest = Manifest {
            format: FORMAT.into(),
            version: 2,
            payload,
            assets,
        };
        asset_refs(&manifest)?;
        let mut metadata = MetadataWriter::<{ MAX_METADATA as usize }>(Vec::new());
        serde_json::to_writer(&mut metadata, &manifest)?;
        let metadata = metadata.0;
        let temporary = temporary_path(path.as_ref())?;
        let result = (|| -> Result<(), ArchiveError> {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut writer = BufWriter::new(options.open(&temporary)?);
            writer.write_all(MAGIC)?;
            writer.write_all(&(metadata.len() as u64).to_le_bytes())?;
            writer.write_all(&Sha256::digest(&metadata))?;
            writer.write_all(&metadata)?;
            let mut total = 0u64;
            for digest in &manifest.assets {
                check_cancelled(cancel)?;
                let bytes = self.read_cover_image(digest)?;
                total = total
                    .checked_add(bytes.len() as u64)
                    .ok_or_else(|| invalid("asset size overflow"))?;
                if total > MAX_TOTAL {
                    return Err(invalid("archive assets exceed size limit"));
                }
                writer.write_all(&(bytes.len() as u64).to_le_bytes())?;
                writer.write_all(&bytes)?;
            }
            writer.flush()?;
            writer.get_ref().sync_all()?;
            check_cancelled(cancel)?;
            fs::rename(&temporary, path.as_ref())?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    /// Restore a version 2 archive or a legacy version 1 JSON archive.
    pub fn import_workspace_from_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<crate::Workspace, ArchiveError> {
        self.import_workspace_from_file_cancellable(path, &AtomicBool::new(false))
    }
    /// Cancel a restore before the workspace transaction starts.
    pub fn import_workspace_from_file_cancellable(
        &mut self,
        path: impl AsRef<Path>,
        cancel: &AtomicBool,
    ) -> Result<crate::Workspace, ArchiveError> {
        check_cancelled(cancel)?;
        let mut reader = BufReader::new(File::open(path)?);
        let mut magic = [0; 8];
        let is_binary = match reader.read_exact(&mut magic) {
            Ok(()) => &magic == MAGIC,
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => false,
            Err(error) => return Err(error.into()),
        };
        if !is_binary {
            reader.seek(SeekFrom::Start(0))?;
            let mut bytes = Vec::new();
            reader
                .take((MAX_ARCHIVE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)?;
            if bytes.len() > MAX_ARCHIVE_BYTES {
                return Err(invalid("archive exceeds size limit"));
            }
            let text =
                std::str::from_utf8(&bytes).map_err(|_| invalid("legacy archive is not UTF-8"))?;
            check_cancelled(cancel)?;
            return self.import_workspace(text);
        }
        let size = read_u64(&mut reader)?;
        if size == 0 || size > MAX_METADATA {
            return Err(invalid("archive metadata exceeds size limit"));
        }
        let file_len = reader.get_ref().metadata()?.len();
        if size > file_len.saturating_sub(48) {
            return Err(invalid("truncated archive metadata"));
        }
        let mut expected = [0; 32];
        reader.read_exact(&mut expected)?;
        let mut metadata = vec![0; size as usize];
        reader.read_exact(&mut metadata)?;
        if Sha256::digest(&metadata).as_slice() != expected {
            return Err(invalid("archive metadata checksum mismatch"));
        }
        let manifest: Manifest = serde_json::from_slice(&metadata)?;
        asset_refs(&manifest)?;
        let asset_start = reader.stream_position()?;
        scan_assets(&mut reader, &manifest.assets, |_, _| {
            check_cancelled(cancel)
        })?;
        reader.seek(SeekFrom::Start(asset_start))?;
        scan_assets(&mut reader, &manifest.assets, |expected, bytes| {
            check_cancelled(cancel)?;
            let digest = crate::page_covers::save_asset(&self.conn, bytes)?;
            if digest != expected {
                return Err(invalid("archive image checksum mismatch"));
            }
            Ok(())
        })?;
        check_cancelled(cancel)?;
        self.import_archive_payload(manifest.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Block, BlockKind, PageKind, new_id};
    use serde_json::json;

    fn dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("moss-archive-file-{}", new_id()));
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn image(seed: u32) -> Vec<u8> {
        let mut n = seed;
        let mut bytes = vec![0; 1024 * 1024 * 3];
        for b in &mut bytes {
            n ^= n << 13;
            n ^= n >> 17;
            n ^= n << 5;
            *b = n as u8;
        }
        let image = image::RgbImage::from_raw(1024, 1024, bytes).unwrap();
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }
    #[test]
    fn metadata_writer_rejects_data_over_its_limit() {
        let mut writer = MetadataWriter::<4>(Vec::new());
        writer.write_all(b"1234").unwrap();
        assert!(writer.write_all(b"5").is_err());
        assert_eq!(writer.0, b"1234");
    }
    #[test]
    fn binary_streams_many_images_and_restores_legacy_json() {
        let root = dir();
        fs::create_dir(root.join("src")).unwrap();
        fs::create_dir(root.join("dst")).unwrap();
        let mut source = Store::open(root.join("src/db.sqlite3")).unwrap();
        let workspace = source.create_workspace("Images", "").unwrap();
        let page = source
            .create_page(&workspace.id, None, "Photos", "", PageKind::Document)
            .unwrap();
        let mut blocks = Vec::new();
        let mut expected = Vec::new();
        let mut legacy_size = 0;
        for i in 1..=7 {
            let data = image(i);
            legacy_size += serde_json::to_vec(&data).unwrap().len();
            let digest = source.import_cover_image(&data).unwrap();
            let mut block = Block::paragraph(&page.id, "", i as i64);
            block.kind = BlockKind::Image;
            block.config = json!({"asset":digest});
            blocks.push(block);
            expected.push(data);
        }
        assert!(legacy_size > MAX_ARCHIVE_BYTES);
        source.save_blocks(&page.id, &blocks).unwrap();
        let file = root.join("workspace.moss");
        source
            .export_workspace_to_file(&workspace.id, &file)
            .unwrap();
        assert!(fs::metadata(&file).unwrap().len() < legacy_size as u64);
        let mut target = Store::open(root.join("dst/db.sqlite3")).unwrap();
        let restored = target.import_workspace_from_file(&file).unwrap();
        let page = target.list_pages(&restored.id).unwrap().remove(0);
        let blocks = target.get_blocks(&page.id).unwrap();
        assert_eq!(blocks.len(), expected.len());
        for (block, data) in blocks.iter().zip(expected) {
            assert_eq!(
                target
                    .read_cover_image(block.config["asset"].as_str().unwrap())
                    .unwrap(),
                data
            );
        }
        let empty = source.create_workspace("Legacy", "").unwrap();
        let legacy = root.join("legacy.moss");
        fs::write(&legacy, source.export_workspace(&empty.id).unwrap()).unwrap();
        assert_eq!(
            target.import_workspace_from_file(&legacy).unwrap().name,
            "Legacy"
        );
        drop(target);
        drop(source);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn corrupted_data_and_failed_export_leave_existing_data_intact() {
        let root = dir();
        fs::create_dir(root.join("src")).unwrap();
        fs::create_dir(root.join("dst")).unwrap();
        let mut source = Store::open(root.join("src/db.sqlite3")).unwrap();
        let ws = source.create_workspace("Images", "").unwrap();
        let page = source
            .create_page(&ws.id, None, "Page", "", PageKind::Document)
            .unwrap();
        let data = image(55);
        let digest = source.import_cover_image(&data).unwrap();
        let mut block = Block::paragraph(&page.id, "", 0);
        block.kind = BlockKind::Image;
        block.config = json!({"asset":digest});
        source.save_blocks(&page.id, &[block.clone()]).unwrap();
        let path = root.join("workspace.moss");
        source.export_workspace_to_file(&ws.id, &path).unwrap();
        let original = fs::read(&path).unwrap();
        let meta_size = u64::from_le_bytes(original[8..16].try_into().unwrap()) as usize;
        let asset_start = 48 + meta_size;
        let expected = [block.config["asset"].as_str().unwrap().to_owned()];
        scan_assets(
            &mut std::io::Cursor::new(&original[asset_start..]),
            &expected,
            |_, _| Ok(()),
        )
        .unwrap();
        let mut bytes = original.clone();
        *bytes.last_mut().unwrap() ^= 0x01;
        let mut saved = false;
        assert!(
            scan_assets(
                &mut std::io::Cursor::new(&bytes[asset_start..]),
                &expected,
                |_, _| {
                    saved = true;
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!saved);
        fs::write(&path, &bytes).unwrap();
        let mut target = Store::open(root.join("dst/db.sqlite3")).unwrap();
        assert!(target.import_workspace_from_file(&path).is_err());
        assert!(target.list_workspaces().unwrap().is_empty());
        assert!(!root.join("dst/cover-assets").exists());
        let mut metadata_corrupt = original.clone();
        metadata_corrupt[48] ^= 1;
        fs::write(&path, metadata_corrupt).unwrap();
        assert!(target.import_workspace_from_file(&path).is_err());
        let mut too_large = original;
        too_large[8..16].copy_from_slice(&(MAX_METADATA + 1).to_le_bytes());
        fs::write(&path, too_large).unwrap();
        assert!(target.import_workspace_from_file(&path).is_err());
        assert!(target.list_workspaces().unwrap().is_empty());
        fs::write(&path, b"existing archive").unwrap();
        block.config = json!({"asset":"a".repeat(64)});
        source.save_blocks(&page.id, &[block]).unwrap();
        assert!(source.export_workspace_to_file(&ws.id, &path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"existing archive");
        let cancel = AtomicBool::new(true);
        assert!(
            source
                .export_workspace_to_file_cancellable(&ws.id, &path, &cancel)
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), b"existing archive");
        assert!(
            target
                .import_workspace_from_file_cancellable(&path, &cancel)
                .is_err()
        );
        assert!(target.list_workspaces().unwrap().is_empty());
        assert!(
            source
                .export_workspace_to_file(&ws.id, root.join("src/db.sqlite3"))
                .is_err()
        );
        let prefs = root.join("src/preferences.json");
        fs::write(&prefs, b"keep preferences").unwrap();
        assert!(source.export_workspace_to_file(&ws.id, &prefs).is_err());
        assert_eq!(fs::read(&prefs).unwrap(), b"keep preferences");
        let wal = root.join("src/db.sqlite3-wal");
        assert!(source.export_workspace_to_file(&ws.id, &wal).is_err());
        drop(target);
        drop(source);
        fs::remove_dir_all(root).unwrap();
    }
}
