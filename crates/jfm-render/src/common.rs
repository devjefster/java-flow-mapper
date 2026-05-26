//! Shared renderer helpers for depth accounting and labels.

use crate::model::{
    CallNode, Confidence, ControlKind, ExternalKind, FlowNode, LoopExecution, LoopKind,
};

/// Render-time `max_depth` trims output only; it is independent from
/// `flow::MAX_DEPTH`, which protects graph construction from runaway recursion.
/// Control nodes (branches, loops, lambda wrappers, and arms) do not count
/// toward render depth; only calls do.
pub(super) fn max_remaining_depth(node: &CallNode) -> usize {
    node.inputs
        .iter()
        .chain(&node.children)
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
            .condition
            .iter()
            .chain(branch.arms.iter().flat_map(|arm| &arm.children))
            .map(flow_node_remaining_depth)
            .max()
            .unwrap_or(0),
        FlowNode::Loop(loop_node) => loop_node
            .init
            .iter()
            .chain(&loop_node.condition)
            .chain(loop_node.arms.iter().flat_map(|arm| &arm.children))
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

pub(super) fn is_low_signal_human_call(node: &CallNode) -> bool {
    node.control_kind.is_none()
        && matches!(node.confidence, Confidence::External)
        && node.external_kind.as_ref() == Some(&ExternalKind::JdkLibrary)
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
        Some(ExternalKind::JdkLibrary) => "JDK library",
        Some(ExternalKind::SpringData) => "Spring Data JPA",
        Some(ExternalKind::ThirdParty) => "third party",
        Some(ExternalKind::Unknown) | None => "unknown",
    }
}

pub(super) fn control_kind_human_label(kind: &ControlKind) -> &'static str {
    match kind {
        ControlKind::Optional => "Optional",
        ControlKind::Traversal => "traversal",
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

pub(super) fn loop_execution_label(execution: LoopExecution) -> &'static str {
    match execution {
        LoopExecution::ZeroOrMore => "0..n",
        LoopExecution::OneOrMore => "1..n",
    }
}
