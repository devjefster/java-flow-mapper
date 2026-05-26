//! Mermaid flowchart renderer.

use std::fmt::Write;

use crate::model::{
    BranchKind, BranchNode, CallNode, Confidence, Flow, FlowNode, LambdaKind, LambdaNode, LoopNode,
};

use super::common::{
    control_kind_human_label, external_kind_human_label, loop_execution_label,
    loop_kind_human_label, max_remaining_depth, short_method, single_line, truncated_marker,
    truncated_note,
};

const DEFAULT_MAX_DEPTH: usize = 5;

/// Render a flow as a fenced Mermaid flowchart.
pub fn render(flow: &Flow, max_depth: Option<usize>) -> String {
    let effective_max_depth = max_depth.unwrap_or(DEFAULT_MAX_DEPTH);
    let mut state = RenderState::default();
    let mut out = String::new();

    writeln!(out, "```mermaid").unwrap();
    writeln!(out, "flowchart TD").unwrap();

    let root_id = state.next_node_id();
    write_node(
        &mut out,
        &root_id,
        &short_method(&flow.root.method_fqn.0),
        Shape::Rect,
    );

    for child in &flow.root.children {
        render_flow_node(
            &mut out,
            &mut state,
            &root_id,
            None,
            child,
            1,
            effective_max_depth,
        );
    }

    writeln!(out, "    %% Notes:").unwrap();
    for note in &flow.notes {
        writeln!(out, "    %% - {note}").unwrap();
    }
    if state.truncated {
        writeln!(out, "    %% - {}", truncated_note(effective_max_depth)).unwrap();
    }
    writeln!(out, "```").unwrap();

    out
}

fn render_flow_node(
    out: &mut String,
    state: &mut RenderState,
    parent_id: &str,
    edge_label: Option<&str>,
    node: &FlowNode,
    depth: usize,
    max_depth: usize,
) {
    match node {
        FlowNode::Call(call) => {
            render_call_node(out, state, parent_id, edge_label, call, depth, max_depth);
        }
        FlowNode::Lambda(lambda) => {
            render_lambda_node(out, state, parent_id, edge_label, lambda, depth, max_depth);
        }
        FlowNode::Branch(branch) => {
            render_branch_node(out, state, parent_id, edge_label, branch, depth, max_depth);
        }
        FlowNode::Loop(loop_node) => {
            render_loop_node(
                out, state, parent_id, edge_label, loop_node, depth, max_depth,
            );
        }
    }
}

fn render_call_node(
    out: &mut String,
    state: &mut RenderState,
    parent_id: &str,
    edge_label: Option<&str>,
    node: &CallNode,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        state.truncated = true;
        render_truncated_node(
            out,
            state,
            parent_id,
            edge_label,
            1 + max_remaining_depth(node),
        );
        return;
    }

    let node_id = state.next_node_id();
    write_node(out, &node_id, &call_label(node), Shape::Rect);
    write_edge(out, parent_id, &node_id, edge_label);

    if !node.children.is_empty() && depth >= max_depth {
        state.truncated = true;
        render_truncated_node(out, state, &node_id, None, max_remaining_depth(node));
        return;
    }

    for child in &node.children {
        render_flow_node(out, state, &node_id, None, child, depth + 1, max_depth);
    }
}

fn render_lambda_node(
    out: &mut String,
    state: &mut RenderState,
    parent_id: &str,
    edge_label: Option<&str>,
    lambda: &LambdaNode,
    depth: usize,
    max_depth: usize,
) {
    let node_id = state.next_node_id();
    write_node(out, &node_id, &lambda_label(lambda), Shape::Rect);
    write_edge(out, parent_id, &node_id, edge_label);

    for child in &lambda.children {
        render_flow_node(out, state, &node_id, None, child, depth, max_depth);
    }
}

fn render_branch_node(
    out: &mut String,
    state: &mut RenderState,
    parent_id: &str,
    edge_label: Option<&str>,
    branch: &BranchNode,
    arm_call_depth: usize,
    max_depth: usize,
) {
    let node_id = state.next_node_id();
    write_node(out, &node_id, &branch_label(branch), Shape::Decision);
    write_edge(out, parent_id, &node_id, edge_label);

    for arm in &branch.arms {
        let label = arm_label(branch.kind, &arm.label, arm.terminates);
        if arm.children.is_empty() {
            if arm.terminates {
                let terminal_id = state.next_node_id();
                write_node(out, &terminal_id, "terminates", Shape::Rect);
                write_edge(out, &node_id, &terminal_id, Some(&label));
            }
            continue;
        }

        for child in &arm.children {
            render_flow_node(
                out,
                state,
                &node_id,
                Some(&label),
                child,
                arm_call_depth,
                max_depth,
            );
        }
    }
}

fn render_loop_node(
    out: &mut String,
    state: &mut RenderState,
    parent_id: &str,
    edge_label: Option<&str>,
    loop_node: &LoopNode,
    section_call_depth: usize,
    max_depth: usize,
) {
    let node_id = state.next_node_id();
    write_node(out, &node_id, &loop_label(loop_node), Shape::Rect);
    write_edge(out, parent_id, &node_id, edge_label);

    render_loop_section(
        out,
        state,
        &node_id,
        "init",
        &loop_node.init,
        section_call_depth,
        max_depth,
    );
    render_loop_section(
        out,
        state,
        &node_id,
        "condition",
        &loop_node.condition,
        section_call_depth,
        max_depth,
    );
    for arm in &loop_node.arms {
        render_loop_section(
            out,
            state,
            &node_id,
            &arm.label,
            &arm.children,
            section_call_depth,
            max_depth,
        );
    }
    render_loop_section(
        out,
        state,
        &node_id,
        "update",
        &loop_node.update,
        section_call_depth,
        max_depth,
    );
}

fn render_loop_section(
    out: &mut String,
    state: &mut RenderState,
    parent_id: &str,
    label: &str,
    children: &[FlowNode],
    depth: usize,
    max_depth: usize,
) {
    for child in children {
        render_flow_node(out, state, parent_id, Some(label), child, depth, max_depth);
    }
}

fn render_truncated_node(
    out: &mut String,
    state: &mut RenderState,
    parent_id: &str,
    edge_label: Option<&str>,
    remaining_levels: usize,
) {
    let node_id = state.next_node_id();
    write_node(
        out,
        &node_id,
        &truncated_marker(remaining_levels),
        Shape::Rect,
    );
    write_edge(out, parent_id, &node_id, edge_label);
}

fn call_label(node: &CallNode) -> String {
    let mut label = short_method(&node.method_fqn.0);
    if let Some(kind) = &node.control_kind {
        push_label_detail(
            &mut label,
            &format!("control flow ({})", control_kind_human_label(kind)),
        );
        return label;
    }

    match node.confidence {
        Confidence::External => push_label_detail(
            &mut label,
            &format!(
                "external ({})",
                external_kind_human_label(node.external_kind.as_ref())
            ),
        ),
        Confidence::Unresolved => {
            if let Some(note) = &node.note {
                push_label_detail(&mut label, &format!("unresolved ({note})"));
            } else {
                push_label_detail(&mut label, "unresolved");
            }
        }
        Confidence::SingleImpl => push_label_detail(&mut label, "single implementation"),
        Confidence::Primary => push_label_detail(&mut label, "primary bean"),
        Confidence::Qualifier => push_label_detail(&mut label, "qualified bean"),
        Confidence::Ambiguous => push_label_detail(&mut label, "ambiguous"),
        Confidence::Resolved => {}
    }

    label
}

fn push_label_detail(label: &mut String, detail: &str) {
    label.push_str(" - ");
    label.push_str(detail);
}

fn lambda_label(lambda: &LambdaNode) -> String {
    let kind = match lambda.kind {
        LambdaKind::Lambda => "lambda",
        LambdaKind::MethodRef => "method ref",
    };
    format!("{kind}: {}", single_line(&lambda.source))
}

fn branch_label(branch: &BranchNode) -> String {
    match branch.kind {
        BranchKind::If => format!("if {}", single_line(&branch.condition_src)),
        BranchKind::Switch => format!("switch {}", single_line(&branch.condition_src)),
        BranchKind::Ternary => format!("ternary {}", single_line(&branch.condition_src)),
        BranchKind::TryCatch => "try/catch".to_string(),
        BranchKind::Optional => "optional".to_string(),
    }
}

fn arm_label(kind: BranchKind, label: &str, terminates: bool) -> String {
    let mut rendered = match (kind, label) {
        (BranchKind::If, "then") => "then".to_string(),
        (BranchKind::If, "else") => "else".to_string(),
        (BranchKind::Switch, "default") => "default".to_string(),
        (BranchKind::Switch, label) => format!("case {label}"),
        (BranchKind::Ternary, "then") => "then".to_string(),
        (BranchKind::Ternary, "else") => "else".to_string(),
        (BranchKind::TryCatch, label) => label.to_string(),
        (BranchKind::Optional, label) => label.to_string(),
        (BranchKind::If | BranchKind::Ternary, label) => label.to_string(),
    };
    if terminates {
        rendered.push_str(" terminates");
    }
    rendered
}

fn loop_label(loop_node: &LoopNode) -> String {
    format!(
        "loop {} {} ({})",
        loop_kind_human_label(loop_node.kind),
        single_line(&loop_node.source),
        loop_execution_label(loop_node.execution)
    )
}

fn write_node(out: &mut String, id: &str, label: &str, shape: Shape) {
    let label = escape_mermaid_text(label);
    match shape {
        Shape::Rect => writeln!(out, "    {id}[\"{label}\"]").unwrap(),
        Shape::Decision => writeln!(out, "    {id}{{\"{label}\"}}").unwrap(),
    }
}

fn write_edge(out: &mut String, from: &str, to: &str, label: Option<&str>) {
    if let Some(label) = label {
        writeln!(
            out,
            "    {from} -->|\"{}\"| {to}",
            escape_mermaid_text(label)
        )
        .unwrap();
    } else {
        writeln!(out, "    {from} --> {to}").unwrap();
    }
}

fn escape_mermaid_text(value: &str) -> String {
    value
        .replace('\n', " ")
        .replace('"', "'")
        .replace('[', "(")
        .replace(']', ")")
}

#[derive(Default)]
struct RenderState {
    next_node: usize,
    truncated: bool,
}

impl RenderState {
    fn next_node_id(&mut self) -> String {
        let id = format!("n{}", self.next_node);
        self.next_node += 1;
        id
    }
}

#[derive(Clone, Copy)]
enum Shape {
    Rect,
    Decision,
}
