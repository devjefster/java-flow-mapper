# Worklog

## 2026-05-25 - PR #1 thin flow slice

Implemented the first end-to-end `jfm flow` path for the demo Spring app:

- Replaced the hello-world binary with a Clap CLI exposing `jfm flow <root> "<VERB> <PATH>"`.
- Added shared model types for endpoints, classes, methods, call sites, confidence, external kinds, and rendered flows.
- Added Java indexing with `tree-sitter-java` and `walkdir`, including manual skips for `target`, `build`, `node_modules`, `.git`, `.idea`, `.mvn`, and `.gradle`.
- Extracted Spring MVC route metadata for class-level `@RequestMapping` and method-level mapping annotations.
- Extracted fields, methods, constructors, parameters, local variables, and method/constructor call sites for the demo app shape.
- Added flow resolution through controller/service calls, intra-class calls, local variables, fields, constructors, project classes, interfaces, JDK calls, and unresolved targets.
- Added Spring Data JPA repository recognition and inherited method synthesis for calls like `UserRepository#findById(Long)`.
- Added markdown rendering for the endpoint header, controller file, call sequence, inputs, unresolved/external calls, and PR #1 notes.
- Added an integration snapshot test for `GET /users/{id}` against `demo-api/demo`.
- Removed unused `pulldown-cmark`, added `anyhow`, `thiserror`, `tracing`, `tracing-subscriber`, `assert_cmd`, and `insta`.
- Added `.idea/` to `.gitignore`.

Verified:

- `cargo fmt --check`
- `cargo build --release`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo run -- flow demo-api/demo "GET /users/{id}"`
- `cargo run -- --help`
- `cargo run -- flow demo-api/demo "GET /does-not-exist"` exits non-zero with a clear missing-endpoint error.
- `cargo run -- flow target/empty-demo "GET /users/{id}"` exits non-zero with a clear no-endpoints error.
- `RUST_LOG=jfm=debug cargo run -- flow demo-api/demo "GET /users/{id}"` logs project indexing and Spring Data repository recognition.

Known PR #1 deferrals:

- No Kuzu storage or `jfm index`.
- No `entrypoints`, `query`, or `doctor` subcommands.
- No JSON or Mermaid output.
- No branch, lambda body, stream, AOP, `@Transactional`, `@ControllerAdvice`, Lombok, Bean Validation, `@Primary`, or `@Qualifier` modeling.
- No Spring Data derived-query interpretation beyond ordinary interface method lookup and inherited repository method synthesis.

## 2026-05-25 - PR #2 flow output formats

Implemented `--format markdown|json|mermaid` for `jfm flow`:

- Moved the existing Markdown renderer behind a `render::render(flow, format)` dispatcher without changing the Markdown snapshot.
- Added `Format` to the model and wired `jfm flow ... --format <format>` through Clap.
- Added `Scope::IntraClass` on `CallNode` and populated it for `this` calls so JSON can identify intra-class edges.
- Added a structured JSON renderer with stable field order and DTOs separate from the internal model.
- Added a Mermaid `sequenceDiagram` renderer with deterministic participants, external-call notes, and a final notes block.
- Added JSON and Mermaid integration snapshot tests alongside the existing Markdown snapshot.

Verified:

- `cargo fmt --check`
- `cargo build`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo run -- flow demo-api/demo "GET /users/{id}"`
- `cargo run -- flow demo-api/demo "GET /users/{id}" --format json | jq .`
- `cargo run -- flow demo-api/demo "GET /users/{id}" --format mermaid`
- `cargo run -- flow demo-api/demo "GET /users/{id}" --format yaml` exits non-zero and lists `markdown`, `json`, and `mermaid`.

Known PR #2 deferrals:

- No `--max-depth` flag.
- No Mermaid return arrows because `CallNode` does not track return types yet.
- No JSON `via` field for `single_impl`.
- No `Scope` variants beyond `IntraClass`.
- No repeated-subtree elision in Markdown.
- No ambiguous-candidate details in JSON or Mermaid.
- No format support for `entrypoints`, `doctor`, or `query` because those subcommands do not exist yet.

## 2026-05-25 - PR #3 max depth and Markdown elision

Implemented render-time `--max-depth N` for `jfm flow`:

- Added `--max-depth <N>` to the `flow` command and threaded it through the renderer dispatcher.
- Documented that render-time `--max-depth` is separate from the resolver's graph-construction recursion cap.
- Added Markdown and Mermaid default render limits of depth 5; existing demo output remains unchanged because it is below that limit.
- Added JSON truncation only when `--max-depth` is passed, with `"truncated": N` and `"calls": []` on truncated nodes.
- Added truncation notes to all three formats when output is trimmed.
- Changed Markdown's unresolved/external roundup to include only nodes that were actually rendered under the depth limit.
- Added Markdown repeated-subtree elision with `(see above)` for repeated expanded methods.
- Added unit tests for Markdown elision, including repeated siblings, repeated methods at different depths, and repeated external/unresolved leaves.
- Added `--max-depth 2` snapshots for Markdown, JSON, and Mermaid.
- Updated the vault `Output Format.md` with the JSON truncation marker spec.

Known PR #3 deferrals:

- No Mermaid repeated-subtree elision.
- No JSON repeated-subtree elision.
- No `--max-depth` support for subcommands other than `flow`.
- No per-format max-depth override flags.
- No extra endpoint snapshots beyond `GET /users/{id}`.

## 2026-05-25 - PR #4 demo endpoint snapshot baseline

Expanded the demo flow snapshot coverage without changing source code:

- Added default-render snapshots for the remaining four demo endpoints: `POST /users`, `GET /users`, `PUT /users/{id}`, and `DELETE /users/{id}`.
- Covered each new endpoint in Markdown, JSON, and Mermaid for 4 endpoints x 3 formats = 12 new snapshots.
- Generalized the `tests/flow_demo.rs` helper so each test passes its endpoint explicitly while keeping the six existing `GET /users/{id}` snapshots stable.
- Captured the current v0.1 behavior, including flattened branches and unresolved stream/method-reference shapes, as a regression baseline for v0.2 flow-control work.
- Left `src/**`, `vault/**`, `Cargo.toml`, and `Cargo.lock` untouched.

Verified:

- `cargo build`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `cargo run --quiet -- flow demo-api/demo "POST /users"`
- `cargo run --quiet -- flow demo-api/demo "GET /users"`
- `cargo run --quiet -- flow demo-api/demo "PUT /users/{id}"`
- `cargo run --quiet -- flow demo-api/demo "DELETE /users/{id}"`
- JSON variants for the four new endpoints pipe through `jq .`.
- Mermaid variants for the four new endpoints render successfully.
- The existing `GET /users/{id}` default and `--max-depth 2` Markdown, JSON, and Mermaid commands still run successfully.

## 2026-05-25 - PR #5 branch awareness

Implemented `if` branch awareness across parsing, flow resolution, and all renderers:

- Added `FlowNode::{Call, Branch}`, `BranchNode`, `Arm`, and `BranchKind::If`; `CallNode.children` now stores branch-aware flow nodes.
- Replaced flat `MethodInfo.body_calls` with parser `BodyElement` trees that preserve `if` structure, condition source text, condition-expression calls, explicit else arms, and top-level arm termination.
- Updated flow expansion so condition calls render as siblings before the branch, while branch arms contain only guarded calls.
- Rendered branches in Markdown (`- if ...:` / `- else:` with `*(terminates)*`), JSON tagged unions (`type: call|branch`), and Mermaid `alt` blocks.
- Added parser unit tests for if-without-else, if/else, nested if, condition calls, lambda skipping inside conditions, plus ignored switch/ternary markers for the next slice.
- Refreshed the 18 demo snapshots for the branch-aware model and JSON schema discriminator.
- Updated the vault output contract and flow-control note to document the shipped branch shape and the lack of synthesized else arms.

Verified:

- `cargo build`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `cargo run --quiet -- flow demo-api/demo "DELETE /users/{id}"`
- `cargo run --quiet -- flow demo-api/demo "PUT /users/{id}"`
- `cargo run --quiet -- flow demo-api/demo "POST /users" --format json | jq '.call_sequence[0].calls[] | select(.type == "branch")'`
- `cargo run --quiet -- flow demo-api/demo "DELETE /users/{id}" --format mermaid`
- `cargo run --quiet -- flow demo-api/demo "PUT /users/{id}" --max-depth 2`

Known PR #5 deferrals:

- No synthesized `else` arm for early-exit guards.
- No `switch_statement`, `switch_expression`, or ternary parsing yet.
- Lambda bodies, method references, stream operators, try/catch, and loops remain deferred.

## 2026-05-25 - Optional flow-control branch nodes

Rendered `java.util.Optional` behavior as explicit present/empty control-flow structure:

- Added `BranchKind::Optional` and renderer support in Markdown, JSON, and Mermaid.
- Added `ControlKind::Optional` so Optional operators render as control-flow calls instead of plain JDK externals.
- Optional method calls keep their method signature, with Optional-specific branch children below the call.
- Modeled `ifPresent`, `ifPresentOrElse`, `map`, `flatMap`, `filter`, `or`, `orElse`, `orElseGet`, `orElseThrow`, and `get` as Optional control-flow shapes where appropriate.
- Kept `orElse(T)` conservative: it records present/empty fallback structure without claiming fallback argument calls are empty-only, because Java evaluates arguments eagerly.
- Added implicit terminating empty arms for `orElseThrow()` and `get()` with `NoSuchElementException#<init>()` as a JDK external call.
- Expanded the JDK return-shape table for Optional factories, presence checks, transformations, fallbacks, and `stream()`.
- Improved method-reference expansion for constructor references such as `IllegalStateException::new`.
- Added resolver unit tests for Optional branch construction and Optional return-shape coverage.
- Refreshed all 18 demo snapshots so existing `findById(...).orElseThrow(...)` output now shows the supplier lambda under a terminating `optional empty` arm.
- Updated Markdown, JSON, and Mermaid snapshots so Optional control operators no longer appear as generic `external (JDK)` entries.
- Follow-up snapshot review found `Optional#orElseThrow(Supplier)` was still too easy to read as a plain external JDK call, so the resolved model now carries `ControlKind::Optional` separately from `ExternalKind::Jdk`.
- Markdown now renders Optional operators as `control - Optional`, JSON emits `"confidence": "control"` with `"control_kind": "optional"`, and Mermaid emits `control flow (Optional)`.
- Optional control operators are intentionally omitted from the Markdown `Unresolved / external` roundup; ordinary JDK calls like `String#trim()` and `Stream#map(_)` remain there.

Verified:

- `cargo run -- flow demo-api/demo "GET /users/{id}"`
- `INSTA_UPDATE=always cargo test`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- Snapshot grep confirmed no remaining `Optional#... external` or `Note over Optional: external` entries.

## 2026-05-25 - PR #6 chain receivers, lambdas, and method references

Implemented the v0.2 slice that makes lambda-backed fluent calls visible while deferring switch, ternary, loops, and try/catch to later PRs:

- Added chain-receiver typing with `ReceiverKind::Chain(Box<CallSite>)` so calls like `a.b().c()` can resolve the inner return type before looking up `c`.
- Added lambda body extraction with lambda syntax attached to call sites and `FlowNode::Lambda` in the resolved tree.
- Added method-reference support with `LambdaKind::MethodRef`; method references render with the verbatim source signature such as `this::toResponse`.
- Treated stream operators as JDK externals through a small hardcoded JDK return-type table, matching the existing inherited-return-type path for Spring Data.
- Removed the load-bearing `findById` / `findByEmail` string-match hack from the Java walker after replacing it with explicit receiver and return-type handling.
- Refreshed the demo snapshots through `cargo insta review`; all 18 endpoint/format snapshots were expected to diff for this behavioral slice.

Demo behaviors fixed:

- `findUserOrThrow` now shows the `orElseThrow` lambda body, including `new BusinessException(...)`, as a child of `orElseThrow`.
- `findAll().stream().map(this::toResponse).toList()` now resolves the fluent chain and expands `this::toResponse` to `UserService#toResponse(User)`.
- `email.trim().toLowerCase()` and `request.getName().trim()` now use chain receiver typing instead of producing `Unknown#toLowerCase()` / `Unknown#trim()` entries.

Known PR #6 deferrals:

- `Optional#orElseThrow` return type still flattens to `Object`; there is no generic-aware unwrapping yet.
- The Stream and Optional known-shapes table is intentionally hardcoded; this is not a full Java type inferencer.
- `Unknown#getDefaultMessage()` in `BusinessException` remains unresolved because constructor parameter typing is still a separate follow-up.
- Mermaid does not annotate edges as `via lambda`; lambda calls render as ordinary edges from the parent participant.

## 2026-05-25 - PR #7 loop elements

Implemented loop-aware flow nodes for Java loop syntax and fluent traversal calls:

- Added `BodyElement::Loop`, `LoopSyntax`, `FlowNode::Loop`, `LoopNode`, and `LoopKind` for `for`, enhanced `for`, `while`, `do/while`, `forEach`, and stream traversal loops.
- Parsed `for_statement`, `enhanced_for_statement`, `while_statement`, and `do_statement` before ordinary recursive call collection so loop bodies no longer flatten into sibling calls.
- Preserved loop header calls separately from body and update calls, and added enhanced-for loop-local typing so calls on variables like `user` resolve inside the loop body.
- Added Stream/List traversal handling for `Stream#map`, `flatMap`, `filter`, `peek`, `anyMatch`, `allMatch`, `noneMatch`, `forEach`, and `List`/`Set`/`Iterable#forEach`.
- Rendered loops in Markdown, JSON, and Mermaid; loop containers do not count toward `--max-depth`, matching branch and lambda wrappers.
- Added `GET /users/active` to the demo app to snapshot enhanced-for and `List#forEach` behavior in all three formats.
- Refreshed the existing `GET /users` snapshots so `stream().map(this::toResponse)` now shows the stream loop body and method-reference expansion.
- Added parser and resolver unit tests for Java loop syntax, nested loop/branch structure, stream traversal loops, forEach loops, and JDK return shapes.

Verified:

- `cargo fmt --check`
- `cargo build`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo run --quiet -- flow demo-api/demo "GET /users"`
- `cargo run --quiet -- flow demo-api/demo "GET /users" --format json | jq .`
- `cargo run --quiet -- flow demo-api/demo "GET /users" --format mermaid`
- `cargo run --quiet -- flow demo-api/demo "GET /users/active"`
- `cargo run --quiet -- flow demo-api/demo "GET /users/active" --format json | jq .`
- `cargo run --quiet -- flow demo-api/demo "GET /users/active" --format mermaid`

Known PR #7 deferrals:

- Loop iteration counts and data-dependent exit behavior are not inferred.
- Classic `for` initializer/update typing remains shallow; only calls are surfaced.
- Stream loop modeling is still a hardcoded known-shapes table, not general Java type inference.
- Mermaid loop rendering does not distinguish condition/body/update sections; Markdown and JSON keep those sections explicit.

## 2026-05-26 - Parser module refactor

Completed the `src/parser/walker.rs` decomposition without changing parser behavior:

- Kept `src/parser/mod.rs` as the high-level entry point for project indexing, `parse_file`, and endpoint assembly.
- Preserved the split modules for focused responsibilities: `class.rs`, `body.rs`, `utils.rs`, and `annotations.rs`.
- Removed the duplicate legacy `src/parser/walker.rs` implementation.
- Moved the walker regression tests into `src/parser/tests.rs` so they exercise the new module entry point directly.
- Cleaned sibling-module imports in `class.rs` and `body.rs` to avoid `crate::parser::...` paths inside the parser module.

Verified:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

## 2026-05-26 - Switch branch elements

Implemented switch-statement flow-control support and added demo snapshot coverage:

- Added `BranchKind::Switch` and reusable parsed branch arms so switch cases share the existing branch expansion/rendering path.
- Parsed tree-sitter `switch_statement` and current `switch_expression` nodes as branch syntax, preserving the discriminant source, condition calls, `case` labels, `default`, arm bodies, and simple terminating arms.
- Updated flow expansion to consume parsed branch arms generically instead of hardcoding only `then`/`else` arms.
- Rendered switch arms in Markdown (`case ...:` / `default:`), JSON (`"kind": "switch"`), and Mermaid `alt`/`else case` blocks.
- Activated the switch parser unit test with cases for ordinary labels, `default`, condition calls, arm calls, and a throwing arm.
- Added an enum-backed switch to the existing `PUT /users/{id}` demo update flow through `UserService#activeChange(...)`.
- Refreshed the PUT endpoint Markdown, JSON, and Mermaid snapshots so the fixture now covers switch output in all formats.

Verified:

- `cargo test parser::tests::parses_switch_statement_branches -- --nocapture`
- `INSTA_UPDATE=always cargo test --test flow_demo flow_put_users_by_id_renders_expected`
- `INSTA_UPDATE=always cargo test`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`

Known deferrals:

- Switch-expression result values are not modeled beyond the branch/case structure discovered by tree-sitter.
- Fallthrough semantics are not inferred; cases are represented as labeled arms with their collected body calls.
- Try/catch flow-control elements remain deferred.

## 2026-05-26 - Ternary branch elements

Implemented ternary-expression flow-control support and added demo snapshot coverage:

- Added `BranchKind::Ternary` and reused parsed branch arms so inline `cond ? a : b` expressions share the existing branch expansion path.
- Parsed `ternary_expression` nodes before generic recursive call collection so condition and arm calls no longer flatten as unconditional siblings.
- Preserved the condition source text, condition calls, a `then` arm for the consequence expression, and an `else` arm for the alternative expression.
- Kept ternary arm termination flags conservative at `false`; expression-result and throw-expression semantics are not modeled yet.
- Rendered ternaries in Markdown (`ternary ...:` / `else:`), JSON (`"kind": "ternary"`), and Mermaid `alt` / `else` blocks.
- Activated the ternary parser unit test with `enabled() ? yes() : no()` to cover condition calls and both arm calls.
- Added a ternary to the existing `PUT /users/{id}` demo update flow using `request.getActive() == null ? unchangedStatus(user) : requestedStatus(request)`.
- Refreshed the PUT endpoint Markdown, JSON, and Mermaid snapshots so the fixture now covers ternary output in all formats.

Verified:

- `cargo test parser::tests::parses_ternary_expression_branches -- --nocapture`
- `INSTA_UPDATE=always cargo test --test flow_demo flow_put_users_by_id_renders_expected`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `INSTA_UPDATE=always cargo test`

Known deferrals:

- Ternary expression result values are not represented separately from branch-arm calls.
- Ternary arm termination remains conservative; throw expressions and richer expression semantics are deferred.

## 2026-05-26 - Try/catch branch elements

Implemented try/catch/finally flow-control support and added demo snapshot coverage:

- Added `BranchKind::TryCatch` and reused parsed branch arms so `try`, each `catch`, and optional `finally` blocks share the existing branch expansion path.
- Parsed `try_statement` nodes before generic recursive call collection so calls inside try/catch/finally arms no longer flatten as unconditional siblings.
- Preserved a `try` arm, one labeled arm per catch clause such as `catch IllegalArgumentException ex`, and a `finally` arm when present.
- Added shallow arm termination detection for try/catch/finally bodies, matching the existing if/switch termination behavior.
- Conservatively collects calls from try-with-resources header nodes into `condition_calls` when tree-sitter exposes them outside block/catch/finally children.
- Rendered try/catch/finally in Markdown (`try:` / `catch ...:` / `finally:`), JSON (`"kind": "try_catch"`), and Mermaid `alt try` / `else catch ...` / `else finally` blocks.
- Added a parser unit test covering try body, catch clause, and finally body extraction.
- Added a try/catch/finally block to the existing `PUT /users/{id}` demo update flow after the ternary status decision.
- Refreshed the PUT endpoint Markdown, JSON, and Mermaid snapshots so the fixture now covers try/catch/finally output in all formats.

Verified:

- `cargo test parser::tests::parses_try_catch_finally_branches -- --nocapture`
- `INSTA_UPDATE=always cargo test --test flow_demo flow_put_users_by_id_renders_expected`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `INSTA_UPDATE=always cargo test`

Known deferrals:

- Exception type flow is not inferred; catch arms are structural and do not model which exceptions reach each handler.
- Try-with-resources is only shallowly surfaced through header calls, not resource lifetime semantics.
- Finally dominance is not modeled; JFM does not infer that finally always runs or overrides prior returns/throws.

## 2026-05-26 - Loop contract alignment

Aligned loop flow nodes with the larger v0.2 control-flow contract:

- Added loop execution cardinality with `LoopExecution::ZeroOrMore` and `LoopExecution::OneOrMore`.
- Marked `for`, enhanced-for, `while`, stream traversal, and `forEach` loops as `0..n`; marked `do/while` loops precisely as `1..n`.
- Added parsed and resolved loop arms (`LoopArmSyntax` / `LoopArm`) so loop bodies now render as a labeled `body` arm rather than a bare `body` list.
- Split classic `for` header calls into `init_calls`, `condition_calls`, and `update_calls`; `start()` now belongs to init while `limit()` remains condition in parser tests.
- Updated flow expansion to resolve loop `init`, `condition`, `arms`, and `update` separately while preserving enhanced-for loop-local typing inside body arms.
- Updated synthetic stream and `forEach` traversal loops to use `execution: 0..n` and a `body` arm around their lambda/method-reference payload.
- Updated Markdown loop rendering to show execution cardinality, e.g. `*(may execute 0..n times)*`, and to render `init`, `condition`, `body`, and `update` sections when present.
- Updated JSON loop output with `execution`, `init`, and `arms` fields, replacing the previous bare `body` field.
- Updated Mermaid loop headers to include cardinality, e.g. `loop for-each User user : users (0..n)`.
- Refreshed loop-bearing demo snapshots for `GET /users` and `GET /users/active` across Markdown, JSON, and Mermaid.

Verified:

- `cargo test parser::tests::parses_classic_for_loop_header_body_and_update_calls -- --nocapture`
- `INSTA_UPDATE=always cargo test`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

Known deferrals:

- Mermaid still flattens loop sections inside a single loop block; Markdown and JSON preserve section labels.
- Loop iteration counts remain structural cardinality only; JFM does not infer data-dependent bounds.
- Classic `for` local-variable typing for initializer-declared variables remains shallow beyond surfaced calls.

## 2026-05-26 - Flowchart control-flow rendering

Reworked Mermaid flowchart output so it reads as source-order control flow rather than a call-tree fanout:

- Cleaned up `src/flow/expand.rs` after it had duplicated/stale expansion code, restoring a single branch-aware expansion path with `inputs` and branch `condition` fields populated.
- Changed the Mermaid flowchart renderer to thread continuation exits through ordered sibling `FlowNode`s, so the main path now chains through calls like validation, guard checks, entity construction, repository save, and response mapping.
- Rendered terminating branch arms as terminal paths that do not continue into later siblings, while implicit fall-through paths from `if` guards continue to the next source-order node.
- Added semantic Mermaid shapes for resolved project calls, external calls, control/loop nodes, decisions, and terminators.
- Added focused renderer tests for sibling sequencing and terminating-branch fall-through.
- Refreshed all Mermaid flowchart snapshots, including the `POST /users` baseline and the max-depth flowchart case.
- Added project-local `.cursor/mcp.json` for `mcp-mermaid`, but Cursor had not exposed the server descriptor yet, so MCP validation was not runnable in this slice.

Verified:

- `cargo check`
- `cargo test render::mermaid_flowchart::tests`
- `INSTA_UPDATE=always cargo test --test flow_demo mermaid_flowchart`
- `cargo fmt --check`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- IDE lints for touched Rust files

Known deferrals:

- Mermaid MCP validation remains pending until Cursor refreshes and exposes the `mcp-mermaid` tool descriptor.
- Flowchart paths remain structural; JFM still does not infer runtime branch truth, exception routing, or data-dependent loop exits.

## 2026-05-26 - Multi-crate workspace layout

Split the single binary crate into focused workspace members without changing `jfm flow` behavior:

- Converted the root `Cargo.toml` into a workspace manifest with shared workspace package/dependency settings.
- Added focused crates for CLI orchestration, flow resolution, shared models, Java parsing, rendering, and Spring helpers.
- Kept the binary name as `jfm` through `crates/jfm-cli`.
- Moved the existing integration tests and snapshots under `crates/jfm-cli/tests` so `assert_cmd` still exercises the workspace binary.
- Updated README layout and snapshot-path notes for the workspace structure.

Verified:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo run -- flow demo-api/demo "GET /users/{id}"`
- `cargo run -- flow demo-api/demo "GET /users/{id}" --format json`
- `cargo run -- flow demo-api/demo "GET /users/{id}" --format mermaid`

## 2026-05-26 - ProjectIndex cache via SurrealDB

Added a new `crates/jfm-graph` crate that round-trips `ProjectIndex` through an embedded SurrealDB:

- Initially scaffolded against Kuzu `0.11.3` but hit a `cxx` (1.0.138) vs `cxx-build` (1.0.194) symbol-mangling skew on arm64 macOS, surfacing as `_kuzu_rs$cxxbridge1$*` undefined symbols at link time. Pinning `cxx-build` to 1.0.138 cleared the link, but the heavy native build and a duplicate `kuzu_test/` probe were not worth keeping for a round-trip-only use case.
- Pivoted to SurrealDB with the pure-Rust `kv-surrealkv` backend. First compile is ~1m 30s and there are no native link steps.
- Added `SurrealGraphStore::{open, save_project_index, load_project_index}` driven by a `tokio` current-thread runtime owned by the store, so the public API stays synchronous from the caller's perspective.
- Stored the entire `ProjectIndex` as a single `cache:main` record wrapping a `CachedIndex { index }` payload. Schema is intentionally minimal; first-class node/edge modeling can be added when ad-hoc query use cases arrive.
- Added `Serialize`/`Deserialize` derives across the `jfm-model` data types. Marked `Fqn` `#[serde(transparent)]` so it round-trips as a bare string inside `HashMap<Fqn, ClassInfo>` keys.
- Added round-trip tests for empty index, a hand-built `UserController` + `getUser` endpoint, and save-overwrites-prior-record.
- Removed the `kuzu_test/` workspace member and the `kuzu_verification.rs` test now that Kuzu is gone.
- Promoted `surrealdb`, `tokio`, `tempfile`, and `jfm-graph` into `[workspace.dependencies]` so version drift between crates is no longer possible.

Verified:

- `cargo build --workspace`
- `cargo test -p jfm-graph`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets`
- `cargo fmt --check`

Known deferrals:

- `jfm-cli` does not call into `jfm-graph` yet; the API is internal until a cache subcommand or query use case lands.
- Schema is a single blob record; first-class `Class` / `Method` / `Endpoint` graph relations and `CALLS` / `EXPOSES` edges are deferred until queries justify them.
- `MethodInfo.body` recursive trees survive round-trip via plain serde, not flattened into graph edges.
- No incremental update API — `save_project_index` overwrites the cache record in one shot.

## 2026-05-27 - Cached index CLI commands

Wired the SurrealDB-backed `ProjectIndex` cache into user-facing CLI commands:

- Added `jfm index <root> [--graph-dir <path>]` to parse a Java project and save the resulting `ProjectIndex` through `SurrealGraphStore::save_project_index`.
- Added a shared default cache location at `<root>/.jfm/index`, while preserving `--graph-dir` for tests and alternate cache locations.
- Added `jfm entrypoints <root> [--method VERB] [--path-prefix PATH] [--format markdown|json] [--graph-dir <path>]` to list cached Spring MVC endpoints without reparsing source.
- Added `jfm doctor <root> [--format markdown|json] [--graph-dir <path>]` to load the cached index, build each endpoint flow, and report class/method/endpoint totals, flow build status, confidence counts, and unresolved warnings.
- Kept `jfm flow` on the existing direct parse/build path; it does not consume the cache yet.
- Added focused CLI integration tests for index cache creation, entrypoint listing/filtering/missing-cache errors, and doctor Markdown/JSON/missing-cache errors.
- Updated README usage and current-scope notes for the new commands.
- Added current-directory root defaults for all root-taking commands: `index`, `entrypoints`, `doctor`, and `flow "<VERB> <PATH>"`.
- Added focused integration tests for omitted-root behavior across `index`, `entrypoints`, `doctor`, and `flow`.

Known deferrals:

- `jfm query` remains deferred because the SurrealDB store is still a single `cache:main` blob record, not a first-class queryable graph schema.
- `entrypoints` and `doctor` do not silently reparse when the cache is missing; users must run `jfm index` first.
- The cache does not yet record index metadata such as timestamp, schema version, or Java file count.
- `jfm flow` still reparses source each run instead of reading the cached `ProjectIndex`.

## 2026-05-27 - Bean Validation input constraints

Implemented the v0.3 Bean Validation slice for endpoint input summaries:

- Added serde-defaulted validation metadata to `ParamInfo`, `FieldInfo`, and `ClassInfo` so cached indexes remain tolerant of older serialized data.
- Preserved raw parameter annotations during parsing, including controller inputs such as `@Valid @RequestBody CreateUserRequest request`.
- Extracted supported built-in field constraints from DTOs: `@NotBlank`, `@NotNull`, `@Email`, `@Min`, `@Max`, `@Size`, and `@Pattern`.
- Indexed Java annotation declarations (`public @interface ...`) so custom constraint annotations can be discovered as project symbols.
- Added flow-time enrichment for parameters annotated with `@Valid`: the resolver walks the DTO fields and attaches validation metadata to `Flow.inputs`.
- Resolved custom constraint validators from annotations declared with `@Constraint(validatedBy = SomeValidator.class)`.
- Rendered validation metadata under the Markdown `## Inputs` section only; JSON and Mermaid remain unchanged except for the global notes text.
- Removed Bean Validation from the generic “not modeled” note and left AOP, `@Transactional`, `@ControllerAdvice`, DI qualifiers, and Lombok in that note.
- Added demo fixture coverage with `@CompanyEmail` and `CompanyEmailValidator`, applied to both create and update request DTOs.
- Refreshed demo snapshots so `POST /users` and `PUT /users/{id}` Markdown show built-in and custom validation constraints.
- Updated vault roadmap/deferred-feature docs to include `@NotNull` and `@Max` in the v0.3 scope.

Verified:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

Known deferrals:

- Programmatic validation through `Validator.validate(obj)` is not modeled.
- Validation groups, message interpolation, payloads, and type-use constraints are not interpreted.
- Validation metadata is not exposed in JSON or Mermaid yet.

## 2026-05-27 - Mermaid flowchart readability pass

Reworked the Mermaid flowchart renderer to make control-flow diagrams more readable, especially around Optional paths, exceptions, and shared helper calls:

- Added reusable flowchart node tracking so repeated meaningful calls, such as exception constructors and helper methods with children, render once instead of duplicating identical nodes.
- Added method subgraphs for nested resolved project calls, with cross-subgraph edges deferred to top level so Mermaid does not link from inside one subgraph to a node owned by another.
- Rendered exception constructor flows as separate subgraphs with light red styling for the exception path.
- Simplified Optional flowchart rendering by removing the synthetic `optional` decision node and using direct `empty` edges from the Optional control call.
- Changed branch condition rendering so `if` / `switch` / ternary nodes remain on the main spine and condition calls hang off them as `condition` dependencies.
- Changed call input rendering so constructors, setters, and other operation calls stay on the main path while getter/input calls hang off as `input` dependencies.
- Treated shared resolved calls with children, such as `Normalizers#normalizeEmail(String)`, as independent reusable subgraphs instead of nesting them under the first caller.
- Tuned reuse for complex diagrams by no longer globally reusing leaf getter/setter calls, which reduced long sweeping edges in `POST /users` and `PUT /users/{id}` flowcharts.
- Refreshed all Mermaid flowchart snapshots under `crates/jfm-cli/tests/snapshots/`.

Verified:

- `cargo test -p jfm-render`
- `cargo test -p jfm-cli --test flow_demo mermaid_flowchart`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

Known deferrals:

- Very large flowcharts are still constrained by Mermaid layout behavior; a future complex-flow mode should collapse method internals by default and allow focused expansion.
- Flowcharts remain structural and do not infer runtime branch truth, exception routing, or data-dependent loop exits.
