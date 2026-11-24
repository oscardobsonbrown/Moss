use crate::state::{Modal, State};
use moss_core::Workspace;
use rune::{TextEdit, Tx};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsSection {
    #[default]
    Preferences,
    Connections,
    General,
    ImportExport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsDropdown {
    Appearance,
    WeekStart,
    WorkspaceIcon,
}

pub struct SettingsState {
    pub section: SettingsSection,
    pub dropdown: Option<SettingsDropdown>,
    pub workspace_name: TextEdit,
    pub workspace_name_error: Option<String>,
    pub icon_search: TextEdit,
    pub agent_feedback: Option<&'static str>,
    pub general_feedback: Option<&'static str>,
    pub codex_preview: Option<Result<String, &'static str>>,
    pub json_preview: Option<Result<String, &'static str>>,
    pub data_path_display: Option<String>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            section: SettingsSection::Preferences,
            dropdown: None,
            workspace_name: TextEdit::new(""),
            workspace_name_error: None,
            icon_search: TextEdit::new(""),
            agent_feedback: None,
            general_feedback: None,
            codex_preview: None,
            json_preview: None,
            data_path_display: None,
        }
    }
}

impl State {
    pub fn open_settings(&mut self) {
        self.settings.dropdown = None;
        self.settings.agent_feedback = None;
        self.settings.general_feedback = None;
        self.settings.workspace_name_error = None;
        self.settings.workspace_name = TextEdit::new(&self.workspace.name);
        self.refresh_settings_cache();
        self.modal = Some(Modal::Settings);
    }

    pub fn close_settings(&mut self) {
        self.settings.dropdown = None;
        self.settings.agent_feedback = None;
        self.settings.general_feedback = None;
        self.settings.workspace_name_error = None;
        self.settings.workspace_name.focused = false;
        self.settings.icon_search.focused = false;
        if self.modal == Some(Modal::Settings) {
            self.modal = None;
        }
    }

    pub fn select_settings_section(&mut self, section: SettingsSection) {
        self.settings.section = section;
        self.settings.dropdown = None;
        self.settings.agent_feedback = None;
        self.settings.general_feedback = None;
        self.settings.workspace_name_error = None;
        if section == SettingsSection::General {
            self.settings.workspace_name = TextEdit::new(&self.workspace.name);
            self.refresh_settings_cache();
        } else if section == SettingsSection::Connections {
            self.refresh_settings_cache();
        }
    }

    fn refresh_settings_cache(&mut self) {
        self.settings.codex_preview = Some(agent_setup_text(&self.path, true));
        self.settings.json_preview = Some(agent_setup_text(&self.path, false));
        self.settings.data_path_display = Some(
            std::path::absolute(&self.path)
                .unwrap_or_else(|_| self.path.clone())
                .display()
                .to_string(),
        );
    }

    pub fn toggle_settings_dropdown(&mut self, dropdown: SettingsDropdown) {
        if self.settings.dropdown == Some(dropdown) {
            self.settings.dropdown = None;
            self.settings.icon_search.focused = false;
        } else {
            self.settings.dropdown = Some(dropdown);
            if dropdown == SettingsDropdown::WorkspaceIcon {
                self.settings.icon_search = TextEdit::new("");
                self.settings.icon_search.focused = true;
            }
        }
    }

    pub fn dismiss_settings_dropdown(&mut self) {
        self.settings.dropdown = None;
        self.settings.icon_search.focused = false;
    }

    pub fn save_workspace_name(&mut self) -> bool {
        let trimmed = self.settings.workspace_name.text.trim().to_owned();
        if trimmed.is_empty() {
            self.settings.workspace_name_error = Some("Workspace name cannot be empty.".into());
            return false;
        }
        if trimmed == self.workspace.name {
            self.settings.workspace_name_error = None;
            return true;
        }
        match self
            .store
            .update_workspace(&self.workspace.id, &trimmed, &self.workspace.icon)
        {
            Ok(updated) => {
                self.settings.workspace_name = TextEdit::new(&updated.name);
                self.settings.workspace_name_error = None;
                self.settings.general_feedback = Some("Workspace name saved.");
                self.apply_workspace_update(updated);
                true
            }
            Err(error) => {
                self.settings.workspace_name_error =
                    Some(format!("Could not save workspace name: {error}"));
                false
            }
        }
    }

    pub fn set_workspace_icon(&mut self, icon: &str) -> bool {
        match self
            .store
            .update_workspace(&self.workspace.id, &self.workspace.name, icon)
        {
            Ok(updated) => {
                self.dismiss_settings_dropdown();
                self.settings.general_feedback = Some("Workspace icon updated.");
                self.apply_workspace_update(updated);
                true
            }
            Err(error) => {
                self.error = Some(format!("Could not save workspace icon: {error}"));
                false
            }
        }
    }

    fn apply_workspace_update(&mut self, updated: Workspace) {
        let id = updated.id.clone();
        self.workspace = updated.clone();
        if let Ok(list) = self.store.list_workspaces() {
            self.workspaces = list;
        } else if let Some(entry) = self.workspaces.iter_mut().find(|w| w.id == id) {
            *entry = updated.clone();
        }
        if let Ok(handles) = self.handles.lock() {
            for (window_id, handle) in handles.iter() {
                if window_id == &id {
                    continue;
                }
                let updated = updated.clone();
                handle.update(move |s, _| {
                    if let Ok(list) = s.store.list_workspaces() {
                        s.workspaces = list;
                    } else if let Some(entry) = s.workspaces.iter_mut().find(|w| w.id == updated.id)
                    {
                        *entry = updated;
                    }
                });
            }
        }
    }

    pub fn copy_agent_setup(&mut self, tx: &mut Tx, codex: bool) {
        let feedback = match agent_setup_text(&self.path, codex) {
            Ok(text) => {
                tx.copy(text);
                "Copied to the clipboard."
            }
            Err(message) => message,
        };
        self.settings.agent_feedback = Some(feedback);
    }

    pub fn copy_workspace_id(&mut self, tx: &mut Tx) {
        tx.copy(self.workspace.id.clone());
        self.settings.general_feedback = Some("Workspace ID copied.");
    }

    pub fn copy_data_path(&mut self, tx: &mut Tx) {
        let path = std::path::absolute(&self.path)
            .unwrap_or_else(|_| self.path.clone())
            .display()
            .to_string();
        tx.copy(path);
        self.settings.general_feedback = Some("Data folder path copied.");
    }
}

pub fn agent_setup_text(data: &Path, codex: bool) -> Result<String, &'static str> {
    let command = std::env::var_os("MOSS_MCP_ADAPTER")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .map(|path| path.with_file_name("moss-mcp"))
        })
        .filter(|path| path.is_file())
        .ok_or("The Moss MCP adapter is missing. Reinstall Moss or run cargo build -p moss-mcp in the development checkout.")?;
    let data = std::path::absolute(data).map_err(|_| "Could not find the Moss data folder.")?;
    let (Some(command), Some(data)) = (command.to_str(), data.to_str()) else {
        return Err("The app or data folder path is not valid UTF-8.");
    };
    if codex {
        let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
        Ok(format!(
            "codex mcp add moss --env {} -- {}",
            quote(&format!("MOSS_DATA_DIR={data}")),
            quote(command),
        ))
    } else {
        let settings = serde_json::json!({
            "mcpServers": { "moss": { "command": command, "env": { "MOSS_DATA_DIR": data } } }
        });
        Ok(format!("{settings:#}"))
    }
}

fn installed_bundle_version() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let bundle = exe.ancestors().nth(3)?;
    if bundle.extension().and_then(|ext| ext.to_str()) == Some("app")
        && exe == bundle.join("Contents/MacOS/Moss")
    {
        moss_updater::bundle::installed_version(bundle).map(moss_updater::version::format_version)
    } else {
        None
    }
}

pub fn app_version() -> &'static str {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION.get_or_init(|| {
        installed_bundle_version()
            .or_else(|| option_env!("MOSS_VERSION").map(str::to_owned))
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned())
    })
}

pub fn version_string() -> String {
    format!(
        "Moss {}{}",
        app_version(),
        if cfg!(debug_assertions) {
            " · Local dev"
        } else {
            ""
        }
    )
}

#[cfg(test)]
mod version_tests {
    use super::{app_version, version_string};

    #[test]
    fn runtime_version_falls_back_outside_app_bundle() {
        assert!(!app_version().is_empty());
        assert!(version_string().starts_with(&format!("Moss {}", app_version())));
    }
}
