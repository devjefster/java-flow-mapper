# Agent Notes

## Repo Shape
- Work from this directory (`jfm/`); the parent `java-flow-mapper/` is not the git/Cargo repo.
- This is a Rust workspace; the `jfm` binary lives in `crates/jfm-cli`.
- `demo-api/demo` is a Spring Boot fixture parsed by `jfm`; Rust tests do not need to build or run the Java app.
- The sibling `../vault/java-flow-mapper/` docs contain product specs and roadmap notes, but trust `Cargo.toml`, `src/**`, and `tests/**` for implemented behavior.

## Commands
- Format check: `cargo fmt --check`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Full tests: `cargo test`
- Focused integration test: `cargo test -p jfm-cli --test flow_demo flow_get_users_by_id_renders_expected_markdown`
- Cache the demo index: `cargo run -- index demo-api/demo`
- List cached entrypoints: `cargo run -- entrypoints demo-api/demo`
- Inspect cached index health: `cargo run -- doctor demo-api/demo`
- Run the main fixture: `cargo run -- flow demo-api/demo "GET /users/{id}"`
- Run alternate formats: `cargo run -- flow demo-api/demo "GET /users/{id}" --format json` or `--format mermaid`
- Debug parser/indexing behavior: `RUST_LOG=jfm=debug cargo run -- flow demo-api/demo "GET /users/{id}"`

## CLI Reality Check
- Implemented subcommands are `flow`, `index`, `entrypoints`, and `doctor`.
- `query` appears in roadmap docs but is not implemented; the SurrealDB store is currently a `ProjectIndex` cache, not a first-class queryable graph schema.
- Commands that accept a project root default to the current directory when the root is omitted; `flow` accepts either `flow <root> "<VERB> <PATH>"` or `flow "<VERB> <PATH>"` from the project root.
- Endpoint selectors must be exactly two whitespace-separated parts, e.g. `GET /users/{id}`.

## Testing And Snapshots
- `crates/jfm-cli/tests/flow_demo.rs` uses `assert_cmd::Command::cargo_bin("jfm")` against `demo-api/demo` and `insta` snapshots under `crates/jfm-cli/tests/snapshots/`.
- Snapshot filters normalize absolute demo paths and line numbers for Markdown/JSON; preserve those filters when adding output assertions.
- To intentionally refresh snapshots, use `INSTA_UPDATE=always cargo test`, then review the `.snap` diffs.

## Implementation Gotchas
- `parser::index_project` manually skips `target`, `build`, `node_modules`, `.git`, `.idea`, `.mvn`, and `.gradle`; it does not currently use `.gitignore` semantics.
- The resolver has a hard graph-construction recursion cap (`flow::MAX_DEPTH = 8`) that is separate from render-time `--max-depth`.
- Markdown and Mermaid default to render depth 5 when `--max-depth` is omitted; JSON is unlimited unless `--max-depth` is passed.
- Spring Data repositories extending `JpaRepository`, `CrudRepository`, or `PagingAndSortingRepository` get synthesized/inherited method handling in `crates/jfm-spring/src/jpa.rs` and flow expansion/resolution crates.

## Local Instructions
- `RULES.md` and `CODEBASE_REASONING_TOPOLOGY.md` are repo-local agent guidance; preserve their caution/simplicity/surgical-change bias.
- `WORKLOG.md` records verified command sets and known deferrals from prior slices; check it before assuming a missing feature is accidental.
