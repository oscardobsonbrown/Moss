# Rune source review

Date: 2026-11-07.

Official repository: https://github.com/anomalyco/rune

Observed `dev` revision: `f3d04f3f2069d177d04c963b0ba9f6b2065e46eb`.

The app now pins this revision as its Rune dependency. The first native editor and database layouts use it.

## Source observations

- Rune is a native GPU-rendered Rust UI framework. Its README lists macOS, Windows, Linux, and web targets.
- Rune is pre-release. Its README requires a pinned revision.
- Its runtime has headless image output and element inspection. These can support later state checks.
- The framework README documents `App::keep_running` and a runtime that retains state, scroll, and GPU resources after window close. On macOS, closing hides the window and reopening restores it. This matches the resident-app direction. It does not prove the required reopen timing.
- The framework README describes rich text rendering and text input with selection, clipboard, undo, and IME composition.
- Rich text rendering and text-field support do not prove that a complete Notion-style block editor is ready to use. Editor behavior still needs a source review and a focused prototype after requirements.

## Local reference files

- `rune-readme.md`: framework README downloaded from the official `dev` branch.
- `hello-main.rs`: official starter example downloaded from the same branch.

These source files are research material. They are not Moss implementation.

## Risks to test

- The framework's launch notes report about 300 ms for a small app's first window with content on Apple silicon. This is a reported source result, not a Moss measurement. The 8.8 ms content-open requirement needs an explicit decision about launch from a stopped process. Source: the framework README, `Launching fast`.
- Rune's default macOS font smoothing differs from its documented browser-style mode. Exact Notion text matching requires a comparison of the font, scale, smoothing, and blend settings. Do not assume default Rune text matches the reference.

- Text shaping, selection, IME, bidirectional text, rich paste, and undo across blocks.
- Keyboard focus and accessibility for a GPU-rendered interface.
- Large documents, virtual lists, database views, and frame cost.
- Static state rendering with no default springs or transition delays on frequent actions.

Do not select a sync framework for v1. The user chose local-only storage.
