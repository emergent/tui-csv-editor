# Repository Guidelines

## Project Structure & Module Organization
- `Cargo.toml`: crate metadata and dependencies (`anyhow`, `ratatui`).
- `Cargo.lock`: resolved dependency versions (committed).
- `src/main.rs`: binary entry point. Add modules as the app grows (e.g., `src/ui/`, `src/app/`, `src/csv/`).
- `target/`: build artifacts (gitignored). Create `tests/` for integration tests when needed.

## Build, Test, and Development Commands
- Build: `cargo build` — compiles debug build to `target/debug/`.
- Run: `cargo run -- [args]` — runs the TUI locally (e.g., `cargo run -- sample.csv`).
- Test: `cargo test` — runs unit/integration tests.
- Lint: `cargo clippy -- -D warnings` — lints and fails on warnings.
- Format: `cargo fmt --all` — formats code with rustfmt.

## Coding Style & Naming Conventions
- Formatting: rustfmt defaults; run before pushing. Prefer small, focused modules.
- Naming: `snake_case` for functions/modules, `PascalCase` for types/traits, `SCREAMING_SNAKE_CASE` for consts.
- Errors: return `anyhow::Result<T>` from fallible functions; avoid `unwrap()`/`expect()` in non-test code.
- UI: isolate terminal drawing in a `ui` module; keep state and I/O separate for testability.

## Testing Guidelines
- Unit tests: colocate in files under `#[cfg(test)] mod tests { ... }`.
- Integration tests: place in `tests/*.rs` using public API of the crate.
- Naming: describe behavior (e.g., `renders_empty_table`, `parses_quoted_cells`).
- Run: `cargo test` (use `-- --nocapture` to see stdout). Aim to cover parsing, state transitions, and key UI behaviors.

## Commit & Pull Request Guidelines
- Commits: use Conventional Commits style (e.g., `feat: add table scrolling`, `fix: handle empty CSV`). Keep commits small and scoped.
- PRs: include a clear description, reproduction/verification steps (commands to run), and terminal screenshots/gifs when UI changes. Link related issues.
- CI/readiness: ensure `cargo fmt`, `clippy`, and tests pass locally before requesting review.

## Security & Configuration Tips
- Handle untrusted CSV input defensively; validate sizes and avoid panics.
- Always restore terminal state on error paths; prefer scoped terminal guards.
- Keep dependencies minimal and pinned via `Cargo.lock`.
