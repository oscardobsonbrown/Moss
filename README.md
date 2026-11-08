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

Build the local macOS app bundle `dist/Moss.app` with `scripts/package-macos.sh`. The Moss icon uses the supplied reference and a Sentient-style M. It uses an ad hoc signature; it is not a signed or notarized public release.

## Releases

Each push to `main`, including a merged pull request, runs the release workflow. It tests the workspace, increments the latest stable minor version, and creates a GitHub release with an Apple Silicon macOS archive. With no earlier tag, the workspace version is the baseline. A retry reuses the tag for the same commit. The app archive uses an ad hoc signature and is not notarized. Releases remain private while the repository is private.

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
