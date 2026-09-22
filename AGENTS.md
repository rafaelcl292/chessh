# Agent instructions

- Follow the existing Rust style and keep changes focused on the task.
- Preserve keyboard and mouse navigation, compact terminal layouts, and multiplayer behavior.
- Update the README when commands, controls, or configuration change.
- Use Conventional Commits (`feat:`, `fix:`, `docs:`, etc.). Add a body when useful to explain behavior, motivation, and validation; follow recent commits.
- Before committing code, run `cargo fmt --check`, `cargo test --locked`, and `cargo clippy --all-targets --locked -- -D warnings`.
- For deployment, follow `docs/deployment.md` and `.local/deploy/README.md`. Build locally, preserve persistent state, and verify the public SSH flow.
- Never commit secrets or machine-local files from `.local/`.
