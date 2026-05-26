//! CLI parsing and command orchestration.
//!
//! This module keeps user input validation near Clap and delegates indexing,
//! flow construction, and rendering to their dedicated modules.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use jfm_flow as flow;
use jfm_model::{Diagram, Format, HttpVerb};
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
    /// Render a call flow for an HTTP endpoint.
    Flow {
        /// Java project root to parse.
        root: PathBuf,
        /// Endpoint selector, for example: "GET /users/{id}".
        endpoint: String,
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

/// Parse CLI arguments, run the selected command, and print its output.
pub fn run() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Flow {
            root,
            endpoint,
            format,
            diagram,
            max_depth,
        } => {
            let (verb, path) = parse_endpoint(&endpoint)?;
            let index = parser::index_project(&root)
                .with_context(|| format!("while parsing {}", root.display()))?;
            let flow = flow::build_flow(&index, verb, &path)?;
            print!("{}", render::render(&flow, format, diagram, max_depth));
            Ok(())
        }
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
