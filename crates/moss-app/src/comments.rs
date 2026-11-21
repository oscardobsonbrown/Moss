use std::collections::BTreeMap;

use crate::state::State;
use moss_core::comments::{CommentAnchor, CommentThread};
use rune::TextEdit;

#[derive(Default)]
pub struct CommentsState {
    pub open: bool,
    pub page_id: Option<String>,
    pub threads: BTreeMap<String, Vec<CommentThread>>,
    pub input: TextEdit,
    pub anchor: Option<CommentAnchor>,
    pub reply: Option<String>,
    pub error: Option<String>,
}

impl State {
    pub fn load_comments(&mut self, page_id: &str) {
        match self.store.list_comments(&self.workspace.id, page_id) {
            Ok(threads) => {
                if !self.comments.threads.contains_key(page_id) && self.comments.threads.len() >= 64
                {
                    if let Some(old) = self
                        .comments
                        .threads
                        .keys()
                        .find(|id| Some(id.as_str()) != self.comments.page_id.as_deref())
                        .cloned()
                    {
                        self.comments.threads.remove(&old);
                    }
                }
                self.comments.threads.insert(page_id.to_owned(), threads);
            }
            Err(error) => self.error = Some(format!("Could not load comments: {error}")),
        }
    }

    pub fn open_comments(&mut self, page_id: &str, anchor: Option<CommentAnchor>) {
        if !self.comments.input.text.trim().is_empty()
            && (self.comments.page_id.as_deref() != Some(page_id) || anchor.is_some())
        {
            self.comments.error = Some("Finish the current comment first.".into());
            self.comments.open = true;
            return;
        }
        self.history.open = false;
        self.load_comments(page_id);
        self.comments.page_id = Some(page_id.to_owned());
        self.comments.open = true;
        self.comments.error = None;
        if let Some(anchor) = anchor {
            self.comments.anchor = Some(anchor);
            self.comments.reply = None;
            self.comments.input.focused = true;
        }
    }

    pub fn comment_for_block(&mut self, page_id: &str, block_id: &str, peek: bool) {
        let block = if peek {
            self.record_peek
                .as_ref()
                .filter(|p| p.page.id == page_id)
                .and_then(|p| p.blocks.iter().find(|b| b.block.id == block_id))
        } else {
            self.tab()
                .filter(|p| p.page.id == page_id)
                .and_then(|p| p.blocks.iter().find(|b| b.block.id == block_id))
        };
        let Some(block) = block else { return };
        let (start, end) = block.edit.range();
        let anchor = if start < end {
            CommentAnchor::Text {
                block_id: block_id.into(),
                start,
                end,
                quote: block.edit.text[start..end].to_owned(),
            }
        } else {
            CommentAnchor::Block {
                block_id: block_id.into(),
            }
        };
        self.block_commands = None;
        self.open_comments(page_id, Some(anchor));
    }

    pub fn submit_comment(&mut self) {
        let Some(page_id) = self.comments.page_id.clone() else {
            return;
        };
        let text = self.comments.input.text.trim().to_owned();
        if text.is_empty() {
            self.comments.error = Some("Enter a comment.".into());
            return;
        }
        let result = if let Some(id) = self.comments.reply.as_deref() {
            self.store
                .reply_comment(&self.workspace.id, id, "You", &text)
        } else {
            let anchor = self.comments.anchor.clone().unwrap_or(CommentAnchor::Page);
            self.store
                .create_comment(&self.workspace.id, &page_id, anchor, "You", &text)
                .map(|_| ())
        };
        match result {
            Ok(()) => {
                self.comments.input.clear();
                self.comments.input.focused = false;
                self.comments.anchor = None;
                self.comments.reply = None;
                self.comments.error = None;
                self.load_comments(&page_id);
            }
            Err(error) => self.comments.error = Some(format!("Comment was not saved: {error}")),
        }
    }

    pub fn comment_resolved(&mut self, id: &str, resolved: bool) {
        match self.store.resolve_comment(&self.workspace.id, id, resolved) {
            Ok(()) => {
                if let Some(page_id) = self.comments.page_id.clone() {
                    self.load_comments(&page_id);
                }
            }
            Err(error) => self.comments.error = Some(format!("Could not update comment: {error}")),
        }
    }
}

impl State {
    pub fn set_agent_presence(&mut self, page_id: &str, agents: Vec<(String, String)>) {
        if agents.is_empty() {
            self.agent_presence.remove(page_id);
        } else {
            self.agent_presence.insert(page_id.to_owned(), agents);
        }
        if self.presence_open.as_ref().is_some_and(|id| {
            !self
                .agent_presence
                .values()
                .flatten()
                .any(|(session, _)| session == id)
        }) {
            self.presence_open = None;
        }
    }
}
