# Moss

A local workspace app for macOS, built with AnomalyCo's Rune UI framework and SQLite. No account or server is needed to open or edit a page.

The first working build has saved pages and rich text blocks, tabs, search, Trash with restore, light and dark themes, and table, board, calendar, timeline, gallery and list layouts. Database records can be created and edited. The computation crate has formula, rollup, filter and dependency functions. Full Notion compatibility is still in progress.

## Run

Install Rust, then run:

```sh
cargo run -p moss
```

Data is saved in `~/Library/Application Support/Moss`. Set `MOSS_DATA_DIR` to use a separate folder. To create the five example workspaces in a new data folder:

```sh
MOSS_DATA_DIR=/tmp/moss-demo MOSS_DEMO=1 cargo run -p moss
```

Build the macOS app with `scripts/package-macos.sh`. Local builds create `dist/Moss.app` and replace `/Applications/Moss.app`; restart Moss to use the new build. CI packages the app without installing it. The Moss icon uses the supplied reference and a Sentient-style M. It uses an ad hoc signature; it is not a signed or notarized public release.

## Releases

Each push to `main`, including a merged pull request, runs the release workflow. It tests the workspace, increments the latest stable minor version, builds an Apple Silicon macOS archive, signs `latest.json` with Ed25519, and publishes both files to [GitHub Releases](https://github.com/oscardobsonbrown/Moss/releases). With no earlier tag, the workspace version is the baseline. A retry reuses the tag for the same commit. The app archive uses an ad hoc signature and is not notarized.

Release builds with an embedded update public key check the static `latest.json` manifest at launch and every six hours (with `ETag` caching). Verified updates stage in `~/Library/Caches/app.moss.desktop/updates/` and persist across restarts. Download progress appears in Software update settings. Normal Quit (`Cmd+Q`) closes Moss without installing. Choose **Restart to install** to run the detached update helper, which waits for Moss to exit, swaps `Moss.app` atomically with rollback on failure, and relaunches Moss. Workspace data stays in its current folder.

Install the first release with updater support manually. Update checks use public release downloads and require no GitHub account or GitHub API quota. To disable update checks for a launch, set `MOSS_DISABLE_UPDATES=1`.

### Configure automatic release signing

Generate an Ed25519 signing keypair once:

```sh
cargo run -p moss-updater --bin moss-update-helper -- keygen
```

Add `MOSS_UPDATE_SIGNING_KEY` as a GitHub Actions repository secret. The release workflow derives `MOSS_UPDATE_PUBLIC_KEY` from that secret automatically, embeds the public key into Moss, signs `latest.json` during the release job, and verifies the manifest against the archive before publishing. Keep the same signing key across future releases so installed copies can verify new manifests.

To sign and notarize macOS releases for Gatekeeper, also set `APPLE_CERTIFICATE_P12_B64`, `APPLE_CERTIFICATE_PASSWORD`, and `APPLE_SIGNING_IDENTITY`, plus either App Store Connect API credentials (`APPLE_NOTARY_KEY_P8_B64`, `APPLE_NOTARY_KEY_ID`, `APPLE_NOTARY_ISSUER_ID`) or Apple ID notarization credentials (`APPLE_ID`, `APPLE_APP_PASSWORD`, `APPLE_TEAM_ID`). Without those secrets, CI falls back to ad hoc signing.

### Test updates locally

In non-release builds, you can test the full update cycle against a local manifest (`file://` or `http://127.0.0.1`) by setting `MOSS_UPDATE_MANIFEST_URL`, `MOSS_UPDATE_PUBLIC_KEY`, and optionally `MOSS_UPDATE_CURRENT_VERSION`, `MOSS_UPDATE_TARGET_APP`, and `MOSS_UPDATE_CACHE_DIR`.

## Hot reload

Use Python 3.13 or later and Dioxus CLI **0.7.10** (`cargo install dioxus-cli --version 0.7.10 --locked --jobs 2`). Then run:

```sh
scripts/dev.sh
```

The command uses the launcher from Moss's pinned Rune version. Saved function-body edits in `src/ui/**/*.rs` and `src/rich_text.rs` update the running app. Live patches keep app state and scroll positions. The first build takes longer; later patches reuse the compiled framework. Builds use two compiler jobs.

Other source files, storage code, and assets cause a rebuild and restart. Declaration, import, constant, and signature changes in a patch file require a restart; stop the command with Ctrl-C and run it again when Rune reports `restart needed`. Unsaved edits can be lost on restart. Use normal builds for performance tests.

Development uses `target/dev-data` with example workspaces by default. Your normal workspace is separate. Set `MOSS_DATA_DIR` to use another folder, `MOSS_DEMO=0` to start without examples, or `RUNE_DX` to select the Dioxus executable. The development server listens on `127.0.0.1:18745`. Hot reload is off in normal and release builds.

Drag the sidebar's right edge to change its width. Tabs and page content follow the divider. The width is saved for each workspace. You can also focus the divider and use the arrow keys, Home, and End.

## Typography

Geist Sans is bundled for the UI. Geist Mono is bundled for code, including inline code. Use the page’s **… → Appearance & layout** menu to select Default (Geist Sans), Serif (locally installed Sentient, or bundled Geist Serif when absent), or Mono (Geist Mono). Small text and Full width are also saved per page. These settings survive reopen and archive restore. No font download is needed at runtime. See the [font notes](crates/moss-app/assets/fonts/README.md) for the optional Sentient files and license.

Interface icons use bundled Phosphor Regular SVGs, rendered directly by Rune. React and icon fonts are not required. User-selected page emoji stay separate. See the [icon source and MIT notice](crates/moss-app/assets/phosphor/README.md).

The fonts retain their SIL Open Font License. See [font sources and notices](crates/moss-app/assets/fonts/README.md).

## Check

```sh
cargo test --workspace --jobs 2
cargo run --release -p moss-core --example storage_bench -- --small --path /tmp/moss-bench.sqlite
```

Omit `--small` to prepare 100,000 pages and 1,000,000 database rows. The probe measures storage reads with a warm OS cache. It does not prove the content-open or cold-start limits.

## Project records

- [PRD](docs/PRD.md)
- [Decisions](docs/DECISIONS.md)
- [Build plan](docs/BUILD-PLAN.md)
- [Performance contract](docs/PERFORMANCE.md)
- [Acceptance workflows](docs/ACCEPTANCE-WORKFLOWS.md)
- [Implementation status](docs/IMPLEMENTATION.md)
- [Paper design project](https://app.paper.design/file/01M6TJR715EZ6FZ0K7K6ZRAFNC)
- [Paper desktop screen inventory](docs/PAPER-SCREENS.md)
- [Paper component inventory](docs/PAPER-COMPONENTS.md)
- [Reference coverage and open checks](docs/REFERENCE-COVERAGE.md)

Private Notion reference captures stay outside Git and release files. The project uses the MIT license. The name, icons and release assets will be reviewed before public distribution.

## Local agent access

1. Open **Settings → Connect an agent…**. Turn on **Local agent access** and keep Moss running. Access is off by default.
2. For Codex CLI, choose **Copy Codex CLI command** and run the command in Terminal. For a client that uses `mcpServers` JSON, choose **Copy MCP settings as JSON** and add the `moss` entry to the client's MCP configuration.
3. Start a new session in the client and ask it to list your Moss workspaces. Ask it to read a test page to see the agent badge in the page header.

Connected local clients can read and change workspace data, including workspaces with closed windows. Enable access for clients you trust. Turn off **Local agent access** to disconnect clients. Moss uses a private local socket and token and does not open a network port.

### Manual setup

For Codex CLI and Moss installed in Applications, run:

```sh
codex mcp add moss -- /Applications/Moss.app/Contents/MacOS/moss-mcp
```

For a client that uses `mcpServers` JSON, use:

```json
{
  "mcpServers": {
    "moss": {
      "command": "/Applications/Moss.app/Contents/MacOS/moss-mcp",
      "env": { "MOSS_AGENT_NAME": "My agent" }
    }
  }
}
```

Use the absolute path to the adapter if you moved Moss. For a development build, run `cargo build -p moss-mcp` and use the absolute path to `target/debug/moss-mcp`. If Moss uses a custom data folder, set the same `MOSS_DATA_DIR` in the client configuration. The copy buttons in Moss include the paths for the current instance.

If the client cannot connect, check that Moss is running and **Local agent access** is on. Check the adapter path, then restart the client to reconnect. The adapter reads its token from the local data folder. Keep the token out of client configuration.

### Available tools

Available tools are `list_workspaces`, `find_pages`, `read_page`, `status`, `create_page`, `rename_page`, `list_trash`, `trash_page`, `restore_page`, `edit_block`, `edit_cell`, `list_comments`, `create_comment`, `reply_comment`, `resolve_comment`, and `page_history`. Use workspace and page IDs from discovery. `read_page` returns stored blocks, visible draft content, unsaved status, and active text edits, including composition data when present. `edit_block` requires the complete expected stored block and only changes plain text; it rejects changes when a conflicting draft is open. `edit_cell` also checks the expected value. Read again and retry if a value changed.

The service does not yet provide all desktop actions. It cannot permanently delete pages, change rich text or images, edit tables, manage database structure, or restore history. A page subtree must be closed in Moss before an agent moves it to Trash. Rename checks the expected saved title in a transaction. Trash and restore check the expected page before the operation; this check does not guard against a separate process changing the database at the same time. It does not provide live cursor positions for agents or multiplayer sync between computers. Agent presence shows the last page that the session accessed and clears on disconnect or timeout.
