# Java Flow Mapper

`jfm` is a Rust CLI for mapping Java/Spring HTTP request flows. It indexes a Java project, finds a Spring MVC endpoint, resolves the controller/service/repository call path it can see, and renders the result as Markdown, JSON, or Mermaid.

The project is currently a single binary crate focused on the `flow` command.

## What It Does

- Parses Java source with `tree-sitter-java`.
- Discovers Spring MVC endpoints from controller mapping annotations.
- Resolves calls through controllers, services, fields, locals, constructors, interfaces, and Spring Data repositories.
- Preserves visible control flow for branches, loops, lambdas, method references, streams, and common `Optional` operations.
- Marks unresolved and external calls instead of hiding them.
- Renders request flows as human-readable Markdown, structured JSON, or Mermaid sequence diagrams.

## Requirements

- Rust toolchain with Cargo.
- A Java/Spring source tree to analyze.

The demo Spring project under `demo-api/demo` is a fixture for exercising the mapper. The Rust tests parse this fixture; they do not need to build or run the Java app.

## Usage

Run the main demo flow:

```sh
cargo run -- flow demo-api/demo "GET /users/{id}"
```

The endpoint selector must be exactly two whitespace-separated parts:

```text
<HTTP_VERB> <PATH>
```

For example:

```sh
cargo run -- flow demo-api/demo "POST /users"
cargo run -- flow demo-api/demo "GET /users"
cargo run -- flow demo-api/demo "PUT /users/{id}"
cargo run -- flow demo-api/demo "DELETE /users/{id}"
```

### Output Formats

Markdown is the default:

```sh
cargo run -- flow demo-api/demo "GET /users/{id}" --format markdown
```

JSON:

```sh
cargo run -- flow demo-api/demo "GET /users/{id}" --format json
```

Mermaid:

```sh
cargo run -- flow demo-api/demo "GET /users/{id}" --format mermaid
```

### Depth Limit

Use `--max-depth` to limit rendered call-tree depth:

```sh
cargo run -- flow demo-api/demo "GET /users/{id}" --max-depth 2
```

Markdown and Mermaid default to a render depth of 5 when omitted. JSON is unlimited unless `--max-depth` is provided.

### Debug Logging

Enable parser and indexing diagnostics with `RUST_LOG`:

```sh
RUST_LOG=jfm=debug cargo run -- flow demo-api/demo "GET /users/{id}"
```

## Development

Common checks:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Focused integration test:

```sh
cargo test --test flow_demo flow_get_users_by_id_renders_expected_markdown
```

Snapshot tests use `insta` and live under `tests/snapshots/`. To intentionally refresh snapshots:

```sh
INSTA_UPDATE=always cargo test
```

Review snapshot diffs before accepting them.

## Current Scope

Implemented:

- `jfm flow <root> "<VERB> <PATH>"`
- `--format markdown|json|mermaid`
- `--max-depth N`
- Spring MVC endpoint discovery
- Spring Data repository method recognition for common repository base interfaces
- Branch, loop, lambda, method-reference, stream traversal, and `Optional` flow rendering

Not implemented yet:

- Persistent graph storage or Kuzu integration
- `index`, `entrypoints`, `query`, or `doctor` subcommands
- Full Java type inference
- Complete handling for switch, ternary, try/catch, AOP, Bean Validation, Lombok, `@Primary`, or `@Qualifier`

## Repository Layout

```text
src/                 Rust CLI, parser, flow resolver, and renderers
tests/               Integration and snapshot tests
tests/snapshots/     Expected Markdown, JSON, and Mermaid outputs
demo-api/demo/       Spring Boot fixture parsed by the CLI
WORKLOG.md           Notes from completed implementation slices
```
