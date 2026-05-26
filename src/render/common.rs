use crate::model::{CallNode, ControlKind, ExternalKind, FlowNode, LoopKind};

/// Render-time `max_depth` trims output only; it is independent from
/// `flow::MAX_DEPTH`, which protects graph construction from runaway recursion.
/// Control nodes (branches, loops, lambda wrappers, and arms) do not count
/// toward render depth; only calls do.
pub(super) fn max_remaining_depth(node: &CallNode) -> usize {
    node.children
        .iter()
        .map(flow_node_remaining_depth)
        .max()
        .unwrap_or(0)
}

fn flow_node_remaining_depth(node: &FlowNode) -> usize {
    match node {
        FlowNode::Call(call) => 1 + max_remaining_depth(call),
        FlowNode::Lambda(lambda) => lambda
            .children
            .iter()
            .map(flow_node_remaining_depth)
            .max()
            .unwrap_or(0),
        FlowNode::Branch(branch) => branch
            .arms
            .iter()
            .flat_map(|arm| &arm.children)
            .map(flow_node_remaining_depth)
            .max()
            .unwrap_or(0),
        FlowNode::Loop(loop_node) => loop_node
            .condition
            .iter()
            .chain(&loop_node.body)
            .chain(&loop_node.update)
            .map(flow_node_remaining_depth)
            .max()
            .unwrap_or(0),
    }
}

pub(super) fn truncated_note(max_depth: usize) -> String {
    format!("Output truncated at depth {max_depth} (see --max-depth).")
}

pub(super) fn truncated_marker(remaining_levels: usize) -> String {
    let suffix = if remaining_levels == 1 {
        "level"
    } else {
        "levels"
    };
    format!("(truncated, {remaining_levels} more {suffix})")
}

pub(super) fn short_method(fqn: &str) -> String {
    let Some((class, method)) = fqn.split_once('#') else {
        return fqn.rsplit('.').next().unwrap_or(fqn).to_string();
    };
    format!("{}#{}", class.rsplit('.').next().unwrap_or(class), method)
}

pub(super) fn single_line(source: &str) -> String {
    source.replace('\n', " ")
}

pub(super) fn external_kind_human_label(kind: Option<&ExternalKind>) -> &'static str {
    match kind {
        Some(ExternalKind::Jdk) => "JDK",
        Some(ExternalKind::SpringData) => "Spring Data JPA",
        Some(ExternalKind::ThirdParty) => "third party",
        Some(ExternalKind::Unknown) | None => "unknown",
    }
}

pub(super) fn control_kind_human_label(kind: &ControlKind) -> &'static str {
    match kind {
        ControlKind::Optional => "Optional",
    }
}

pub(super) fn loop_kind_human_label(kind: LoopKind) -> &'static str {
    match kind {
        LoopKind::For => "for",
        LoopKind::EnhancedFor => "for-each",
        LoopKind::While => "while",
        LoopKind::DoWhile => "do-while",
        LoopKind::ForEach => "forEach",
        LoopKind::Stream => "stream",
    }
}
