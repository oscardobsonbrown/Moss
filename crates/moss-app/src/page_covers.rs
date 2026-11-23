use crate::state::State;
use moss_core::{Page, PageCover, Store};
use rune::{Image, ImageCache, TextEdit, Tx, Waker};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::mpsc,
};

pub const DEFAULT_GRADIENT: (&str, &str) = ("#acc7df", "#efd3c2");

pub struct CoverEditor {
    pub page_id: String,
    pub generation: u64,
    pub anchor: String,
    pub expected: Option<PageCover>,
    pub draft: Option<PageCover>,
    pub reposition: bool,
    pub gradient_picker: bool,
    pub gradient_start: TextEdit,
    pub gradient_end: TextEdit,
    pub gradient_error: Option<String>,
    pub error: Option<String>,
}
#[derive(Default)]
pub struct CoverState {
    pub editor: Option<CoverEditor>,
    pub images: BTreeMap<String, Image>,
    pub loading: bool,
    pub widths: BTreeMap<String, f32>,
    pub image_errors: BTreeMap<String, String>,
    pub gallery_covers: BTreeMap<String, (String, [f32; 4])>,
    pub gallery_viewport: [f32; 4],
    editor_generation: u64,
    image_tx: Option<mpsc::SyncSender<(String, PathBuf)>>,
    cache: Option<ImageCache>,
    wake: Option<Waker>,
    pending_images: BTreeSet<String>,
    ready_images: BTreeSet<String>,
    result_tx: Option<mpsc::Sender<CoverResult>>,
    result_rx: Option<mpsc::Receiver<CoverResult>>,
}
enum CoverResult {
    Loaded(String, Result<Vec<u8>, String>),
    Uploaded(String, u64, Option<PageCover>, Result<String, String>),
    BlockUploaded(
        String,
        String,
        Option<String>,
        Result<moss_core::Block, String>,
    ),
}
enum BlockImageSource {
    File(PathBuf),
    Bytes(Vec<u8>),
}
impl CoverState {
    pub fn set_waker(&mut self, wake: Waker) {
        self.cache = Some(ImageCache::new(wake.clone()));
        self.wake = Some(wake.clone());
        let (tx, rx) = mpsc::channel();
        let (image_tx, image_rx) = mpsc::sync_channel::<(String, PathBuf)>(2);
        let results = tx.clone();
        std::thread::spawn(move || {
            while let Ok((asset, db)) = image_rx.recv() {
                let result = Store::open_read_only(db)
                    .and_then(|store| store.read_cover_image(&asset))
                    .map_err(|e| e.to_string());
                if results.send(CoverResult::Loaded(asset, result)).is_err() {
                    break;
                }
                wake();
            }
        });
        self.image_tx = Some(image_tx);
        self.result_tx = Some(tx);
        self.result_rx = Some(rx);
    }
}
impl State {
    /// Close a saved editor or cancel a position draft before navigation.
    /// Keep a failed save visible until the user retries or discards it.
    pub fn finish_page_cover(&mut self) -> bool {
        if self
            .page_cover
            .editor
            .as_ref()
            .is_some_and(|editor| editor.error.is_some())
        {
            return false;
        }
        self.page_cover.editor = None;
        true
    }
    pub fn open_page_cover(&mut self, page: &Page, anchor: &str) {
        if self
            .page_cover
            .editor
            .as_ref()
            .is_some_and(|e| e.error.is_some() || e.reposition)
        {
            return;
        }
        let (start, end) = match &page.cover {
            Some(PageCover::Gradient { start, end }) => (start.as_str(), end.as_str()),
            _ => DEFAULT_GRADIENT,
        };
        self.page_cover.editor_generation = self.page_cover.editor_generation.wrapping_add(1);
        self.page_cover.editor = Some(CoverEditor {
            generation: self.page_cover.editor_generation,
            page_id: page.id.clone(),
            anchor: anchor.into(),
            expected: page.cover.clone(),
            draft: page.cover.clone(),
            reposition: false,
            gradient_picker: false,
            gradient_start: TextEdit::new(start),
            gradient_end: TextEdit::new(end),
            gradient_error: None,
            error: None,
        });
    }
    pub fn open_gradient_picker(&mut self) {
        let Some(editor) = &mut self.page_cover.editor else {
            return;
        };
        editor.gradient_picker = true;
        editor.gradient_error = None;
        if let Some(PageCover::Gradient { start, end }) = editor.draft.clone() {
            editor.gradient_start = TextEdit::new(&start);
            editor.gradient_end = TextEdit::new(&end);
        } else {
            let (start, end) = DEFAULT_GRADIENT;
            editor.gradient_start = TextEdit::new(start);
            editor.gradient_end = TextEdit::new(end);
            self.set_cover_draft(Some(PageCover::Gradient {
                start: start.into(),
                end: end.into(),
            }));
        }
    }
    pub fn set_gradient_colors(&mut self, start: &str, end: &str) {
        let start = start.to_ascii_lowercase();
        let end = end.to_ascii_lowercase();
        if let Some(editor) = &mut self.page_cover.editor {
            editor.gradient_start.replace_text(&start);
            editor.gradient_end.replace_text(&end);
            editor.gradient_error = None;
        }
        self.set_cover_draft(Some(PageCover::Gradient { start, end }));
    }
    pub fn set_gradient_stop(&mut self, is_start: bool, hex: &str) {
        let (start, end) = {
            let Some(editor) = &mut self.page_cover.editor else {
                return;
            };
            let current = match &editor.draft {
                Some(PageCover::Gradient { start, end }) => (start.clone(), end.clone()),
                _ => (DEFAULT_GRADIENT.0.into(), DEFAULT_GRADIENT.1.into()),
            };
            let next = if is_start {
                (hex.to_ascii_lowercase(), current.1)
            } else {
                (current.0, hex.to_ascii_lowercase())
            };
            editor.gradient_start.replace_text(&next.0);
            editor.gradient_end.replace_text(&next.1);
            editor.gradient_error = None;
            next
        };
        self.set_cover_draft(Some(PageCover::Gradient { start, end }));
    }
    pub fn swap_gradient_colors(&mut self) {
        let next = {
            let Some(editor) = &mut self.page_cover.editor else {
                return;
            };
            let (start, end) = match &editor.draft {
                Some(PageCover::Gradient { start, end }) => (end.clone(), start.clone()),
                _ => (DEFAULT_GRADIENT.1.into(), DEFAULT_GRADIENT.0.into()),
            };
            editor.gradient_start.replace_text(&start);
            editor.gradient_end.replace_text(&end);
            editor.gradient_error = None;
            (start, end)
        };
        self.set_cover_draft(Some(PageCover::Gradient {
            start: next.0,
            end: next.1,
        }));
    }
    pub fn apply_gradient_inputs(&mut self, strict: bool) {
        fn normalize_hex(raw: &str) -> Option<String> {
            let trimmed = raw.trim();
            let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
            if digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()) {
                Some(format!("#{}", digits.to_ascii_lowercase()))
            } else {
                None
            }
        }
        let parsed = {
            let Some(editor) = &mut self.page_cover.editor else {
                return;
            };
            match (
                normalize_hex(&editor.gradient_start.text),
                normalize_hex(&editor.gradient_end.text),
            ) {
                (Some(start), Some(end)) => {
                    editor.gradient_error = None;
                    Some((start, end))
                }
                _ => {
                    if strict {
                        editor.gradient_error = Some("Enter hex colors like #acc7df.".into());
                    }
                    None
                }
            }
        };
        if let Some((start, end)) = parsed {
            self.set_cover_draft(Some(PageCover::Gradient { start, end }));
        }
    }
    pub fn set_cover_draft(&mut self, cover: Option<PageCover>) {
        if let Some(editor) = &mut self.page_cover.editor {
            editor.draft = cover;
            editor.error = None;
        }
        self.save_cover_draft();
    }
    pub fn save_cover_draft(&mut self) {
        let Some(editor) = &self.page_cover.editor else {
            return;
        };
        let result = self.store.set_page_cover_if_unchanged(
            &self.workspace.id,
            &editor.page_id,
            &editor.expected,
            editor.draft.clone(),
        );
        match result {
            Ok(page) => {
                let peek_record = self
                    .record_peek
                    .as_ref()
                    .filter(|peek| peek.page.id == page.id)
                    .map(|peek| peek.record.id.clone());
                for tab in &mut self.tabs {
                    if tab.page.id == page.id {
                        tab.page.cover = page.cover.clone();
                        tab.page.updated_at = page.updated_at.clone();
                    }
                    for record in &tab.records {
                        if record.page_id.as_deref() == Some(page.id.as_str())
                            || peek_record.as_deref() == Some(record.id.as_str())
                        {
                            if let Some(cover) = &page.cover {
                                tab.gallery_covers.insert(record.id.clone(), cover.clone());
                            } else {
                                tab.gallery_covers.remove(&record.id);
                            }
                        }
                    }
                }
                if let Some(peek) = &mut self.record_peek {
                    if peek.page.id == page.id {
                        peek.page.cover = page.cover.clone();
                        peek.page.updated_at = page.updated_at.clone();
                    }
                }
                if let Some(editor) = &mut self.page_cover.editor {
                    editor.expected = page.cover;
                    editor.error = None;
                    editor.reposition = false;
                }
            }
            Err(error) => {
                if let Some(editor) = &mut self.page_cover.editor {
                    editor.error = Some(format!("Cover is not saved: {error}"));
                }
            }
        }
    }
    pub fn discard_cover_draft(&mut self) {
        let Some(editor) = &self.page_cover.editor else {
            return;
        };
        let id = editor.page_id.clone();
        match self.store.get_page(&id) {
            Ok(Some(page))
                if page.workspace_id == self.workspace.id && page.deleted_at.is_none() =>
            {
                for tab in &mut self.tabs {
                    if tab.page.id == id {
                        tab.page.cover = page.cover.clone();
                    }
                }
                if let Some(peek) = &mut self.record_peek {
                    if peek.page.id == id {
                        peek.page.cover = page.cover.clone();
                    }
                }
                self.page_cover.editor = None;
            }
            Ok(_) => {
                self.page_cover.editor.as_mut().unwrap().error = Some("Page was not found.".into())
            }
            Err(error) => {
                self.page_cover.editor.as_mut().unwrap().error =
                    Some(format!("Cover is not loaded: {error}"))
            }
        }
    }
    fn spawn_block_image_import(
        &mut self,
        page_id: &str,
        block_id: &str,
        expected: Option<String>,
        source: BlockImageSource,
    ) {
        let Some(sender) = self.page_cover.result_tx.clone() else {
            self.error = Some("Image worker is unavailable.".into());
            return;
        };
        let (workspace, page_id, block_id) = (
            self.workspace.id.clone(),
            page_id.to_owned(),
            block_id.to_owned(),
        );
        let db = self.path.join("workspace.sqlite3");
        let wake = self.page_cover.wake.clone();
        std::thread::spawn(move || {
            let result = match source {
                BlockImageSource::File(path) => Store::import_block_image_file(
                    &db,
                    &workspace,
                    &page_id,
                    &block_id,
                    expected.as_deref(),
                    &path,
                ),
                BlockImageSource::Bytes(bytes) => Store::import_block_image_bytes(
                    &db,
                    &workspace,
                    &page_id,
                    &block_id,
                    expected.as_deref(),
                    &bytes,
                ),
            }
            .map_err(|e| e.to_string());
            let _ = sender.send(CoverResult::BlockUploaded(
                page_id, block_id, expected, result,
            ));
            if let Some(wake) = wake {
                wake();
            }
        });
    }

    pub fn upload_block_image(
        &mut self,
        page_id: &str,
        block_id: &str,
        expected: Option<String>,
        path: PathBuf,
    ) {
        self.spawn_block_image_import(page_id, block_id, expected, BlockImageSource::File(path));
    }

    fn insert_editor_images(
        &mut self,
        page_id: &str,
        peek: bool,
        anchor: Option<(&str, crate::editor_drag::Side)>,
        sources: Vec<BlockImageSource>,
    ) -> bool {
        if sources.is_empty() {
            return false;
        }
        let Some(blocks) = self.editor_blocks_mut_for_page(page_id, peek) else {
            return false;
        };
        let is_reusable = |b: &crate::state::EditableBlock| {
            (b.block.kind == moss_core::BlockKind::Paragraph && b.edit.text.is_empty())
                || (b.block.kind == moss_core::BlockKind::Image
                    && b.block.config.get("asset").is_none())
        };
        let (mut at, reuse_first, col_source) = match anchor
            .and_then(|(id, side)| blocks.iter().position(|b| b.block.id == id).map(|i| (i, side)))
        {
            Some((index, side)) => {
                let col_block = blocks[index].block.clone();
                if is_reusable(&blocks[index]) {
                    (index, true, Some(col_block))
                } else {
                    (
                        index + usize::from(side == crate::editor_drag::Side::After),
                        false,
                        Some(col_block),
                    )
                }
            }
            None => {
                let reuse = blocks.last().is_some_and(is_reusable);
                (if reuse { blocks.len() - 1 } else { blocks.len() }, reuse, None)
            }
        };
        for b in blocks.iter_mut() {
            b.edit.focused = false;
        }
        let mut uploads = Vec::with_capacity(sources.len());
        for (idx, source) in sources.into_iter().enumerate() {
            let block_id = if idx == 0 && reuse_first {
                blocks[at].block.kind = moss_core::BlockKind::Image;
                blocks[at].block.checked = false;
                blocks[at].block.id.clone()
            } else {
                let mut block = moss_core::Block::paragraph(page_id, "", at as i64);
                block.kind = moss_core::BlockKind::Image;
                if let Some(src) = &col_source {
                    crate::editor_drag::copy_column(src, &mut block);
                }
                let id = block.id.clone();
                blocks.insert(
                    at,
                    crate::state::EditableBlock {
                        block,
                        edit: TextEdit::new("").rich(),
                    },
                );
                id
            };
            at += 1;
            uploads.push((block_id, source));
        }
        self.save_editor_blocks(peek);
        if !self.editor_blocks_dirty(peek) {
            for (block_id, source) in uploads {
                self.spawn_block_image_import(page_id, &block_id, None, source);
            }
        }
        true
    }

    pub fn drop_editor_image_files(
        &mut self,
        page_id: &str,
        peek: bool,
        files: &[PathBuf],
        point: Option<[f32; 2]>,
        zones: &[crate::editor_drag::DropZone],
    ) {
        self.editor_file_drop = None;
        let target = point
            .and_then(|p| crate::editor_drag::file_drop_target(p, zones))
            .filter(|t| t.id != "@end");
        let anchor = target.as_ref().map(|t| (t.id.as_str(), t.side));
        let sources = files.iter().cloned().map(BlockImageSource::File).collect();
        self.insert_editor_images(page_id, peek, anchor, sources);
    }

    pub fn paste_editor_image_content(
        &mut self,
        block_id: Option<&str>,
        peek: bool,
        content: &rune::ClipboardContent,
    ) -> bool {
        let sources: Vec<BlockImageSource> = if let Some(image) = content.image() {
            vec![BlockImageSource::Bytes(image.png().to_vec())]
        } else {
            content
                .files()
                .iter()
                .filter(|path| {
                    path.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| {
                            matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp")
                        })
                })
                .cloned()
                .map(BlockImageSource::File)
                .collect()
        };
        if sources.is_empty() {
            return false;
        }
        let Some((page_id, _, blocks, _)) = self.editor_surface(peek) else {
            return false;
        };
        let page_id = page_id.to_owned();
        let anchor_id = block_id
            .map(str::to_owned)
            .or_else(|| blocks.iter().find(|b| b.edit.focused).map(|b| b.block.id.clone()));
        let anchor = anchor_id
            .as_deref()
            .map(|id| (id, crate::editor_drag::Side::After));
        self.insert_editor_images(&page_id, peek, anchor, sources)
    }
    pub fn retry_cover_image(&mut self, asset: &str) {
        self.page_cover.image_errors.remove(asset);
    }
    pub fn upload_page_cover(&mut self, page_id: &str, generation: u64, path: PathBuf) {
        if !self
            .page_cover
            .editor
            .as_ref()
            .is_some_and(|e| e.page_id == page_id && e.generation == generation)
        {
            return;
        }
        if self.page_cover.loading {
            return;
        }
        let Some(editor) = &self.page_cover.editor else {
            return;
        };
        let Some(sender) = self.page_cover.result_tx.clone() else {
            self.page_cover.editor.as_mut().unwrap().error =
                Some("Image worker is unavailable.".into());
            return;
        };
        let page_id = editor.page_id.clone();
        let expected = editor.expected.clone();
        let db = self.path.join("workspace.sqlite3");
        let wake = self.page_cover.wake.clone();
        self.page_cover.loading = true;
        std::thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
                if meta.len() > 12 * 1024 * 1024 {
                    return Err("Image exceeds 12 MiB.".into());
                }
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(path)
                    .map_err(|e| e.to_string())?
                    .take(12 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                Store::open_read_only(db)
                    .map_err(|e| e.to_string())?
                    .import_cover_image(&bytes)
                    .map_err(|e| e.to_string())
            })();
            let _ = sender.send(CoverResult::Uploaded(page_id, generation, expected, result));
            if let Some(w) = wake {
                w();
            }
        });
    }
}
/// Reconcile only image assets; keep the editor's current text and other settings.
pub(crate) fn sync_saved_image_assets(
    editable: &mut [crate::state::EditableBlock],
    saved: &[moss_core::Block],
) {
    let assets: std::collections::HashMap<_, _> = saved
        .iter()
        .filter(|block| block.kind == moss_core::BlockKind::Image)
        .map(|block| (block.id.as_str(), block.config.get("asset")))
        .collect();
    for block in editable
        .iter_mut()
        .filter(|b| b.block.kind == moss_core::BlockKind::Image)
    {
        let Some(asset) = assets.get(block.block.id.as_str()) else {
            continue;
        };
        if block.block.config.is_null() {
            block.block.config = serde_json::json!({});
        }
        if let Some(config) = block.block.config.as_object_mut() {
            match asset {
                Some(asset) => {
                    config.insert("asset".into(), (*asset).clone());
                }
                None => {
                    config.remove("asset");
                }
            }
        }
    }
}
pub fn poll(s: &mut State, _time: f64, _tx: &mut Tx) -> bool {
    let mut wanted = Vec::new();
    if let Some(tab) = s.tab() {
        if let Some(PageCover::Image { asset, .. }) = &tab.page.cover {
            wanted.push(asset.clone());
        }
    }
    if let Some(tab) = s.tab() {
        wanted.extend(
            tab.blocks
                .iter()
                .filter(|b| b.block.kind == moss_core::BlockKind::Image)
                .filter_map(|b| {
                    b.block
                        .config
                        .get("asset")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                }),
        );
    }
    if let Some(peek) = &s.record_peek {
        if let Some(PageCover::Image { asset, .. }) = &peek.page.cover {
            wanted.push(asset.clone());
        }
        wanted.extend(
            peek.blocks
                .iter()
                .filter(|b| b.block.kind == moss_core::BlockKind::Image)
                .filter_map(|b| {
                    b.block
                        .config
                        .get("asset")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                }),
        );
    }
    if let Some(tab) = s.tab() {
        if let Some(view) = tab
            .database
            .as_ref()
            .and_then(|db| db.views.get(tab.view))
            .filter(|view| view.kind == moss_core::ViewKind::Gallery)
        {
            let viewport = s.page_cover.gallery_viewport;
            wanted.extend(s.page_cover.gallery_covers.iter().filter(|(id, (asset, rect))| {
                let preview = view.config.get("gallery_preview").and_then(serde_json::Value::as_str);
                let shows_cover = preview == Some("page_cover") || (matches!(preview, None | Some("page_content")) && tab.gallery_previews.get(*id).is_none_or(|text| text.trim().is_empty()));
                shows_cover && matches!(tab.gallery_covers.get(*id), Some(PageCover::Image { asset: current, .. }) if current == asset)
                    && rect[0] < viewport[0] + viewport[2] && rect[0] + rect[2] > viewport[0]
                    && rect[1] < viewport[1] + viewport[3] && rect[1] + rect[3] > viewport[1]
            }).take(12).map(|(_, (asset, _))| asset.clone()));
        }
    }
    let current_gallery_ids: BTreeSet<_> = s
        .tab()
        .map(|tab| tab.gallery_covers.keys().cloned().collect())
        .unwrap_or_default();
    s.page_cover
        .gallery_covers
        .retain(|id, _| current_gallery_ids.contains(id));
    if let Some((db, view)) = s
        .tab()
        .and_then(|tab| Some((tab.database.as_ref()?, tab.view)))
    {
        if db.views.get(view).is_some_and(|view| {
            view.kind == moss_core::ViewKind::Board
                && view
                    .config
                    .get("card_preview")
                    .and_then(serde_json::Value::as_str)
                    == Some("cover")
        }) {
            if let Some(projection) = s
                .database_views
                .board
                .projection_for(&db.id, &db.views[view].id)
            {
                let viewport = s.database_views.board_ui.viewport;
                wanted.extend(
                    s.database_views
                        .board_ui
                        .covers
                        .iter()
                        .filter(|(id, (_, rect))| {
                            projection.covers.contains_key(*id)
                                && rect[0] < viewport[0] + viewport[2]
                                && rect[0] + rect[2] > viewport[0]
                                && rect[1] < viewport[1] + viewport[3]
                                && rect[1] + rect[3] > viewport[1]
                        })
                        .take(12)
                        .map(|(_, (asset, _))| asset.clone()),
                );
            }
        }
    }
    let keep: BTreeSet<_> = wanted.iter().cloned().collect();
    for asset in wanted {
        if s.page_cover.images.contains_key(&asset)
            || s.page_cover.image_errors.contains_key(&asset)
            || s.page_cover.pending_images.contains(&asset)
        {
            continue;
        }
        let Some(sender) = &s.page_cover.image_tx else {
            continue;
        };
        match sender.try_send((asset.clone(), s.path.join("workspace.sqlite3"))) {
            Ok(()) => {
                s.page_cover.pending_images.insert(asset);
            }
            Err(mpsc::TrySendError::Full(_)) => {}
            Err(mpsc::TrySendError::Disconnected(_)) => {
                s.page_cover
                    .image_errors
                    .insert(asset, "Image worker stopped.".into());
            }
        }
    }
    let mut results = Vec::new();
    if let Some(rx) = &s.page_cover.result_rx {
        while let Ok(result) = rx.try_recv() {
            results.push(result);
        }
    }
    let mut changed = false;
    for result in results {
        changed = true;
        match result {
            CoverResult::Loaded(asset, result) => {
                s.page_cover.pending_images.remove(&asset);
                match result {
                    Ok(bytes) => {
                        if let Some(cache) = &s.page_cover.cache {
                            if s.page_cover.images.len() >= 16 {
                                if let Some(key) = s
                                    .page_cover
                                    .images
                                    .keys()
                                    .find(|key| !keep.contains(*key))
                                    .cloned()
                                {
                                    s.page_cover.images.remove(&key);
                                }
                            }
                            s.page_cover.ready_images.remove(&asset);
                            s.page_cover.images.insert(asset, cache.load(bytes));
                        }
                    }
                    Err(error) => {
                        if s.page_cover.image_errors.len() >= 16 {
                            if let Some(key) = s.page_cover.image_errors.keys().next().cloned() {
                                s.page_cover.image_errors.remove(&key);
                            }
                        }
                        s.page_cover.image_errors.insert(asset, error);
                    }
                }
            }
            CoverResult::BlockUploaded(page_id, block_id, expected, result) => match result {
                Ok(saved) => {
                    let asset = saved.config.get("asset").cloned();
                    let update = |block: &mut crate::state::EditableBlock| {
                        if block.block.kind != moss_core::BlockKind::Image
                            || block
                                .block
                                .config
                                .get("asset")
                                .and_then(serde_json::Value::as_str)
                                != expected.as_deref()
                        {
                            return;
                        }
                        if block.block.config.is_null() {
                            block.block.config = serde_json::json!({});
                        }
                        if let (Some(config), Some(asset)) =
                            (block.block.config.as_object_mut(), asset.as_ref())
                        {
                            config.insert("asset".into(), asset.clone());
                        }
                    };
                    for tab in &mut s.tabs {
                        if tab.page.id == page_id {
                            if let Some(block) =
                                tab.blocks.iter_mut().find(|b| b.block.id == block_id)
                            {
                                update(block);
                            }
                        }
                    }
                    if let Some(peek) = &mut s.record_peek {
                        if peek.page.id == page_id {
                            if let Some(block) =
                                peek.blocks.iter_mut().find(|b| b.block.id == block_id)
                            {
                                update(block);
                            }
                        }
                    }
                }
                Err(error) => s.error = Some(format!("Image is not imported: {error}")),
            },
            CoverResult::Uploaded(page_id, generation, expected, result) => {
                s.page_cover.loading = false;
                if s.page_cover.editor.as_ref().is_some_and(|e| {
                    e.page_id == page_id
                        && e.generation == generation
                        && e.expected == expected
                        && e.draft == expected
                }) {
                    match result {
                        Ok(asset) => s.set_cover_draft(Some(PageCover::Image {
                            asset,
                            position: 0.5,
                        })),
                        Err(error) => {
                            s.page_cover.editor.as_mut().unwrap().error =
                                Some(format!("Image is not imported: {error}"))
                        }
                    }
                }
            }
        }
    }
    for (asset, image) in &s.page_cover.images {
        if image.is_ready() && s.page_cover.ready_images.insert(asset.clone()) {
            changed = true;
        }
    }
    s.page_cover
        .ready_images
        .retain(|asset| s.page_cover.images.contains_key(asset));
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::new_id;
    #[test]
    fn conflicting_cover_edit_keeps_the_visible_draft_and_page_content() {
        let path = std::env::temp_dir().join(format!("moss-cover-app-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let page = state.tab().unwrap().page.clone();
        state.open_page_cover(&page, "cover-anchor");
        let changed = Some(PageCover::Color {
            color: "#123456".into(),
        });
        state
            .store
            .set_page_cover_if_unchanged(&state.workspace.id, &page.id, &None, changed.clone())
            .unwrap();
        let attempted = Some(PageCover::Color {
            color: "#abcdef".into(),
        });
        state.set_cover_draft(attempted.clone());
        let editor = state.page_cover.editor.as_ref().unwrap();
        assert_eq!(editor.draft, attempted);
        assert!(editor.error.is_some());
        assert!(!state.finish_page_cover());
        assert_eq!(
            state.store.get_page(&page.id).unwrap().unwrap().cover,
            changed
        );
        assert_eq!(state.tab().unwrap().page.title, page.title);
        drop(state);
        let _ = std::fs::remove_dir_all(path);
    }
    #[test]
    fn cover_popup_saves_color_and_cancels_image_reposition() {
        use rune::{Env, Runtime};
        let path = std::env::temp_dir().join(format!("moss-cover-ui-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let page = state.tab().unwrap().page.clone();
        let mut rt = Runtime::new(state, crate::ui::view, (1000., 800.), Env::default());
        rt.frame(0.);
        let click = |rt: &mut Runtime<State>, key: &str| {
            let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
            rt.pointer_move(x, y);
            rt.frame(1. / 60.);
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            rt.frame(1. / 60.);
        };
        click(&mut rt, &format!("page-cover-open-{}-document", page.id));
        rt.frame(1. / 60.);
        click(&mut rt, "cover-color-#b8c5d6");
        assert_eq!(
            rt.state.tab().unwrap().page.cover,
            Some(PageCover::Color {
                color: "#b8c5d6".into()
            })
        );
        click(&mut rt, "cover-upload");
        let previous_generation = rt.state.page_cover.editor.as_ref().unwrap().generation;
        assert!(rt.state.finish_page_cover());
        let current = rt.state.tab().unwrap().page.clone();
        rt.state.open_page_cover(&current, "new-anchor");
        assert_ne!(
            rt.state.page_cover.editor.as_ref().unwrap().generation,
            previous_generation
        );
        rt.resolve_file_dialog(Some(path.join("wrong-target.png")))
            .unwrap();
        rt.frame(1. / 60.);
        assert!(!rt.state.page_cover.loading);
        assert!(rt.state.page_cover.editor.as_ref().unwrap().error.is_none());
        rt.state.finish_page_cover();
        rt.update(|s, _| {
            s.open_page_cover(&current, &format!("page-cover-open-{}-document", page.id))
        });
        rt.frame(1. / 60.);
        let png = [
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
            130,
        ];
        let asset = rt.state.store.import_cover_image(&png).unwrap();
        rt.update(|s, _| {
            s.set_cover_draft(Some(PageCover::Image {
                asset: asset.clone(),
                position: 0.5,
            }))
        });
        rt.frame(1. / 60.);
        click(&mut rt, "cover-reposition");
        if let Some(editor) = &mut rt.state.page_cover.editor {
            editor.draft = Some(PageCover::Image {
                asset: asset.clone(),
                position: 0.9,
            });
        }
        rt.frame(1. / 60.);
        click(&mut rt, "cover-position-cancel");
        assert_eq!(
            rt.state.tab().unwrap().page.cover,
            Some(PageCover::Image {
                asset,
                position: 0.5
            })
        );
        assert!(!rt.state.page_cover.editor.as_ref().unwrap().reposition);
        drop(rt);
        let _ = std::fs::remove_dir_all(path);
    }
    #[test]
    fn editor_save_between_worker_commit_and_poll_preserves_the_image_and_other_edits() {
        use rune::{Env, Runtime};
        use std::sync::Arc;
        let path = std::env::temp_dir().join(format!("moss-image-save-race-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let page = state.tab().unwrap().page.id.clone();
        let id = state.tab().unwrap().blocks[0].block.id.clone();
        state.tabs[state.active].blocks[0].block.kind = moss_core::BlockKind::Image;
        state.tabs[state.active].blocks[0].block.config = serde_json::json!({"note":"before"});
        state.save_blocks();
        let png = [
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
            130,
        ];
        let asset = state.store.import_cover_image(&png).unwrap();
        state.page_cover.set_waker(Arc::new(|| {}));
        let saved = state
            .store
            .set_block_image_if_unchanged(&state.workspace.id, &page, &id, None, &asset)
            .unwrap();
        state
            .page_cover
            .result_tx
            .as_ref()
            .unwrap()
            .send(CoverResult::BlockUploaded(
                page.clone(),
                id.clone(),
                None,
                Ok(saved),
            ))
            .unwrap();
        assert!(
            state.tab().unwrap().blocks[0]
                .block
                .config
                .get("asset")
                .is_none()
        );
        state.tabs[state.active].blocks[0]
            .edit
            .replace_text("new caption");
        state.tabs[state.active].blocks[0].block.config["note"] = serde_json::json!("new setting");
        state.save_blocks();
        let stored = state.store.get_blocks(&page).unwrap().remove(0);
        assert_eq!(stored.text, "new caption");
        assert_eq!(stored.config["note"], "new setting");
        assert_eq!(stored.config["asset"], asset);
        assert_eq!(state.tab().unwrap().blocks[0].block.config["asset"], asset);
        let mut rt = Runtime::new(state, crate::ui::view, (1000., 800.), Env::default());
        rt.poll = Some(crate::poll);
        rt.frame(0.);
        assert_eq!(rt.state.tab().unwrap().blocks[0].edit.text, "new caption");
        assert_eq!(
            rt.state.tab().unwrap().blocks[0].block.config["note"],
            "new setting"
        );
        assert_eq!(
            rt.state.tab().unwrap().blocks[0].block.config["asset"],
            asset
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn image_file_read_waits_in_worker_not_the_ui() {
        use std::{
            io::Write,
            sync::{Arc, mpsc},
            time::Duration,
        };
        let path = std::env::temp_dir().join(format!("moss-image-nonblocking-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let page = state.tab().unwrap().page.id.clone();
        let id = state.tab().unwrap().blocks[0].block.id.clone();
        state.tabs[state.active].blocks[0].block.kind = moss_core::BlockKind::Image;
        state.save_blocks();
        state.page_cover.set_waker(Arc::new(|| {}));
        let fifo = path.join("delayed-image");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let (release, wait) = mpsc::channel();
        let writer = std::thread::spawn(move || {
            let released = wait.recv_timeout(Duration::from_secs(3)).is_ok();
            let png = [
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0,
                1, 8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99,
                248, 207, 192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68,
                174, 66, 96, 130,
            ];
            std::fs::OpenOptions::new()
                .write(true)
                .open(fifo)
                .unwrap()
                .write_all(&png)
                .unwrap();
            released
        });
        state.upload_block_image(&page, &id, None, path.join("delayed-image"));
        release.send(()).unwrap();
        assert!(
            writer.join().unwrap(),
            "image reading blocked the UI thread"
        );
        let result = state
            .page_cover
            .result_rx
            .as_ref()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(matches!(result, CoverResult::BlockUploaded(_, _, _, Ok(_))));
        assert!(
            state.store.get_blocks(&page).unwrap()[0]
                .config
                .get("asset")
                .is_some()
        );
        drop(state);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn image_block_upload_persists_and_displays() {
        use rune::{Env, Runtime};
        use std::{
            sync::Arc,
            time::{Duration, Instant},
        };
        let path = std::env::temp_dir().join(format!("moss-image-upload-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let page = state.tab().unwrap().page.id.clone();
        let id = state.tab().unwrap().blocks[0].block.id.clone();
        let other = state
            .store
            .create_page(
                &state.workspace.id,
                None,
                "Other",
                "",
                moss_core::PageKind::Document,
            )
            .unwrap();
        state.tabs[state.active].blocks[0].block.kind = moss_core::BlockKind::Image;
        state.save_blocks();
        state.page_cover.set_waker(Arc::new(|| {}));
        let png = [
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
            130,
        ];
        let image_path = path.join("test.png");
        std::fs::write(&image_path, png).unwrap();
        let mut rt = Runtime::new(state, crate::ui::view, (1000., 800.), Env::default());
        rt.poll = Some(crate::poll);
        rt.frame(0.);
        let (x, y) = rt.find(&format!("image-upload-{id}")).unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 60.);
        rt.resolve_file_dialog(Some(image_path)).unwrap();
        let other_id = other.id.clone();
        rt.update(move |s, tx| {
            s.open_page(&other_id, false);
            tx.instant();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while rt.state.store.get_blocks(&page).unwrap()[0]
            .config
            .get("asset")
            .is_none()
        {
            assert!(Instant::now() < deadline, "image upload did not finish");
            rt.frame(1. / 60.);
            std::thread::sleep(Duration::from_millis(2));
        }
        let asset = rt.state.store.get_blocks(&page).unwrap()[0].config["asset"]
            .as_str()
            .unwrap()
            .to_owned();
        let page_id = page.clone();
        rt.update(move |s, tx| {
            s.open_page(&page_id, false);
            tx.instant();
        });
        assert_eq!(
            rt.state.tab().unwrap().blocks[0].block.config["asset"],
            asset
        );
        while !rt
            .state
            .page_cover
            .images
            .get(&asset)
            .is_some_and(|image| image.size().is_some())
        {
            assert!(Instant::now() < deadline, "image did not load");
            rt.frame(1. / 60.);
            std::thread::sleep(Duration::from_millis(2));
        }
        rt.frame(1. / 60.);
        assert!(rt.find(&format!("image-content-{id}")).is_some());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn missing_image_waits_for_explicit_retry() {
        use rune::{Env, Runtime};
        use std::{
            sync::Arc,
            time::{Duration, Instant},
        };
        let path = std::env::temp_dir().join(format!("moss-cover-retry-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let png = [
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
            130,
        ];
        let asset = state.store.import_cover_image(&png).unwrap();
        state.tabs[state.active].page.cover = Some(PageCover::Image {
            asset: asset.clone(),
            position: 0.5,
        });
        std::fs::remove_file(path.join("cover-assets").join(&asset)).unwrap();
        state.page_cover.set_waker(Arc::new(|| {}));
        let mut rt = Runtime::new(state, crate::ui::view, (1000., 800.), Env::default());
        rt.poll = Some(crate::poll);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !rt.state.page_cover.image_errors.contains_key(&asset) {
            assert!(Instant::now() < deadline);
            rt.frame(1. / 60.);
            std::thread::sleep(Duration::from_millis(2));
        }
        for _ in 0..10 {
            rt.frame(1. / 60.);
        }
        assert!(rt.state.page_cover.pending_images.is_empty());
        assert!(!rt.state.page_cover.images.contains_key(&asset));
        assert_eq!(rt.state.store.import_cover_image(&png).unwrap(), asset);
        rt.state.retry_cover_image(&asset);
        while !rt.state.page_cover.images.contains_key(&asset) {
            assert!(Instant::now() < deadline);
            rt.frame(1. / 60.);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!rt.state.page_cover.image_errors.contains_key(&asset));
        drop(rt);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn gradient_picker_loads_default_applies_presets_and_custom_colors() {
        use rune::{Env, Runtime};
        let path = std::env::temp_dir().join(format!("moss-cover-grad-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let page = state.tab().unwrap().page.clone();
        let mut rt = Runtime::new(state, crate::ui::view, (1000., 800.), Env::default());
        rt.frame(0.);
        let click = |rt: &mut Runtime<State>, key: &str| {
            let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
            rt.pointer_move(x, y);
            rt.frame(1. / 60.);
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            rt.frame(1. / 60.);
        };
        click(&mut rt, &format!("page-cover-open-{}-document", page.id));
        rt.frame(1. / 60.);
        click(&mut rt, "cover-gradient");
        assert!(rt.state.page_cover.editor.as_ref().unwrap().gradient_picker);
        assert_eq!(
            rt.state.tab().unwrap().page.cover,
            Some(PageCover::Gradient {
                start: DEFAULT_GRADIENT.0.into(),
                end: DEFAULT_GRADIENT.1.into(),
            })
        );
        // Pick Sunset preset.
        click(&mut rt, "cover-gradient-preset-1");
        assert_eq!(
            rt.state.tab().unwrap().page.cover,
            Some(PageCover::Gradient {
                start: "#e96443".into(),
                end: "#904e95".into(),
            })
        );
        // Pick custom start and end swatches, then swap.
        click(&mut rt, "cover-gradient-start-#3b82f6");
        click(&mut rt, "cover-gradient-end-#34d399");
        assert_eq!(
            rt.state.tab().unwrap().page.cover,
            Some(PageCover::Gradient {
                start: "#3b82f6".into(),
                end: "#34d399".into(),
            })
        );
        click(&mut rt, "cover-gradient-swap");
        assert_eq!(
            rt.state.tab().unwrap().page.cover,
            Some(PageCover::Gradient {
                start: "#34d399".into(),
                end: "#3b82f6".into(),
            })
        );
        // Custom hex inputs persist valid colors and reject invalid hex cleanly.
        rt.state
            .page_cover
            .editor
            .as_mut()
            .unwrap()
            .gradient_start
            .replace_text("#123456");
        rt.state
            .page_cover
            .editor
            .as_mut()
            .unwrap()
            .gradient_end
            .replace_text("#abcdef");
        rt.state.apply_gradient_inputs(true);
        assert_eq!(
            rt.state.tab().unwrap().page.cover,
            Some(PageCover::Gradient {
                start: "#123456".into(),
                end: "#abcdef".into(),
            })
        );
        rt.state
            .page_cover
            .editor
            .as_mut()
            .unwrap()
            .gradient_start
            .replace_text("nope");
        rt.state.apply_gradient_inputs(true);
        assert!(
            rt.state
                .page_cover
                .editor
                .as_ref()
                .unwrap()
                .gradient_error
                .is_some()
        );
        assert_eq!(
            rt.state.tab().unwrap().page.cover,
            Some(PageCover::Gradient {
                start: "#123456".into(),
                end: "#abcdef".into(),
            })
        );
        drop(rt);
        let _ = std::fs::remove_dir_all(path);
    }
}
