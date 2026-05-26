# Agent Notes

## Repo Shape
- Work from this directory (`jfm/`); the parent `java-flow-mapper/` is not the git/Cargo repo.
- This is currently a single Rust binary crate (`Cargo.toml`) named `jfm`, not the future multi-crate layout described in the vault docs.
- `demo-api/demo` is a Spring Boot fixture parsed by `jfm`; Rust tests do not need to build or run the Java app.
- The sibling `../vault/java-flow-mapper/` docs contain product specs and roadmap notes, but trust `Cargo.toml`, `src/**`, and `tests/**` for implemented behavior.

## Commands
- Format check: `cargo fmt --check`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Full tests: `cargo test`
- Focused integration test: `cargo test --test flow_demo flow_get_users_by_id_renders_expected_markdown`
- Run the main fixture: `cargo run -- flow demo-api/demo "GET /users/{id}"`
- Run alternate formats: `cargo run -- flow demo-api/demo "GET /users/{id}" --format json` or `--format mermaid`
- Debug parser/indexing behavior: `RUST_LOG=jfm=debug cargo run -- flow demo-api/demo "GET /users/{id}"`

## CLI Reality Check
- The only implemented subcommand is `jfm flow <root> "<VERB> <PATH>" [--format markdown|json|mermaid] [--max-depth N]`.
- `index`, `entrypoints`, `query`, and `doctor` appear in roadmap docs but are not implemented in `src/cli.rs`.
- Endpoint selectors must be exactly two whitespace-separated parts, e.g. `GET /users/{id}`.

## Testing And Snapshots
- `tests/flow_demo.rs` uses `assert_cmd::Command::cargo_bin("jfm")` against `demo-api/demo` and `insta` snapshots under `tests/snapshots/`.
- Snapshot filters normalize absolute demo paths and line numbers for Markdown/JSON; preserve those filters when adding output assertions.
- To intentionally refresh snapshots, use `INSTA_UPDATE=always cargo test`, then review the `.snap` diffs.

## Implementation Gotchas
- `parser::index_project` manually skips `target`, `build`, `node_modules`, `.git`, `.idea`, `.mvn`, and `.gradle`; it does not currently use `.gitignore` semantics.
- The resolver has a hard graph-construction recursion cap (`flow::MAX_DEPTH = 8`) that is separate from render-time `--max-depth`.
- Markdown and Mermaid default to render depth 5 when `--max-depth` is omitted; JSON is unlimited unless `--max-depth` is passed.
- Spring Data repositories extending `JpaRepository`, `CrudRepository`, or `PagingAndSortingRepository` get synthesized/inherited method handling in `src/spring/jpa.rs` and `src/flow.rs`.
- `src/model.rs` allows `dead_code` because some output-contract variants are defined ahead of implementation slices.

## Local Instructions
- `RULES.md` and `CODEBASE_REASONING_TOPOLOGY.md` are repo-local agent guidance; preserve their caution/simplicity/surgical-change bias.
- `WORKLOG.md` records verified command sets and known deferrals from prior slices; check it before assuming a missing feature is accidental.
