use crate::state::State;
use moss_core::{Page, PageCover, Store};
use rune::{Image, ImageCache, Tx, Waker};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::mpsc,
};

pub struct CoverEditor {
    pub page_id: String,
    pub generation: u64,
    pub anchor: String,
    pub expected: Option<PageCover>,
    pub draft: Option<PageCover>,
    pub reposition: bool,
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
    result_tx: Option<mpsc::Sender<CoverResult>>,
    result_rx: Option<mpsc::Receiver<CoverResult>>,
}
enum CoverResult {
    Loaded(String, Result<Vec<u8>, String>),
    Uploaded(String, u64, Option<PageCover>, Result<String, String>),
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
        self.page_cover.editor_generation = self.page_cover.editor_generation.wrapping_add(1);
        self.page_cover.editor = Some(CoverEditor {
            generation: self.page_cover.editor_generation,
            page_id: page.id.clone(),
            anchor: anchor.into(),
            expected: page.cover.clone(),
            draft: page.cover.clone(),
            reposition: false,
            error: None,
        });
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
pub fn poll(s: &mut State, _time: f64, _tx: &mut Tx) -> bool {
    let mut wanted = Vec::new();
    if let Some(tab) = s.tab() {
        if let Some(PageCover::Image { asset, .. }) = &tab.page.cover {
            wanted.push(asset.clone());
        }
    }
    if let Some(peek) = &s.record_peek {
        if let Some(PageCover::Image { asset, .. }) = &peek.page.cover {
            wanted.push(asset.clone());
        }
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
}
