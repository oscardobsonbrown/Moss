//! App-owned MCP bridge. All workspace operations run on their owning Rune state.
use crate::state::State;
use moss_mcp::{MAX_MESSAGE, Request, Response};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

type Handles = Arc<Mutex<BTreeMap<String, rune::AppHandle<State>>>>;
type Presence = Arc<Mutex<HashMap<String, (String, String, String)>>>;
type Connections = Arc<Mutex<HashMap<usize, UnixStream>>>;
unsafe extern "C" {
    fn geteuid() -> u32;
    fn flock(fd: i32, operation: i32) -> i32;
}
fn uid() -> u32 {
    unsafe { geteuid() }
}

pub struct LocalService {
    stop: Arc<AtomicBool>,
    socket: PathBuf,
    inode: u64,
    connections: Connections,
    thread: Option<thread::JoinHandle<()>>,
    _lock: File,
}
impl Drop for LocalService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        for stream in self.connections.lock().unwrap().values() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        if fs::symlink_metadata(&self.socket)
            .is_ok_and(|m| m.file_type().is_socket() && m.uid() == uid() && m.ino() == self.inode)
        {
            let _ = fs::remove_file(&self.socket);
        }
    }
}

fn prepare(path: &Path) -> io::Result<(PathBuf, String)> {
    let dir = path.join("mcp");
    match fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    let meta = fs::symlink_metadata(&dir)?;
    if !meta.file_type().is_dir() || meta.uid() != uid() || meta.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Moss MCP directory must be owned by you and private",
        ));
    }
    let token_path = dir.join("token");
    let token = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&token_path)
    {
        Ok(mut f) => {
            let token = format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            );
            f.write_all(token.as_bytes())?;
            f.sync_all()?;
            token
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            let meta = fs::symlink_metadata(&token_path)?;
            if !meta.file_type().is_file()
                || meta.uid() != uid()
                || meta.permissions().mode() & 0o077 != 0
                || meta.len() != 64
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Moss MCP token must be an owner-only file",
                ));
            }
            fs::read_to_string(&token_path)?
        }
        Err(e) => return Err(e),
    };
    Ok((dir.join("service.sock"), token))
}

pub fn start(data_dir: &Path, handles: Handles) -> io::Result<LocalService> {
    let (socket, token) = prepare(data_dir)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .open(data_dir.join("mcp/service.lock"))?;
    let metadata = lock.metadata()?;
    if !metadata.is_file() || metadata.uid() != uid() || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Moss MCP lock must be an owner-only file",
        ));
    }
    if unsafe { flock(lock.as_raw_fd(), 2 | 4) } != 0 {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "Moss local service is already running",
        ));
    }
    if let Ok(meta) = fs::symlink_metadata(&socket) {
        if !meta.file_type().is_socket() || meta.uid() != uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Unexpected Moss MCP socket path",
            ));
        }
        match UnixStream::connect(&socket) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "Moss local service is already running",
                ));
            }
            Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => fs::remove_file(&socket)?,
            Err(e) => return Err(e),
        }
    }
    let listener = UnixListener::bind(&socket)?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
    let inode = fs::symlink_metadata(&socket)?.ino();
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let done = stop.clone();
    let presence: Presence = Arc::new(Mutex::new(HashMap::new()));
    let connections = Arc::new(AtomicUsize::new(0));
    let open: Connections = Arc::new(Mutex::new(HashMap::new()));
    let open_thread = open.clone();
    let next_id = Arc::new(AtomicUsize::new(0));
    let thread = thread::spawn(move || {
        while !done.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, _)) => {
                    if stream.set_nonblocking(false).is_err() {
                        continue;
                    }
                    if connections.fetch_add(1, Ordering::AcqRel) >= 16 {
                        connections.fetch_sub(1, Ordering::AcqRel);
                        continue;
                    }
                    let id = next_id.fetch_add(1, Ordering::Relaxed);
                    let Ok(copy) = stream.try_clone() else {
                        connections.fetch_sub(1, Ordering::AcqRel);
                        continue;
                    };
                    open_thread.lock().unwrap().insert(id, copy);
                    let (token, handles, presence, connections, open, stop) = (
                        token.clone(),
                        handles.clone(),
                        presence.clone(),
                        connections.clone(),
                        open_thread.clone(),
                        done.clone(),
                    );
                    thread::spawn(move || {
                        handle_client(stream, &token, &handles, &presence, &stop);
                        open.lock().unwrap().remove(&id);
                        connections.fetch_sub(1, Ordering::AcqRel);
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50))
                }
                Err(_) => break,
            }
        }
    });
    Ok(LocalService {
        stop,
        socket,
        inode,
        connections: open,
        thread: Some(thread),
        _lock: lock,
    })
}

fn handle_client(
    mut stream: UnixStream,
    token: &str,
    handles: &Handles,
    presence: &Presence,
    stop: &Arc<AtomicBool>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    let Ok(reader) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader);
    let mut session: Option<String> = None;
    loop {
        let mut bytes = Vec::new();
        let result = reader
            .by_ref()
            .take((MAX_MESSAGE + 1) as u64)
            .read_until(b'\n', &mut bytes);
        if result.is_err()
            || bytes.is_empty()
            || bytes.len() > MAX_MESSAGE
            || !bytes.ends_with(b"\n")
        {
            break;
        }
        if stop.load(Ordering::Acquire) {
            break;
        }
        let request: Request = match serde_json::from_slice(&bytes) {
            Ok(r) => r,
            Err(_) => break,
        };
        if request.token != token
            || request.session_id.len() > 128
            || request.session_id.is_empty()
            || request.actor_name.trim().is_empty()
            || request.actor_name.len() > 256
            || session.as_ref().is_some_and(|id| id != &request.session_id)
        {
            break;
        }
        session = Some(request.session_id.clone());
        let workspace = request.workspace_id.clone();
        let page = request
            .args
            .get("page_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let name = request.actor_name.clone();
        let session_id = request.session_id.clone();
        let response = dispatch(request, handles, presence, stop);
        if response.ok && !page.is_empty() {
            let old = presence
                .lock()
                .unwrap()
                .insert(session_id, (workspace.clone(), page.clone(), name));
            set_presence(handles, presence, &workspace, &page);
            if let Some((old_workspace, old_page, _)) =
                old.filter(|(old, previous, _)| *old != workspace || *previous != page)
            {
                set_presence(handles, presence, &old_workspace, &old_page);
            }
        }
        let Ok(mut data) = serde_json::to_vec(&response) else {
            break;
        };
        if data.len() > MAX_MESSAGE {
            break;
        }
        data.push(b'\n');
        if stream.write_all(&data).is_err() {
            break;
        }
    }
    if let Some(session) = session {
        let removed = presence.lock().unwrap().remove(&session);
        if let Some((workspace, page, _)) = removed {
            set_presence(handles, presence, &workspace, &page);
        }
    }
}

fn set_presence(handles: &Handles, presence: &Presence, workspace: &str, page: &str) {
    let handle = { handles.lock().unwrap().get(workspace).cloned() };
    if let Some(handle) = handle {
        let page = page.to_owned();
        let workspace = workspace.to_owned();
        let presence = presence.clone();
        handle.update(move |s, _| {
            let presence = presence.lock().unwrap();
            s.collaboration_live = presence.values().any(|(w, _, _)| w == &workspace);
            let agents = presence
                .iter()
                .filter(|(_, (w, p, _))| w == &workspace && p == &page)
                .map(|(id, (_, _, name))| (id.clone(), name.clone()))
                .collect();
            s.set_agent_presence(&page, agents);
        });
    }
}

fn dispatch(
    request: Request,
    handles: &Handles,
    presence: &Presence,
    stop: &Arc<AtomicBool>,
) -> Response {
    if request.method == "list_workspaces" {
        let selected: Vec<_> = handles.lock().unwrap().values().cloned().collect();
        let count = selected.len();
        let (send, recv) = mpsc::channel();
        for handle in selected {
            let send = send.clone();
            handle.update(move |s, _| {
                let _ = send.send(s.workspace.clone());
            });
        }
        drop(send);
        let mut workspaces = Vec::with_capacity(count);
        for _ in 0..count {
            match recv.recv_timeout(Duration::from_secs(10)) {
                Ok(workspace) => workspaces.push(workspace),
                Err(_) => return Response::error("Moss did not return all workspaces"),
            }
        }
        return Response::ok(json!({"workspaces":workspaces}));
    }
    let handle = { handles.lock().unwrap().get(&request.workspace_id).cloned() };
    let Some(handle) = handle else {
        return Response::error("Workspace is unavailable in this Moss session");
    };
    let (send, recv) = mpsc::sync_channel(1);
    let presence = presence.clone();
    let stop = stop.clone();
    handle.update(move |state, _tx| {
        let result = if stop.load(Ordering::Acquire) {
            Response::error("Moss local service stopped")
        } else {
            process(state, &request, &presence)
        };
        let _ = send.send(result);
    });
    recv.recv_timeout(Duration::from_secs(10))
        .unwrap_or_else(|_| {
            Response::error("Moss did not respond; the operation outcome is unknown")
        })
}

fn within_subtree(store: &moss_core::Store, page_id: &str, root: &str) -> Result<bool, String> {
    let mut current = Some(page_id.to_owned());
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = current {
        if id == root {
            return Ok(true);
        }
        if !seen.insert(id.clone()) {
            return Err("Page hierarchy has a cycle".into());
        }
        current = store
            .get_page(&id)
            .map_err(|e| e.to_string())?
            .ok_or("Page in hierarchy is missing")?
            .parent_id;
    }
    Ok(false)
}

fn process(s: &mut State, request: &Request, presence: &Presence) -> Response {
    fn run(s: &mut State, request: &Request, presence: &Presence) -> Result<Value, String> {
        let workspace = &request.workspace_id;
        if s.workspace.id != *workspace {
            return Err("Workspace mismatch".into());
        }
        let args = &request.args;
        let field = |name: &str| {
            args.get(name)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("Missing {name}"))
        };
        if request.method == "find_pages" {
            let query = field("query")?;
            if query.is_empty() {
                return Ok(
                    json!({"pages":s.store.page_suggestions(workspace,30).map_err(|e|e.to_string())?}),
                );
            }
            return Ok(
                json!({"hits":s.store.search(query,Some(workspace),50).map_err(|e|e.to_string())?}),
            );
        }
        if request.method == "list_trash" {
            return Ok(json!({"pages":s.store.list_trash(workspace).map_err(|e| e.to_string())?}));
        }
        if request.method == "create_page" {
            let title = field("title")?;
            let parent = args.get("parent_id").and_then(Value::as_str);
            if let Some(id) = parent {
                let page = s
                    .store
                    .get_page(id)
                    .map_err(|e| e.to_string())?
                    .ok_or("Parent page not found")?;
                if page.workspace_id != *workspace || page.deleted_at.is_some() {
                    return Err("Parent page is unavailable".into());
                }
            }
            let page = s
                .store
                .create_page(workspace, parent, title, "▤", moss_core::PageKind::Document)
                .map_err(|e| e.to_string())?;
            if let Some(parent) = parent {
                s.keep_draft_for_page(parent);
                s.refresh_child_pages(parent);
            }
            s.refresh_pages();
            return Ok(json!({"page":page}));
        }
        let page_id = field("page_id")?;
        let mut page = s
            .store
            .get_page(page_id)
            .map_err(|e| e.to_string())?
            .ok_or("Page not found")?;
        if page.workspace_id != *workspace
            || (page.deleted_at.is_some() && request.method != "restore_page")
        {
            return Err("Page is unavailable".into());
        }
        let actor = moss_core::history::HistoryActor {
            id: request.session_id.clone(),
            name: request.actor_name.clone(),
            kind: "agent".into(),
        };
        let needs_actor = matches!(
            request.method.as_str(),
            "edit_block"
                | "edit_cell"
                | "create_comment"
                | "reply_comment"
                | "resolve_comment"
                | "rename_page"
                | "trash_page"
                | "restore_page"
        );
        if needs_actor {
            s.store
                .set_history_actor(Some(actor))
                .map_err(|e| e.to_string())?;
        }
        let result = (|| -> Result<Value, String> {
            match request.method.as_str() {
                "rename_page" => {
                    if s.tabs
                        .iter()
                        .filter(|tab| tab.page.id == page_id)
                        .any(|tab| {
                            tab.dirty_title || tab.title.focused || tab.title.preedit.is_some()
                        })
                        || s.record_peek
                            .as_ref()
                            .filter(|peek| peek.page.id == page_id)
                            .is_some_and(|peek| {
                                peek.dirty_title
                                    || peek.title.focused
                                    || peek.title.preedit.is_some()
                            })
                        || s.sidebar_page_menu
                            .as_ref()
                            .is_some_and(|menu| menu.page.id == page_id && menu.renaming)
                    {
                        return Err("The page title is being edited in Moss".into());
                    }
                    let updated = s
                        .store
                        .rename_page_if_unchanged(
                            page_id,
                            field("expected_title")?,
                            field("title")?,
                        )
                        .map_err(|e| e.to_string())?;
                    for tab in s.tabs.iter_mut().filter(|tab| tab.page.id == page_id) {
                        tab.title.replace_text(&updated.title);
                        tab.page = updated.clone();
                    }
                    if let Some(peek) = s
                        .record_peek
                        .as_mut()
                        .filter(|peek| peek.page.id == page_id)
                    {
                        peek.title.replace_text(&updated.title);
                        peek.page = updated.clone();
                    }
                    s.keep_draft_for_page(page_id);
                    s.update_child_page(&updated);
                    s.refresh_pages();
                    Ok(json!({"page":updated}))
                }
                "trash_page" | "restore_page" => {
                    let expected: moss_core::Page = serde_json::from_value(
                        args.get("expected")
                            .cloned()
                            .ok_or("Missing expected page")?,
                    )
                    .map_err(|e| e.to_string())?;
                    if page != expected {
                        return Err("Page changed; read it again".into());
                    }
                    if page.kind == moss_core::PageKind::Record {
                        return Err("Manage record pages from their database".into());
                    }
                    if request.method == "trash_page" {
                        for tab in &s.tabs {
                            if within_subtree(&s.store, &tab.page.id, page_id)? {
                                return Err("Close the page and its open descendants before moving it to Trash".into());
                            }
                        }
                        if let Some(peek) = &s.record_peek {
                            if within_subtree(&s.store, &peek.page.id, page_id)? {
                                return Err(
                                    "Close the open record before moving its parent to Trash"
                                        .into(),
                                );
                            }
                        }
                        for draft in s.new_item_drafts.values() {
                            if let Some(draft_page) = &draft.page {
                                if within_subtree(&s.store, &draft_page.id, page_id)? {
                                    return Err(
                                        "Finish the page draft before moving it to Trash".into()
                                    );
                                }
                            }
                            if let Some(record) = &draft.record {
                                let database = s
                                    .store
                                    .get_database(&record.database_id)
                                    .map_err(|e| e.to_string())?
                                    .ok_or("Draft database is missing")?;
                                if within_subtree(&s.store, &database.page_id, page_id)? {
                                    return Err("Finish the record draft before moving its database to Trash".into());
                                }
                            }
                        }
                        let menu_in_subtree = match s.sidebar_page_menu.as_ref() {
                            Some(menu) => within_subtree(&s.store, &menu.page.id, page_id)?,
                            None => false,
                        };
                        if menu_in_subtree
                            && s.sidebar_page_menu
                                .as_ref()
                                .is_some_and(|menu| menu.renaming)
                        {
                            return Err(
                                "Finish editing the page name before moving it to Trash".into()
                            );
                        }
                        s.store.delete_page(page_id).map_err(|e| e.to_string())?;
                        if menu_in_subtree {
                            s.sidebar_page_menu = None;
                        }
                        if let Some(parent) = page.parent_id.as_deref() {
                            s.refresh_child_pages(parent);
                        }
                        s.refresh_pages();
                    } else {
                        if page.deleted_at.is_none() {
                            return Err("Page is not in Trash".into());
                        }
                        s.store.restore_page(page_id).map_err(|e| e.to_string())?;
                        if let Some(parent) = page.parent_id.as_deref() {
                            s.refresh_child_pages(parent);
                        }
                        s.refresh_pages();
                    }
                    if s.modal == Some(crate::state::Modal::Trash) {
                        s.trash = s.store.list_trash(workspace).map_err(|e| e.to_string())?;
                    }
                    let updated = s
                        .store
                        .get_page(page_id)
                        .map_err(|e| e.to_string())?
                        .ok_or("Page missing after operation")?;
                    Ok(json!({"page":updated}))
                }
                "read_page" => {
                    let stored_blocks = s.store.get_blocks(page_id).map_err(|e| e.to_string())?;
                    let mut blocks = stored_blocks.clone();
                    let mut has_draft = false;
                    let mut unsaved = false;
                    let mut editing = Vec::new();
                    let mut draft_title: Option<String> = None;
                    for tab in s.tabs.iter().filter(|t| t.page.id == page_id) {
                        unsaved |= tab.dirty_blocks
                            || tab.dirty_title
                            || tab.title.preedit.is_some()
                            || tab.blocks.iter().any(|b| b.edit.preedit.is_some());
                        if draft_title
                            .as_ref()
                            .is_some_and(|title| title != &tab.title.text)
                        {
                            return Err(
                                "Multiple open views have different titles; resolve their drafts"
                                    .into(),
                            );
                        }
                        draft_title = Some(tab.title.text.clone());
                        page.title = tab.title.text.clone();
                        if tab.title.focused || tab.title.preedit.is_some() {
                            editing.push(json!({"actor":"You","field":"title","selection":tab.title.range(),"composition":tab.title.preedit}));
                        }
                        let mut visible = Vec::new();
                        for item in &tab.blocks {
                            if item.edit.focused || item.edit.preedit.is_some() {
                                editing.push(json!({"actor":"You","block_id":item.block.id,"selection":item.edit.range(),"composition":item.edit.preedit}));
                            }
                            let mut block = item.block.clone();
                            let mut edit = item.edit.clone();
                            crate::rich_text::save(&mut block, &mut edit);
                            visible.push(block);
                        }
                        if has_draft && visible != blocks {
                            return Err("Multiple open views have different content; retry after resolving their drafts".into());
                        }
                        blocks = visible;
                        has_draft = true;
                    }
                    if let Some(peek) = s.record_peek.as_ref().filter(|p| p.page.id == page_id) {
                        unsaved |= peek.dirty_blocks
                            || peek.dirty_title
                            || peek.title.preedit.is_some()
                            || peek.blocks.iter().any(|b| b.edit.preedit.is_some());
                        if draft_title
                            .as_ref()
                            .is_some_and(|title| title != &peek.title.text)
                        {
                            return Err(
                                "Multiple open views have different titles; resolve their drafts"
                                    .into(),
                            );
                        }
                        page.title = peek.title.text.clone();
                        if peek.title.focused || peek.title.preedit.is_some() {
                            editing.push(json!({"actor":"You","field":"title","selection":peek.title.range(),"composition":peek.title.preedit}));
                        }
                        let mut visible = Vec::new();
                        for item in &peek.blocks {
                            if item.edit.focused || item.edit.preedit.is_some() {
                                editing.push(json!({"actor":"You","block_id":item.block.id,"selection":item.edit.range(),"composition":item.edit.preedit}));
                            }
                            let mut block = item.block.clone();
                            let mut edit = item.edit.clone();
                            crate::rich_text::save(&mut block, &mut edit);
                            visible.push(block);
                        }
                        if has_draft && visible != blocks {
                            return Err("Multiple open views have different content; retry after resolving their drafts".into());
                        }
                        blocks = visible;
                        has_draft = true;
                    }
                    Ok(
                        json!({"page":page,"blocks":blocks,"stored_blocks":stored_blocks,"live_view":has_draft,"unsaved":unsaved,"editing":editing}),
                    )
                }
                "status" => {
                    let actors: Vec<_> = presence
                        .lock()
                        .unwrap()
                        .values()
                        .filter(|(w, p, _)| w == workspace && p == page_id)
                        .map(|(_, _, name)| name.clone())
                        .collect();
                    Ok(json!({"connected":true,"participants":actors}))
                }
                "edit_block" => {
                    let block_id = field("block_id")?;
                    for tab in s.tabs.iter().filter(|t| t.page.id == page_id) {
                        if tab.dirty_blocks
                            || tab.blocks.iter().any(|b| {
                                b.block.id == block_id
                                    && (b.edit.focused || b.edit.preedit.is_some())
                            })
                        {
                            return Err(
                                "This block is being edited in Moss; retry after the edit is saved"
                                    .into(),
                            );
                        }
                    }
                    if let Some(peek) = s.record_peek.as_ref().filter(|p| p.page.id == page_id) {
                        if peek.dirty_blocks
                            || peek.blocks.iter().any(|b| {
                                b.block.id == block_id
                                    && (b.edit.focused || b.edit.preedit.is_some())
                            })
                        {
                            return Err(
                                "This block is being edited in Moss; retry after the edit is saved"
                                    .into(),
                            );
                        }
                    }
                    let expected: moss_core::Block = serde_json::from_value(
                        args.get("expected")
                            .cloned()
                            .ok_or("Missing expected block")?,
                    )
                    .map_err(|e| e.to_string())?;
                    if expected.id != block_id || expected.page_id != page_id {
                        return Err("Expected block does not match target".into());
                    }
                    if !matches!(
                        expected.kind,
                        moss_core::BlockKind::Paragraph
                            | moss_core::BlockKind::Heading1
                            | moss_core::BlockKind::Heading2
                            | moss_core::BlockKind::Heading3
                            | moss_core::BlockKind::Quote
                            | moss_core::BlockKind::Code
                    ) || expected
                        .config
                        .get("marks")
                        .and_then(Value::as_array)
                        .is_some_and(|m| !m.is_empty())
                    {
                        return Err("Only plain text blocks can be edited by this service".into());
                    }
                    let mut blocks = s.store.get_blocks(page_id).map_err(|e| e.to_string())?;
                    let before = blocks.clone();
                    let block = blocks
                        .iter_mut()
                        .find(|b| b.id == block_id)
                        .ok_or("Block not found")?;
                    if *block != expected {
                        return Err("Block changed; read the page again".into());
                    }
                    let mut edit = crate::rich_text::load(block);
                    if !edit.marks().is_empty() || !edit.soft_breaks().is_empty() {
                        return Err("Only plain text blocks can be edited by this service".into());
                    }
                    edit.replace_text(field("text")?);
                    crate::rich_text::save(block, &edit);
                    let result = s
                        .store
                        .save_editor_blocks_if_unchanged(page_id, &before, &blocks);
                    let saved = result.map_err(|e| e.to_string())?;
                    s.external_refresh_pending = true;
                    s.refresh_pages();
                    Ok(json!({"blocks":saved,"revision":saved}))
                }
                "edit_cell" => {
                    let record_id = field("record_id")?;
                    let property_id = field("property_id")?;
                    let record = s
                        .store
                        .get_record(record_id)
                        .map_err(|e| e.to_string())?
                        .ok_or("Record not found")?;
                    let db = s
                        .store
                        .get_database(&record.database_id)
                        .map_err(|e| e.to_string())?
                        .ok_or("Database not found")?;
                    if db.page_id != page_id {
                        return Err("Record is not in this database".into());
                    }
                    if s.editing_cell.as_ref().is_some_and(|c| {
                        c.record_id == record_id && c.property_id.as_deref() == Some(property_id)
                    }) {
                        return Err("This cell is being edited in Moss".into());
                    }
                    let result = s.store.set_record_value_if_unchanged(
                        record_id,
                        property_id,
                        args.get("expected").ok_or("Missing expected value")?,
                        args.get("value").cloned().ok_or("Missing value")?,
                    );
                    let record = result.map_err(|e| e.to_string())?;
                    s.sync_record(&record);
                    Ok(json!({"record":record}))
                }
                "list_comments" => Ok(json!(
                    s.store
                        .list_comments(workspace, page_id)
                        .map_err(|e| e.to_string())?
                )),
                "create_comment" => {
                    let anchor = serde_json::from_value(
                        args.get("anchor").cloned().ok_or("Missing anchor")?,
                    )
                    .map_err(|e| e.to_string())?;
                    Ok(json!(
                        s.store
                            .create_comment(
                                workspace,
                                page_id,
                                anchor,
                                &request.actor_name,
                                field("text")?
                            )
                            .map_err(|e| e.to_string())?
                    ))
                }
                "reply_comment" | "resolve_comment" => {
                    let id = field("thread_id")?;
                    if !s
                        .store
                        .list_comments(workspace, page_id)
                        .map_err(|e| e.to_string())?
                        .iter()
                        .any(|t| t.id == id)
                    {
                        return Err("Comment is not on this page".into());
                    }
                    if request.method == "reply_comment" {
                        s.store
                            .reply_comment(workspace, id, &request.actor_name, field("text")?)
                            .map_err(|e| e.to_string())?;
                    } else {
                        s.store
                            .resolve_comment(
                                workspace,
                                id,
                                args.get("resolved")
                                    .and_then(Value::as_bool)
                                    .ok_or("Missing resolved")?,
                            )
                            .map_err(|e| e.to_string())?;
                    }
                    Ok(json!({"ok":true}))
                }
                "page_history" => Ok(json!(
                    s.store
                        .page_history(
                            workspace,
                            page_id,
                            None,
                            args.get("before").and_then(Value::as_i64),
                            args.get("limit")
                                .and_then(Value::as_u64)
                                .unwrap_or(50)
                                .min(200) as u32
                        )
                        .map_err(|e| e.to_string())?
                )),
                _ => Err("Unknown operation".into()),
            }
        })();
        if needs_actor {
            s.store
                .set_history_actor(Some(moss_core::history::HistoryActor {
                    id: "local-user".into(),
                    name: "You".into(),
                    kind: "person".into(),
                }))
                .map_err(|e| e.to_string())?;
        }
        if result.is_ok()
            && matches!(
                request.method.as_str(),
                "create_comment" | "reply_comment" | "resolve_comment"
            )
        {
            s.load_comments(page_id);
        }
        result
    }
    match run(s, request, presence) {
        Ok(value) => Response::ok(value),
        Err(error) => Response::error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_is_private_authenticates_and_shuts_down_connections() {
        let root = PathBuf::from("/tmp").join(format!("mcp-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir(&root).unwrap();
        let handles: Handles = Arc::new(Mutex::new(BTreeMap::new()));
        let service = start(&root, handles).unwrap();
        let dir = fs::symlink_metadata(root.join("mcp")).unwrap();
        let token_meta = fs::symlink_metadata(root.join("mcp/token")).unwrap();
        let socket_meta = fs::symlink_metadata(root.join("mcp/service.sock")).unwrap();
        assert_eq!(dir.permissions().mode() & 0o777, 0o700);
        assert_eq!(token_meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(socket_meta.permissions().mode() & 0o777, 0o600);
        let token = fs::read_to_string(root.join("mcp/token")).unwrap();
        let mut bad = UnixStream::connect(root.join("mcp/service.sock")).unwrap();
        bad.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let req = Request {
            token: "wrong".into(),
            session_id: "s".into(),
            actor_name: "Agent".into(),
            workspace_id: "w".into(),
            method: "status".into(),
            args: json!({"page_id":"p"}),
        };
        writeln!(bad, "{}", serde_json::to_string(&req).unwrap()).unwrap();
        let mut read = String::new();
        assert_eq!(BufReader::new(bad).read_line(&mut read).unwrap(), 0);
        let mut good = UnixStream::connect(root.join("mcp/service.sock")).unwrap();
        good.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        thread::sleep(Duration::from_millis(150));
        let req = Request { token, ..req };
        writeln!(good, "{}", serde_json::to_string(&req).unwrap()).unwrap();
        let mut reader = BufReader::new(good);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let response: Response = serde_json::from_str(&line).unwrap();
        assert!(!response.ok);
        drop(service);
        line.clear();
        assert_eq!(reader.read_line(&mut line).unwrap(), 0);
        assert!(!root.join("mcp/service.sock").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn active_socket_is_preserved_and_stale_socket_recovers() {
        let root = PathBuf::from("/tmp").join(format!("mcp-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir(&root).unwrap();
        let handles: Handles = Arc::new(Mutex::new(BTreeMap::new()));
        let service = start(&root, handles.clone()).unwrap();
        assert_eq!(
            start(&root, handles.clone()).err().unwrap().kind(),
            io::ErrorKind::AddrInUse
        );
        assert!(UnixStream::connect(root.join("mcp/service.sock")).is_ok());
        drop(service);
        let stale = UnixListener::bind(root.join("mcp/service.sock")).unwrap();
        drop(stale);
        let recovered = start(&root, handles).unwrap();
        assert!(UnixStream::connect(root.join("mcp/service.sock")).is_ok());
        drop(recovered);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn page_operations_guard_open_work_and_scope() {
        let root = PathBuf::from("/tmp").join(format!("mcp-{}", uuid::Uuid::new_v4().simple()));
        let mut s = State::open(root.clone(), None).unwrap();
        let presence = Arc::new(Mutex::new(HashMap::new()));
        let mut req = Request {
            token: String::new(),
            session_id: "session".into(),
            actor_name: "Agent".into(),
            workspace_id: s.workspace.id.clone(),
            method: "create_page".into(),
            args: json!({"parent_id":null,"title":"Agent page"}),
        };
        let result = process(&mut s, &req, &presence);
        assert!(result.ok, "{:?}", result.result);
        let page: moss_core::Page = serde_json::from_value(result.result["page"].clone()).unwrap();
        req.method = "rename_page".into();
        req.args = json!({"page_id":page.id,"expected_title":"wrong","title":"New title"});
        assert!(!process(&mut s, &req, &presence).ok);
        req.args["expected_title"] = json!("Agent page");
        let result = process(&mut s, &req, &presence);
        assert!(result.ok, "{:?}", result.result);
        let renamed: moss_core::Page =
            serde_json::from_value(result.result["page"].clone()).unwrap();
        assert_eq!(renamed.title, "New title");
        s.open_page(&renamed.id, true);
        s.tabs[s.active].title.focused = true;
        req.args =
            json!({"page_id":renamed.id,"expected_title":"New title","title":"Overwrites edit"});
        assert!(!process(&mut s, &req, &presence).ok);
        s.tabs[s.active].title.focused = false;
        s.tabs.retain(|tab| tab.page.id != renamed.id);
        let child = s
            .store
            .create_page(
                &s.workspace.id,
                Some(&renamed.id),
                "Child",
                "",
                moss_core::PageKind::Document,
            )
            .unwrap();
        s.open_page(&child.id, true);
        req.method = "trash_page".into();
        req.args = json!({"page_id":renamed.id,"expected":renamed});
        assert!(!process(&mut s, &req, &presence).ok);
        s.tabs.retain(|tab| tab.page.id != child.id);
        s.track_new_page(&child);
        assert!(!process(&mut s, &req, &presence).ok);
        s.new_item_drafts.remove(&child.id);
        req.args = json!({"page_id":page.id,"expected":page});
        assert!(!process(&mut s, &req, &presence).ok);
        s.open_page(&renamed.id, true);
        req.args["expected"] = json!(renamed);
        assert!(!process(&mut s, &req, &presence).ok);
        assert!(
            s.store
                .get_page(&renamed.id)
                .unwrap()
                .unwrap()
                .deleted_at
                .is_none()
        );
        s.tabs.retain(|tab| tab.page.id != renamed.id);
        let result = process(&mut s, &req, &presence);
        assert!(result.ok, "{:?}", result.result);
        let trashed: moss_core::Page =
            serde_json::from_value(result.result["page"].clone()).unwrap();
        assert!(trashed.deleted_at.is_some());
        req.method = "list_trash".into();
        let listed = process(&mut s, &req, &presence);
        assert!(
            listed.result["pages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["id"] == renamed.id)
        );
        req.method = "restore_page".into();
        req.args = json!({"page_id":renamed.id,"expected":trashed});
        let restored = process(&mut s, &req, &presence);
        assert!(restored.ok, "{:?}", restored.result);
        assert!(restored.result["page"]["deleted_at"].is_null());
        req.method = "create_page".into();
        let other = s.store.create_workspace("Other", "").unwrap();
        let foreign = s
            .store
            .create_page(
                &other.id,
                None,
                "Foreign",
                "",
                moss_core::PageKind::Document,
            )
            .unwrap();
        req.args = json!({"parent_id":foreign.id,"title":"Invalid"});
        assert!(!process(&mut s, &req, &presence).ok);
        drop(s);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn setting_starts_stops_and_persists_service() {
        let root = PathBuf::from("/tmp").join(format!("mcp-{}", uuid::Uuid::new_v4().simple()));
        let mut s = State::open(root.clone(), None).unwrap();
        assert!(!s.prefs.mcp_enabled);
        s.set_mcp_enabled(true);
        assert!(s.prefs.mcp_enabled);
        assert!(UnixStream::connect(root.join("mcp/service.sock")).is_ok());
        let prefs: crate::state::Preferences =
            serde_json::from_slice(&fs::read(root.join("preferences.json")).unwrap()).unwrap();
        assert!(prefs.mcp_enabled);
        s.set_mcp_enabled(false);
        assert!(!s.prefs.mcp_enabled);
        assert!(!root.join("mcp/service.sock").exists());
        let prefs: crate::state::Preferences =
            serde_json::from_slice(&fs::read(root.join("preferences.json")).unwrap()).unwrap();
        assert!(!prefs.mcp_enabled);
        drop(s);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn page_read_reports_composition_and_checks_workspace() {
        let root = PathBuf::from("/tmp").join(format!("mcp-{}", uuid::Uuid::new_v4().simple()));
        let mut s = State::open(root.clone(), None).unwrap();
        let page_id = s.tab().unwrap().page.id.clone();
        let block = &mut s.tabs[s.active].blocks[0].edit;
        block.replace_text("abc");
        block.set_selection(1, 2);
        block.focused = true;
        block.preedit = Some(("composing".into(), None));
        let mut req = Request {
            token: String::new(),
            session_id: "session".into(),
            actor_name: "Agent".into(),
            workspace_id: s.workspace.id.clone(),
            method: "read_page".into(),
            args: json!({"page_id":page_id}),
        };
        let presence = Arc::new(Mutex::new(HashMap::new()));
        let response = process(&mut s, &req, &presence);
        assert!(response.ok);
        assert_eq!(response.result["unsaved"], true);
        assert_eq!(response.result["editing"][0]["composition"][0], "composing");
        assert_eq!(response.result["editing"][0]["selection"], json!([1, 2]));
        req.workspace_id = "other".into();
        assert!(!process(&mut s, &req, &presence).ok);
        drop(s);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_insecure_directory_or_symlinked_token() {
        let root = PathBuf::from("/tmp").join(format!("mcp-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("mcp")).unwrap();
        fs::set_permissions(root.join("mcp"), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            prepare(&root).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::set_permissions(root.join("mcp"), fs::Permissions::from_mode(0o700)).unwrap();
        std::os::unix::fs::symlink("/dev/null", root.join("mcp/token")).unwrap();
        assert_eq!(
            prepare(&root).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::remove_dir_all(root).unwrap();
    }
}
