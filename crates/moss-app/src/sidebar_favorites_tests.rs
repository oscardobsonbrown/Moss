use super::*;
use moss_core::{PageKind, new_id};
use std::path::PathBuf;

fn fixture() -> (State, PathBuf) {
    let path = std::env::temp_dir().join(format!("moss-favorites-test-{}", new_id()));
    (State::open(path.clone(), None).unwrap(), path)
}

#[test]
fn favorites_persist_per_workspace_and_ignore_missing_or_trashed_pages() {
    let (mut state, path) = fixture();
    let workspace = state.workspace.id.clone();
    let local = state
        .store
        .create_page(&workspace, None, "Local", "🌱", PageKind::Document)
        .unwrap();
    let other = state.store.create_workspace("Other", "").unwrap();
    let foreign = state
        .store
        .create_page(&other.id, None, "Foreign", "", PageKind::Document)
        .unwrap();
    assert!(!state.toggle_sidebar_favorite(&foreign.id));
    assert!(!state.is_sidebar_favorite(&foreign.id));
    assert!(state.toggle_sidebar_favorite(&local.id));
    assert!(state.is_sidebar_favorite(&local.id));
    assert_eq!(
        state
            .sidebar_favorites
            .pages
            .iter()
            .map(|p| &p.id)
            .collect::<Vec<_>>(),
        vec![&local.id]
    );
    {
        let mut shared = state.shared_preferences.lock().unwrap();
        shared.favorite_pages.get_mut(&workspace).unwrap().extend([
            foreign.id.clone(),
            local.id.clone(),
            "missing".to_owned(),
        ]);
    }
    assert!(state.refresh_sidebar_favorites());
    assert_eq!(
        state
            .sidebar_favorites
            .pages
            .iter()
            .map(|p| &p.id)
            .collect::<Vec<_>>(),
        vec![&local.id]
    );
    // Remove the invalid fixture IDs before the reopen check.
    state
        .shared_preferences
        .lock()
        .unwrap()
        .favorite_pages
        .insert(workspace.clone(), vec![local.id.clone()]);
    drop(state);
    let mut reopened = State::open(path.clone(), Some(&workspace)).unwrap();
    assert!(reopened.refresh_sidebar_favorites());
    assert_eq!(reopened.sidebar_favorites.pages[0].id, local.id);
    reopened.store.delete_page(&local.id).unwrap();
    assert!(reopened.refresh_sidebar_favorites());
    assert!(reopened.sidebar_favorites.pages.is_empty());
    assert!(reopened.is_sidebar_favorite(&local.id));
    assert!(reopened.toggle_sidebar_favorite(&local.id));
    assert!(!reopened.is_sidebar_favorite(&local.id));
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn failed_save_and_limit_keep_the_previous_favorites() {
    let (mut state, path) = fixture();
    let page = state.tab().unwrap().page.clone();
    let temp = path.join("preferences.json.tmp");
    std::fs::create_dir(&temp).unwrap();
    assert!(!state.toggle_sidebar_favorite(&page.id));
    assert!(!state.is_sidebar_favorite(&page.id));
    assert!(state.sidebar_favorites.pages.is_empty());
    assert!(
        state
            .error
            .as_deref()
            .is_some_and(|error| error.contains("could not be saved"))
    );
    std::fs::remove_dir(temp).unwrap();
    {
        let mut shared = state.shared_preferences.lock().unwrap();
        shared.favorite_pages.insert(
            state.workspace.id.clone(),
            (0..MAX_FAVORITES).map(|i| format!("missing-{i}")).collect(),
        );
    }
    assert!(!state.toggle_sidebar_favorite(&page.id));
    assert!(!state.is_sidebar_favorite(&page.id));
    assert!(
        state
            .error
            .as_deref()
            .is_some_and(|error| error.contains("full"))
    );
    drop(state);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn shared_preferences_keep_favorites_separate_between_workspaces() {
    let (mut first, path) = fixture();
    let first_workspace = first.workspace.id.clone();
    let first_page = first.tab().unwrap().page.id.clone();
    let other = first.store.create_workspace("Other", "").unwrap();
    let other_page = first
        .store
        .create_page(&other.id, None, "Other page", "", PageKind::Document)
        .unwrap();
    let mut second = State::open(path.clone(), Some(&other.id)).unwrap();
    second.shared_preferences = first.shared_preferences.clone();
    assert!(first.toggle_sidebar_favorite(&first_page));
    assert!(second.toggle_sidebar_favorite(&other_page.id));
    assert!(first.is_sidebar_favorite(&first_page));
    assert!(!first.is_sidebar_favorite(&other_page.id));
    assert!(second.is_sidebar_favorite(&other_page.id));
    assert!(!second.is_sidebar_favorite(&first_page));
    drop(first);
    drop(second);
    let reopened = State::open(path.clone(), Some(&first_workspace)).unwrap();
    assert!(reopened.is_sidebar_favorite(&first_page));
    assert_eq!(
        reopened.shared_preferences.lock().unwrap().favorite_pages[&other.id],
        vec![other_page.id]
    );
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}
