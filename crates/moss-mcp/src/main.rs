use moss_mcp::{MAX_MESSAGE, Request, Response};
use rmcp::{
    ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_router,
    transport::stdio,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    sync::Mutex,
};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Scope {
    workspace_id: String,
    page_id: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WorkspaceScope {
    workspace_id: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreatePage {
    workspace_id: String,
    parent_id: Option<String>,
    title: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RenamePage {
    workspace_id: String,
    page_id: String,
    expected_title: String,
    title: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ChangeTrash {
    workspace_id: String,
    page_id: String,
    expected: Value,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FindPages {
    workspace_id: String,
    query: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EditBlock {
    workspace_id: String,
    page_id: String,
    block_id: String,
    expected: Value,
    text: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EditCell {
    workspace_id: String,
    page_id: String,
    record_id: String,
    property_id: String,
    expected: Value,
    value: Value,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateComment {
    workspace_id: String,
    page_id: String,
    anchor: Value,
    text: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReplyComment {
    workspace_id: String,
    page_id: String,
    thread_id: String,
    text: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ResolveComment {
    workspace_id: String,
    page_id: String,
    thread_id: String,
    resolved: bool,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct History {
    workspace_id: String,
    page_id: String,
    before: Option<i64>,
    limit: Option<u32>,
}

#[derive(Clone)]
struct Bridge {
    stream: Arc<Mutex<BufReader<UnixStream>>>,
    token: String,
    session_id: String,
    actor_name: String,
}
impl Bridge {
    async fn call(&self, workspace_id: String, method: &str, args: Value) -> CallToolResult {
        let request = Request {
            token: self.token.clone(),
            session_id: self.session_id.clone(),
            actor_name: self.actor_name.clone(),
            workspace_id,
            method: method.into(),
            args,
        };
        let result = async {
            let mut stream = self.stream.lock().await;
            let mut bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
            if bytes.len() > MAX_MESSAGE {
                return Err("Request exceeds the local service limit".to_string());
            }
            bytes.push(b'\n');
            stream
                .get_mut()
                .write_all(&bytes)
                .await
                .map_err(|e| e.to_string())?;
            let mut response = Vec::new();
            let n = (&mut *stream)
                .take((MAX_MESSAGE + 1) as u64)
                .read_until(b'\n', &mut response)
                .await
                .map_err(|e| e.to_string())?;
            if n == 0 || n > MAX_MESSAGE || !response.ends_with(b"\n") {
                return Err("Moss local service returned an invalid response".to_string());
            }
            let response: Response =
                serde_json::from_slice(&response).map_err(|e| e.to_string())?;
            Ok::<_, String>(response)
        }
        .await;
        match result {
            Ok(response) if response.ok => {
                CallToolResult::success(vec![ContentBlock::text(response.result.to_string())])
            }
            Ok(response) => {
                CallToolResult::error(vec![ContentBlock::text(response.result.to_string())])
            }
            Err(error) => CallToolResult::error(vec![ContentBlock::text(error)]),
        }
    }
}

#[tool_router(server_handler)]
impl Bridge {
    #[tool(description = "List local workspaces and their stable IDs.")]
    async fn list_workspaces(&self) -> CallToolResult {
        self.call(String::new(), "list_workspaces", json!({})).await
    }
    #[tool(description = "Create a document in a workspace, optionally under a live parent page.")]
    async fn create_page(&self, Parameters(p): Parameters<CreatePage>) -> CallToolResult {
        self.call(
            p.workspace_id,
            "create_page",
            json!({"parent_id":p.parent_id,"title":p.title}),
        )
        .await
    }
    #[tool(
        description = "Rename a document or database if its saved title equals expected_title. Active title edits are guarded."
    )]
    async fn rename_page(&self, Parameters(p): Parameters<RenamePage>) -> CallToolResult {
        self.call(
            p.workspace_id,
            "rename_page",
            json!({"page_id":p.page_id,"expected_title":p.expected_title,"title":p.title}),
        )
        .await
    }
    #[tool(
        description = "List pages in a workspace's Trash. Restore each trash root before its descendants."
    )]
    async fn list_trash(&self, Parameters(p): Parameters<WorkspaceScope>) -> CallToolResult {
        self.call(p.workspace_id, "list_trash", json!({})).await
    }
    #[tool(
        description = "Move a closed page subtree to Trash. Pass the full page object as expected."
    )]
    async fn trash_page(&self, Parameters(p): Parameters<ChangeTrash>) -> CallToolResult {
        self.call(
            p.workspace_id,
            "trash_page",
            json!({"page_id":p.page_id,"expected":p.expected}),
        )
        .await
    }
    #[tool(
        description = "Restore a page subtree from Trash. Pass its full page object from list_trash as expected."
    )]
    async fn restore_page(&self, Parameters(p): Parameters<ChangeTrash>) -> CallToolResult {
        self.call(
            p.workspace_id,
            "restore_page",
            json!({"page_id":p.page_id,"expected":p.expected}),
        )
        .await
    }
    #[tool(
        description = "Find pages in a workspace by text, or pass an empty query for page suggestions."
    )]
    async fn find_pages(&self, Parameters(p): Parameters<FindPages>) -> CallToolResult {
        self.call(p.workspace_id, "find_pages", json!({"query":p.query}))
            .await
    }
    #[tool(
        description = "Read a Moss page, visible content, stored blocks and editing status. Use stored_blocks as the expected value when editing."
    )]
    async fn read_page(&self, Parameters(p): Parameters<Scope>) -> CallToolResult {
        self.call(p.workspace_id, "read_page", json!({"page_id":p.page_id}))
            .await
    }
    #[tool(
        description = "Change a plain text block if its full stored block equals expected. A block being edited or a page with unsaved edits is guarded."
    )]
    async fn edit_block(&self, Parameters(p): Parameters<EditBlock>) -> CallToolResult {
        self.call(
            p.workspace_id,
            "edit_block",
            json!({"page_id":p.page_id,"block_id":p.block_id,"expected":p.expected,"text":p.text}),
        )
        .await
    }
    #[tool(description = "Change a database record property if its current value equals expected.")]
    async fn edit_cell(&self, Parameters(p): Parameters<EditCell>) -> CallToolResult {
        self.call(p.workspace_id,"edit_cell",json!({"page_id":p.page_id,"record_id":p.record_id,"property_id":p.property_id,"expected":p.expected,"value":p.value})).await
    }
    #[tool(
        description = "List page comments including replies, resolved state and anchor availability."
    )]
    async fn list_comments(&self, Parameters(p): Parameters<Scope>) -> CallToolResult {
        self.call(
            p.workspace_id,
            "list_comments",
            json!({"page_id":p.page_id}),
        )
        .await
    }
    #[tool(description = "Create a comment. anchor is a typed page, block, text, or cell anchor.")]
    async fn create_comment(&self, Parameters(p): Parameters<CreateComment>) -> CallToolResult {
        self.call(
            p.workspace_id,
            "create_comment",
            json!({"page_id":p.page_id,"anchor":p.anchor,"text":p.text}),
        )
        .await
    }
    #[tool(description = "Reply to a comment thread on the selected page.")]
    async fn reply_comment(&self, Parameters(p): Parameters<ReplyComment>) -> CallToolResult {
        self.call(
            p.workspace_id,
            "reply_comment",
            json!({"page_id":p.page_id,"thread_id":p.thread_id,"text":p.text}),
        )
        .await
    }
    #[tool(description = "Resolve or reopen a comment thread on the selected page.")]
    async fn resolve_comment(&self, Parameters(p): Parameters<ResolveComment>) -> CallToolResult {
        self.call(
            p.workspace_id,
            "resolve_comment",
            json!({"page_id":p.page_id,"thread_id":p.thread_id,"resolved":p.resolved}),
        )
        .await
    }
    #[tool(description = "Read paged page change history, most recent first.")]
    async fn page_history(&self, Parameters(p): Parameters<History>) -> CallToolResult {
        self.call(
            p.workspace_id,
            "page_history",
            json!({"page_id":p.page_id,"before":p.before,"limit":p.limit}),
        )
        .await
    }
    #[tool(description = "Get connection status and participants for a page.")]
    async fn status(&self, Parameters(p): Parameters<Scope>) -> CallToolResult {
        self.call(p.workspace_id, "status", json!({"page_id":p.page_id}))
            .await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::env::var_os("MOSS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join("Library/Application Support/Moss")
        });
    let token = std::fs::read_to_string(data.join("mcp/token"))?;
    let stream = UnixStream::connect(data.join("mcp/service.sock")).await?;
    let bridge = Bridge {
        stream: Arc::new(Mutex::new(BufReader::new(stream))),
        token: token.trim().into(),
        session_id: uuid::Uuid::new_v4().to_string(),
        actor_name: std::env::var("MOSS_AGENT_NAME").unwrap_or_else(|_| "Agent".into()),
    };
    let service = bridge.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
