# Rotion

A local workspace app for macOS, built with AnomalyCo's Rune UI framework and SQLite. No account or server is needed to open or edit a page.

The first working build has saved pages and rich text blocks, tabs, search, Trash with restore, light and dark themes, and table, board, calendar, timeline, gallery and list layouts. Database records can be created and edited. The computation crate has formula, rollup, filter and dependency functions. Full Notion compatibility is still in progress.

## Run

Install Rust, then run:

```sh
cargo run -p rotion
```

Data is saved in `~/Library/Application Support/Rotion`. Set `ROTION_DATA_DIR` to use a separate data folder. To create the five example workspaces in a new data folder:

```sh
ROTION_DATA_DIR=/tmp/rotion-demo ROTION_DEMO=1 cargo run -p rotion
```

Build a local macOS app bundle with `scripts/package-macos.sh`. It uses an ad hoc signature; it is not a signed or notarized public release.

## Typography

Geist Sans is bundled for the UI. Geist Mono is bundled for code, including inline code. Use the page’s **… → Appearance & layout** menu to select Default (Geist Sans), Serif (Geist Serif), or Mono (Geist Mono). Small text and Full width are also saved per page. These settings survive reopen and archive restore. No font download is needed at runtime.

The fonts retain their SIL Open Font License. See [font sources and notices](crates/rotion-app/assets/fonts/README.md).

## Check

```sh
cargo test --workspace --jobs 2
cargo run --release -p rotion-core --example storage_bench -- --small --path /tmp/rotion-bench.sqlite
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

Private Notion reference captures stay outside Git and release files. The project uses the MIT license. The name, icons and release assets will be reviewed before public distribution.
