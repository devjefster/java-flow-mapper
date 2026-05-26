//! Output rendering dispatch for supported flow formats.

mod common;
mod json;
mod markdown;
mod mermaid;
mod mermaid_flowchart;

use crate::model::{Diagram, Flow, Format};

/// Render a flow in the requested format.
///
/// Render-time `max_depth` trims output only; it is independent from
/// `flow::MAX_DEPTH`, which protects graph construction from runaway recursion.
/// Control nodes (branches, loops, lambda wrappers, and arms) do not count
/// toward render depth; only calls do.
pub fn render(flow: &Flow, format: Format, diagram: Diagram, max_depth: Option<usize>) -> String {
    match format {
        Format::Markdown => markdown::render(flow, max_depth),
        Format::Json => json::render(flow, max_depth),
        Format::Mermaid => match diagram {
            Diagram::Sequence => mermaid::render(flow, max_depth),
            Diagram::Flowchart => mermaid_flowchart::render(flow, max_depth),
        },
    }
}
