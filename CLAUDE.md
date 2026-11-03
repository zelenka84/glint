# Glint: notes for Claude

Glint is a pixel-style Minecraft: Java Edition launcher built with Tauri 2, Rust and React 19. The UI is in English. The user writes in Russian; reply in Russian.

## UI work: read the design system, don't re-study the code

Before any UI change, read `docs/DESIGN.md` and build from it directly. It covers the tokens, components (`B3D`, `Modal`, `Icon`, `EmptyState` and others), ready CSS classes, motion recipes, screen patterns and a checklist. Open existing files only to find the place you are changing. Add new styles as a new `/* ---- name ---- */` section at the end of `apps/desktop/src/App.css`. If you add a reusable component or pattern, add it to `docs/DESIGN.md` too.

Architecture and roadmap are in `docs/ARCHITECTURE.md`.

## Language of files

Everything in the repository is in English: code, comments, docs, README, commit messages. Russian copies of docs sit next to them with a `_RUS` suffix (`README_RUS.md`, `docs/DESIGN_RUS.md`, `docs/ARCHITECTURE_RUS.md`); `*_RUS.*` is in `.gitignore`. When you change an English doc, update its `_RUS` copy too.

## Commands

- Checks: `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cd apps/desktop && npx tsc --noEmit`.
- Install the user's Dev build: `powershell -ExecutionPolicy Bypass -File scripts\install-local.ps1`. It fails while Glint is running, so ask the user to close it first.
- Channel installers: `scripts\build.ps1 -Channel dev|beta|public`. Build them only when the user asks.

## Rules

- Commit only when the user says so. Use the repo's own git identity (zelenka84) with no `-c user.name/user.email` overrides, and end messages with the `Co-Authored-By` line.
- Keep the Tauri layer thin: logic goes in `crates/*`, with tests there.
- Channels: Dev (`dev` feature) has experimental features and offline names; Beta (`beta`) has offline names and no experiments; Public has neither. Offline names and Ely.by in Public need Microsoft ownership verification. Never add an offline bypass to Public builds, and never use borrowed or official Microsoft/Mojang client IDs.
- Leave the untracked `brand/` folder alone.
