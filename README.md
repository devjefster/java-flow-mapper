# Java Flow Mapper

`jfm` is a Rust CLI for mapping Java/Spring HTTP request flows. It indexes a Java project, finds a Spring MVC endpoint, resolves the controller/service/repository call path it can see, and renders the result as Markdown, JSON, or Mermaid diagrams.

The project is currently a single binary crate focused on the `flow` command.

## What It Does

- Parses Java source with `tree-sitter-java`.
- Discovers Spring MVC endpoints from controller mapping annotations.
- Resolves calls through controllers, services, fields, locals, constructors, interfaces, and Spring Data repositories.
- Preserves visible control flow for `if`, `switch`, ternary, try/catch/finally, loops, lambdas, method references, stream traversal, and common `Optional` operations.
- Marks unresolved and external calls instead of hiding them.
- Renders request flows as human-readable Markdown, structured JSON, Mermaid sequence diagrams, or Mermaid flowcharts.

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

Mermaid sequence diagram:

```sh
cargo run -- flow demo-api/demo "GET /users/{id}" --format mermaid
```

Mermaid flowchart:

```sh
cargo run -- flow demo-api/demo "GET /users/{id}" --format mermaid --diagram flowchart
```

`--diagram sequence|flowchart` only applies to Mermaid output. Sequence diagrams are the default.

Flowcharts render the visible source-order path: ordinary sibling calls are chained as continuations, branch arms that throw or return terminate visually, and implicit guard fall-through continues to the next source statement. Data dependencies such as call inputs and branch conditions are still shown with labeled edges.

### Depth Limit

Use `--max-depth` to limit rendered call-tree depth:

```sh
cargo run -- flow demo-api/demo "GET /users/{id}" --max-depth 2
```

Markdown and Mermaid default to a render depth of 5 when omitted. JSON is unlimited unless `--max-depth` is provided.

### Control Flow

`jfm flow` renders control structures as first-class nodes instead of flattening all reachable calls as unconditional siblings:

- Branches: `if`, `switch`, ternary expressions, and common `Optional` present/empty behavior.
- Try/catch/finally: one arm for the `try` body, one arm per `catch`, and an optional `finally` arm.
- Loops: `for`, enhanced-for, `while`, `do/while`, stream traversal, and `forEach` traversal.
- Loop execution: most loops are marked `0..n`; `do/while` is marked `1..n`.
- Loop sections: classic `for` loops split `init`, `condition`, `body`, and `update`; other loops expose the sections that apply.

The mapper records source structure and source text. It does not evaluate conditions, predict which branch runs, infer loop bounds, or model exception propagation.

Mermaid flowcharts use semantic shapes to make the structure easier to scan: project calls, external calls, control/loop nodes, decisions, and terminal paths are rendered differently while preserving Mermaid's default theme styling.

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
- `--diagram sequence|flowchart` for Mermaid output
- `--max-depth N`
- Spring MVC endpoint discovery
- Spring Data repository method recognition for common repository base interfaces
- Branch rendering for `if`, `switch`, ternary, and `Optional` control flow
- Try/catch/finally rendering with labeled arms
- Loop rendering with execution cardinality, labeled body arms, and split `for` init/condition/update sections
- Lambda, method-reference, stream traversal, and `forEach` traversal rendering
- Mermaid sequence and flowchart diagram rendering

Not implemented yet:

- Persistent graph storage or Kuzu integration
- `index`, `entrypoints`, `query`, or `doctor` subcommands
- Full Java type inference
- Symbolic condition evaluation, exception propagation, or data-dependent loop bounds
- Complete handling for AOP, Bean Validation, Lombok, `@Primary`, or `@Qualifier`

## Repository Layout

```text
src/                 Rust CLI, parser, flow resolver, and renderers
tests/               Integration and snapshot tests
tests/snapshots/     Expected Markdown, JSON, and Mermaid outputs
demo-api/demo/       Spring Boot fixture parsed by the CLI
WORKLOG.md           Notes from completed implementation slices
```
