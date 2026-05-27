//! CLI parsing and command orchestration.
//!
//! This module keeps user input validation near Clap and delegates indexing,
//! flow construction, and rendering to their dedicated modules.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};

use jfm_flow as flow;
use jfm_graph::SurrealGraphStore;
use jfm_model::{
    BranchNode, CallNode, Confidence, Diagram, Endpoint, FlowNode, Format, HttpVerb, LoopNode,
};
use jfm_parser as parser;
use jfm_render as render;

#[derive(Debug, Parser)]
#[command(name = "jfm", about = "Map Java/Spring request flows")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Parse and cache a Java project index.
    Index {
        /// Java project root to parse. Defaults to the current directory.
        root: Option<PathBuf>,
        /// Directory for the embedded graph cache.
        #[arg(long)]
        graph_dir: Option<PathBuf>,
    },
    /// List cached HTTP entry points for an indexed Java project.
    Entrypoints {
        /// Java project root whose cached index should be loaded. Defaults to the current directory.
        root: Option<PathBuf>,
        /// Filter by HTTP method.
        #[arg(long)]
        method: Option<HttpVerb>,
        /// Filter by endpoint path prefix.
        #[arg(long)]
        path_prefix: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = EntryPointsFormat::Markdown)]
        format: EntryPointsFormat,
        /// Directory for the embedded graph cache.
        #[arg(long)]
        graph_dir: Option<PathBuf>,
    },
    /// Inspect cached index and endpoint flow health.
    Doctor {
        /// Java project root whose cached index should be loaded. Defaults to the current directory.
        root: Option<PathBuf>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = DoctorFormat::Markdown)]
        format: DoctorFormat,
        /// Directory for the embedded graph cache.
        #[arg(long)]
        graph_dir: Option<PathBuf>,
    },
    /// Render a call flow for an HTTP endpoint.
    Flow {
        /// Either `<root> "<VERB> <PATH>"` or just `"<VERB> <PATH>"` from the project root.
        #[arg(value_name = "ROOT_OR_ENDPOINT", num_args = 1..=2)]
        args: Vec<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = Format::Markdown)]
        format: Format,
        /// Mermaid diagram shape.
        #[arg(long, value_enum, default_value_t = Diagram::Sequence)]
        diagram: Diagram,
        /// Render-time depth limit for the call tree.
        #[arg(long)]
        max_depth: Option<usize>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum EntryPointsFormat {
    Markdown,
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DoctorFormat {
    Markdown,
    Json,
}

/// Parse CLI arguments, run the selected command, and print its output.
pub fn run() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Index { root, graph_dir } => {
            let root = root_or_current(root)?;
            let graph_dir = graph_dir.unwrap_or_else(|| default_graph_dir(&root));
            if let Some(parent) = graph_dir
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
            {
                fs::create_dir_all(parent)
                    .with_context(|| format!("while creating {}", parent.display()))?;
            }

            let index = parser::index_project(&root)
                .with_context(|| format!("while parsing {}", root.display()))?;
            let store = SurrealGraphStore::open(&graph_dir)
                .with_context(|| format!("while opening graph cache {}", graph_dir.display()))?;
            store
                .save_project_index(&index)
                .with_context(|| format!("while saving graph cache {}", graph_dir.display()))?;

            println!(
                "Indexed {} classes, {} endpoints into {}",
                index.classes.len(),
                index.endpoints.len(),
                graph_dir.display()
            );
            Ok(())
        }
        Commands::Entrypoints {
            root,
            method,
            path_prefix,
            format,
            graph_dir,
        } => {
            let root = root_or_current(root)?;
            let graph_dir = graph_dir.unwrap_or_else(|| default_graph_dir(&root));
            let store = SurrealGraphStore::open(&graph_dir)
                .with_context(|| format!("while opening graph cache {}", graph_dir.display()))?;
            let index = store.load_project_index()?.with_context(|| {
                format!(
                    "no cached project index found at {}. Run `jfm index {}` first.",
                    graph_dir.display(),
                    root.display()
                )
            })?;

            let mut endpoints: Vec<&Endpoint> = index
                .endpoints
                .iter()
                .filter(|endpoint| method.as_ref().is_none_or(|verb| endpoint.verb == *verb))
                .filter(|endpoint| {
                    path_prefix
                        .as_ref()
                        .is_none_or(|prefix| endpoint.path.starts_with(prefix))
                })
                .collect();
            endpoints.sort_by(|left, right| {
                left.verb
                    .to_string()
                    .cmp(&right.verb.to_string())
                    .then_with(|| left.path.cmp(&right.path))
                    .then_with(|| left.handler_fqn.0.cmp(&right.handler_fqn.0))
                    .then_with(|| left.file.cmp(&right.file))
                    .then_with(|| left.line.cmp(&right.line))
            });

            match format {
                EntryPointsFormat::Markdown => {
                    print!("{}", render_entrypoints_markdown(&endpoints))
                }
                EntryPointsFormat::Json => println!("{}", render_entrypoints_json(&endpoints)?),
            }
            Ok(())
        }
        Commands::Doctor {
            root,
            format,
            graph_dir,
        } => {
            let root = root_or_current(root)?;
            let graph_dir = graph_dir.unwrap_or_else(|| default_graph_dir(&root));
            let store = SurrealGraphStore::open(&graph_dir)
                .with_context(|| format!("while opening graph cache {}", graph_dir.display()))?;
            let index = store.load_project_index()?.with_context(|| {
                format!(
                    "no cached project index found at {}. Run `jfm index {}` first.",
                    graph_dir.display(),
                    root.display()
                )
            })?;
            let report = build_doctor_report(&index);

            match format {
                DoctorFormat::Markdown => print!("{}", render_doctor_markdown(&report)),
                DoctorFormat::Json => println!("{}", render_doctor_json(&report)?),
            }
            Ok(())
        }
        Commands::Flow {
            args,
            format,
            diagram,
            max_depth,
        } => {
            let (root, endpoint) = parse_flow_args(args)?;
            let (verb, path) = parse_endpoint(&endpoint)?;
            let index = parser::index_project(&root)
                .with_context(|| format!("while parsing {}", root.display()))?;
            let flow = flow::build_flow(&index, verb, &path)?;
            print!("{}", render::render(&flow, format, diagram, max_depth));
            Ok(())
        }
    }
}

fn default_graph_dir(root: &Path) -> PathBuf {
    root.join(".jfm").join("index")
}

fn root_or_current(root: Option<PathBuf>) -> Result<PathBuf> {
    match root {
        Some(root) => Ok(root),
        None => std::env::current_dir().context("while resolving current directory"),
    }
}

fn parse_flow_args(args: Vec<String>) -> Result<(PathBuf, String)> {
    match args.as_slice() {
        [endpoint] => Ok((root_or_current(None)?, endpoint.clone())),
        [root, endpoint] => Ok((PathBuf::from(root), endpoint.clone())),
        _ => bail!("flow expects either `<ROOT> <VERB PATH>` or `<VERB PATH>`"),
    }
}

fn parse_endpoint(endpoint: &str) -> Result<(HttpVerb, String)> {
    let mut parts = endpoint.split_whitespace();
    let verb = parts
        .next()
        .context("endpoint must include an HTTP verb, e.g. `GET /users/{id}`")?
        .parse()
        .map_err(|message: String| anyhow::anyhow!(message))?;
    let path = parts
        .next()
        .context("endpoint must include a path, e.g. `GET /users/{id}`")?;

    if parts.next().is_some() {
        bail!("endpoint must be exactly `<VERB> <PATH>`, got `{endpoint}`");
    }

    Ok((verb, path.to_string()))
}

fn render_entrypoints_markdown(endpoints: &[&Endpoint]) -> String {
    let mut out = String::new();
    writeln!(out, "# Entry Points").unwrap();
    writeln!(out).unwrap();

    if endpoints.is_empty() {
        writeln!(out, "_No entry points matched._").unwrap();
        return out;
    }

    for endpoint in endpoints {
        writeln!(
            out,
            "- `{} {}` -> `{}` (`{}:{}`)",
            endpoint.verb,
            endpoint.path,
            endpoint.handler_fqn,
            endpoint.file.display(),
            endpoint.line
        )
        .unwrap();
    }

    out
}

fn render_entrypoints_json(endpoints: &[&Endpoint]) -> Result<String> {
    let value: Vec<_> = endpoints
        .iter()
        .map(|endpoint| {
            serde_json::json!({
                "method": endpoint.verb.to_string(),
                "path": endpoint.path,
                "handler": endpoint.handler_fqn.0,
                "file": endpoint.file.display().to_string(),
                "line": endpoint.line,
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&value)?)
}

#[derive(Default)]
struct DoctorReport {
    class_count: usize,
    method_count: usize,
    endpoint_count: usize,
    flows_built: usize,
    flow_errors: usize,
    confidence: ConfidenceCounts,
    endpoints: Vec<DoctorEndpointSummary>,
    warnings: Vec<String>,
}

struct DoctorEndpointSummary {
    method: String,
    path: String,
    handler: String,
    status: String,
    unresolved: usize,
    error: Option<String>,
    confidence: ConfidenceCounts,
}

#[derive(Clone, Copy, Default)]
struct ConfidenceCounts {
    resolved: usize,
    single_impl: usize,
    primary: usize,
    qualifier: usize,
    ambiguous: usize,
    external: usize,
    unresolved: usize,
}

fn build_doctor_report(index: &jfm_model::ProjectIndex) -> DoctorReport {
    let mut endpoints: Vec<&Endpoint> = index.endpoints.iter().collect();
    endpoints.sort_by(|left, right| {
        left.verb
            .to_string()
            .cmp(&right.verb.to_string())
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.handler_fqn.0.cmp(&right.handler_fqn.0))
    });

    let mut report = DoctorReport {
        class_count: index.classes.len(),
        method_count: index
            .classes
            .values()
            .map(|class| class.methods.len())
            .sum(),
        endpoint_count: index.endpoints.len(),
        ..DoctorReport::default()
    };

    for endpoint in endpoints {
        match flow::build_flow(index, endpoint.verb.clone(), &endpoint.path) {
            Ok(flow) => {
                let mut confidence = ConfidenceCounts::default();
                count_call_confidence(&flow.root, &mut confidence);
                report.confidence.add(confidence);
                report.flows_built += 1;

                let unresolved = flow.unresolved.len() + confidence.unresolved;
                if unresolved > 0 {
                    report.warnings.push(format!(
                        "{} {} has {} unresolved reference(s)",
                        endpoint.verb, endpoint.path, unresolved
                    ));
                }

                report.endpoints.push(DoctorEndpointSummary {
                    method: endpoint.verb.to_string(),
                    path: endpoint.path.clone(),
                    handler: endpoint.handler_fqn.0.clone(),
                    status: "ok".to_string(),
                    unresolved,
                    error: None,
                    confidence,
                });
            }
            Err(error) => {
                let message = error.to_string();
                report.flow_errors += 1;
                report.warnings.push(format!(
                    "{} {} failed to build flow: {}",
                    endpoint.verb, endpoint.path, message
                ));
                report.endpoints.push(DoctorEndpointSummary {
                    method: endpoint.verb.to_string(),
                    path: endpoint.path.clone(),
                    handler: endpoint.handler_fqn.0.clone(),
                    status: "error".to_string(),
                    unresolved: 0,
                    error: Some(message),
                    confidence: ConfidenceCounts::default(),
                });
            }
        }
    }

    report
}

fn count_call_confidence(call: &CallNode, counts: &mut ConfidenceCounts) {
    counts.add_one(&call.confidence);
    for node in call.inputs.iter().chain(call.children.iter()) {
        count_node_confidence(node, counts);
    }
}

fn count_node_confidence(node: &FlowNode, counts: &mut ConfidenceCounts) {
    match node {
        FlowNode::Call(call) => count_call_confidence(call, counts),
        FlowNode::Lambda(lambda) => {
            for child in &lambda.children {
                count_node_confidence(child, counts);
            }
        }
        FlowNode::Branch(branch) => count_branch_confidence(branch, counts),
        FlowNode::Loop(loop_node) => count_loop_confidence(loop_node, counts),
    }
}

fn count_branch_confidence(branch: &BranchNode, counts: &mut ConfidenceCounts) {
    for node in &branch.condition {
        count_node_confidence(node, counts);
    }
    for arm in &branch.arms {
        for child in &arm.children {
            count_node_confidence(child, counts);
        }
    }
}

fn count_loop_confidence(loop_node: &LoopNode, counts: &mut ConfidenceCounts) {
    for node in loop_node
        .init
        .iter()
        .chain(loop_node.condition.iter())
        .chain(loop_node.update.iter())
    {
        count_node_confidence(node, counts);
    }
    for arm in &loop_node.arms {
        for child in &arm.children {
            count_node_confidence(child, counts);
        }
    }
}

fn render_doctor_markdown(report: &DoctorReport) -> String {
    let mut out = String::new();
    writeln!(out, "# JFM Doctor").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Summary").unwrap();
    writeln!(out, "- Classes: {}", report.class_count).unwrap();
    writeln!(out, "- Methods: {}", report.method_count).unwrap();
    writeln!(out, "- Endpoints: {}", report.endpoint_count).unwrap();
    writeln!(out, "- Flows built: {}", report.flows_built).unwrap();
    writeln!(out, "- Flow errors: {}", report.flow_errors).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Confidence Totals").unwrap();
    write_confidence_markdown(&mut out, &report.confidence);
    writeln!(out).unwrap();
    writeln!(out, "## Endpoints").unwrap();
    if report.endpoints.is_empty() {
        writeln!(out, "_No endpoints found._").unwrap();
    } else {
        for endpoint in &report.endpoints {
            writeln!(
                out,
                "- `{} {}` -> `{}`: {} (unresolved: {})",
                endpoint.method,
                endpoint.path,
                endpoint.handler,
                endpoint.status,
                endpoint.unresolved
            )
            .unwrap();
        }
    }
    writeln!(out).unwrap();
    writeln!(out, "## Warnings").unwrap();
    if report.warnings.is_empty() {
        writeln!(out, "_No warnings._").unwrap();
    } else {
        for warning in &report.warnings {
            writeln!(out, "- {}", warning).unwrap();
        }
    }

    out
}

fn write_confidence_markdown(out: &mut String, counts: &ConfidenceCounts) {
    writeln!(out, "- resolved: {}", counts.resolved).unwrap();
    writeln!(out, "- single_impl: {}", counts.single_impl).unwrap();
    writeln!(out, "- primary: {}", counts.primary).unwrap();
    writeln!(out, "- qualifier: {}", counts.qualifier).unwrap();
    writeln!(out, "- ambiguous: {}", counts.ambiguous).unwrap();
    writeln!(out, "- external: {}", counts.external).unwrap();
    writeln!(out, "- unresolved: {}", counts.unresolved).unwrap();
}

fn render_doctor_json(report: &DoctorReport) -> Result<String> {
    let value = serde_json::json!({
        "summary": {
            "classes": report.class_count,
            "methods": report.method_count,
            "endpoints": report.endpoint_count,
            "flows_built": report.flows_built,
            "flow_errors": report.flow_errors,
        },
        "confidence": confidence_json(&report.confidence),
        "endpoints": report.endpoints.iter().map(|endpoint| {
            serde_json::json!({
                "method": endpoint.method,
                "path": endpoint.path,
                "handler": endpoint.handler,
                "status": endpoint.status,
                "unresolved": endpoint.unresolved,
                "error": endpoint.error,
                "confidence": confidence_json(&endpoint.confidence),
            })
        }).collect::<Vec<_>>(),
        "warnings": report.warnings,
    });

    Ok(serde_json::to_string_pretty(&value)?)
}

fn confidence_json(counts: &ConfidenceCounts) -> serde_json::Value {
    serde_json::json!({
        "resolved": counts.resolved,
        "single_impl": counts.single_impl,
        "primary": counts.primary,
        "qualifier": counts.qualifier,
        "ambiguous": counts.ambiguous,
        "external": counts.external,
        "unresolved": counts.unresolved,
    })
}

impl ConfidenceCounts {
    fn add(&mut self, other: Self) {
        self.resolved += other.resolved;
        self.single_impl += other.single_impl;
        self.primary += other.primary;
        self.qualifier += other.qualifier;
        self.ambiguous += other.ambiguous;
        self.external += other.external;
        self.unresolved += other.unresolved;
    }

    fn add_one(&mut self, confidence: &Confidence) {
        match confidence {
            Confidence::Resolved => self.resolved += 1,
            Confidence::SingleImpl => self.single_impl += 1,
            Confidence::Primary => self.primary += 1,
            Confidence::Qualifier => self.qualifier += 1,
            Confidence::Ambiguous => self.ambiguous += 1,
            Confidence::External => self.external += 1,
            Confidence::Unresolved => self.unresolved += 1,
        }
    }
}
