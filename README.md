# Java Flow Mapper

`jfm` is a Rust CLI for mapping Java/Spring HTTP request flows. It indexes a Java project, finds a Spring MVC endpoint, resolves the controller/service/repository call path it can see, and renders the result as Markdown, JSON, or Mermaid diagrams.

The project is a Rust workspace of focused crates, with the `jfm` binary provided by `crates/jfm-cli`.

## What It Does

- Parses Java source with `tree-sitter-java`.
- Discovers Spring MVC endpoints from controller mapping annotations.
- Resolves calls through controllers, services, fields, locals, constructors, interfaces, and Spring Data repositories.
- Preserves visible control flow for `if`, `switch`, ternary, try/catch/finally, loops, lambdas, method references, stream traversal, and common `Optional` operations.
- Extracts Bean Validation constraints from DTO fields reached through `@Valid` controller inputs and renders them in Markdown input summaries.
- Marks unresolved and external calls instead of hiding them.
- Renders request flows as human-readable Markdown, structured JSON, Mermaid sequence diagrams, or Mermaid flowcharts.

## Requirements

- Rust toolchain with Cargo.
- A Java/Spring source tree to analyze.

The demo Spring project under `demo-api/demo` is a fixture for exercising the mapper. The Rust tests parse this fixture; they do not need to build or run the Java app.

## Usage

Build or refresh the local project index cache:

```sh
cargo run -- index demo-api/demo
```

The default cache location is `<root>/.jfm/index`. Pass `--graph-dir <path>` to use a different SurrealDB cache directory.

For every command that accepts `<root>`, omitting it uses the current directory as the Java project root.

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

List cached entrypoints after indexing:

```sh
cargo run -- entrypoints demo-api/demo
cargo run -- entrypoints demo-api/demo --method GET --path-prefix /users --format json
```

Inspect cached index and flow health:

```sh
cargo run -- doctor demo-api/demo
cargo run -- doctor demo-api/demo --format json
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

### Bean Validation

Markdown output includes Bean Validation constraints for DTO fields reached from controller parameters annotated with `@Valid`, such as `@Valid @RequestBody CreateUserRequest request`.

Supported built-in constraints are `@NotBlank`, `@NotNull`, `@Email`, `@Min`, `@Max`, `@Size`, and `@Pattern`. Custom constraint annotations are also surfaced when their annotation declaration uses `@Constraint(validatedBy = SomeValidator.class)`.

Validation metadata is attached to the Markdown `## Inputs` section only. JSON and Mermaid intentionally remain focused on flow structure and do not currently include validation metadata.

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

Snapshot tests use `insta` and live under `crates/jfm-cli/tests/snapshots/`. To intentionally refresh snapshots:

```sh
INSTA_UPDATE=always cargo test
```

Review snapshot diffs before accepting them.

## Current Scope

Implemented:

- `jfm flow <root> "<VERB> <PATH>"`
- `jfm flow "<VERB> <PATH>"` from the project root
- `jfm index <root> [--graph-dir <path>]`
- `jfm entrypoints <root> [--method VERB] [--path-prefix PATH] [--format markdown|json] [--graph-dir <path>]`
- `jfm doctor <root> [--format markdown|json] [--graph-dir <path>]`
- `--format markdown|json|mermaid`
- `--diagram sequence|flowchart` for Mermaid output
- `--max-depth N`
- SurrealDB-backed `ProjectIndex` cache through `crates/jfm-graph`
- Spring MVC endpoint discovery
- Spring Data repository method recognition for common repository base interfaces
- Branch rendering for `if`, `switch`, ternary, and `Optional` control flow
- Try/catch/finally rendering with labeled arms
- Loop rendering with execution cardinality, labeled body arms, and split `for` init/condition/update sections
- Lambda, method-reference, stream traversal, and `forEach` traversal rendering
- Bean Validation input summaries in Markdown for `@Valid` DTOs, including selected built-ins and custom `ConstraintValidator` annotations
- Mermaid sequence and flowchart diagram rendering

Not implemented yet:

- Graph-shaped schema with first-class `Class` / `Method` / `Endpoint` records and `CALLS` / `EXPOSES` edges for ad-hoc queries
- `query` subcommand
- Full Java type inference
- Symbolic condition evaluation, exception propagation, or data-dependent loop bounds
- Complete handling for AOP, Lombok, `@Primary`, or `@Qualifier`
- Programmatic Bean Validation calls such as `Validator.validate(obj)`, validation groups, message interpolation, and type-use constraints

## Repository Layout

```text
crates/jfm-cli/      Binary crate, Clap wiring, and command orchestration
crates/jfm-flow/     Flow expansion and call resolution
crates/jfm-graph/    Embedded SurrealDB store for round-tripping ProjectIndex
crates/jfm-model/    Shared data contracts
crates/jfm-parser/   Java parsing and project indexing
crates/jfm-render/   Markdown, JSON, and Mermaid renderers
crates/jfm-spring/   Spring Data/JPA helpers
crates/jfm-cli/tests/ Integration and snapshot tests
demo-api/demo/       Spring Boot fixture parsed by the CLI
WORKLOG.md           Notes from completed implementation slices
```
