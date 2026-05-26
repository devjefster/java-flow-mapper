//! Mermaid sequence diagram renderer.

use std::collections::HashSet;
use std::fmt::Write;

use crate::model::{BranchKind, BranchNode, CallNode, Confidence, Flow, FlowNode, LoopNode};

use super::common::{
    control_kind_human_label, external_kind_human_label, loop_kind_human_label,
    max_remaining_depth, single_line, truncated_marker, truncated_note,
};

const DEFAULT_MAX_DEPTH: usize = 5;

/// Render a flow as a fenced Mermaid sequence diagram.
pub fn render(flow: &Flow, max_depth: Option<usize>) -> String {
    let effective_max_depth = max_depth.unwrap_or(DEFAULT_MAX_DEPTH);
    let mut truncated = false;
    let mut out = String::new();
    writeln!(out, "```mermaid").unwrap();
    writeln!(out, "sequenceDiagram").unwrap();

    let participants = participants(flow, effective_max_depth);
    for participant in &participants {
        writeln!(out, "    participant {participant}").unwrap();
    }

    for child in &flow.root.children {
        render_flow_node(
            &mut out,
            flow.root.method_fqn.0.as_str(),
            child,
            1,
            effective_max_depth,
            &mut truncated,
        );
    }

    writeln!(out, "    %% Notes:").unwrap();
    for note in &flow.notes {
        writeln!(out, "    %% - {note}").unwrap();
    }
    writeln!(
        out,
        "    %% - Return arrows are not rendered: CallNode does not yet track return types."
    )
    .unwrap();
    if truncated {
        writeln!(out, "    %% - {}", truncated_note(effective_max_depth)).unwrap();
    }
    writeln!(out, "```").unwrap();
    out
}

fn render_flow_node(
    out: &mut String,
    caller_fqn: &str,
    node: &FlowNode,
    depth: usize,
    max_depth: usize,
    truncated: &mut bool,
) {
    match node {
        FlowNode::Call(call) => render_edge(out, caller_fqn, call, depth, max_depth, truncated),
        FlowNode::Lambda(lambda) => {
            for child in &lambda.children {
                render_flow_node(out, caller_fqn, child, depth, max_depth, truncated);
            }
        }
        FlowNode::Branch(branch) => {
            render_branch(out, caller_fqn, branch, depth, max_depth, truncated);
        }
        FlowNode::Loop(loop_node) => {
            render_loop(out, caller_fqn, loop_node, depth, max_depth, truncated);
        }
    }
}

fn render_edge(
    out: &mut String,
    caller_fqn: &str,
    node: &CallNode,
    depth: usize,
    max_depth: usize,
    truncated: &mut bool,
) {
    if depth > max_depth {
        *truncated = true;
        return;
    }

    let caller = class_name(caller_fqn);
    let callee = class_name(&node.method_fqn.0);
    writeln!(
        out,
        "    {}->>{}: {}",
        caller,
        callee,
        method_signature(&node.method_fqn.0)
    )
    .unwrap();

    if let Some(kind) = &node.control_kind {
        writeln!(
            out,
            "    Note over {}: control flow ({})",
            callee,
            control_kind_human_label(kind)
        )
        .unwrap();
    } else {
        match node.confidence {
            Confidence::External => {
                writeln!(
                    out,
                    "    Note over {}: external ({})",
                    callee,
                    external_kind_human_label(node.external_kind.as_ref())
                )
                .unwrap();
            }
            Confidence::Unresolved => {
                if let Some(note) = &node.note {
                    writeln!(out, "    Note over {callee}: unresolved ({note})").unwrap();
                }
            }
            _ => {}
        }
    }

    if !node.children.is_empty() && depth >= max_depth {
        *truncated = true;
        writeln!(
            out,
            "    Note over {}: {}",
            callee,
            truncated_marker(max_remaining_depth(node))
        )
        .unwrap();
        return;
    }

    for child in &node.children {
        render_flow_node(
            out,
            &node.method_fqn.0,
            child,
            depth + 1,
            max_depth,
            truncated,
        );
    }
}

fn render_branch(
    out: &mut String,
    caller_fqn: &str,
    branch: &BranchNode,
    arm_call_depth: usize,
    max_depth: usize,
    truncated: &mut bool,
) {
    let condition = match branch.kind {
        BranchKind::If => mermaid_condition(&branch.condition_src),
        BranchKind::Switch => format!(
            "switch {} case {}",
            mermaid_condition(&branch.condition_src),
            branch
                .arms
                .first()
                .map(|arm| arm.label.as_str())
                .unwrap_or("")
        ),
        BranchKind::Optional => format!("optional {}", branch.arms[0].label),
    };

    writeln!(out, "    alt {condition}").unwrap();
    for (idx, arm) in branch.arms.iter().enumerate() {
        if idx > 0 {
            let label = match branch.kind {
                BranchKind::If => "else".to_string(),
                BranchKind::Switch => format!("else case {}", arm.label),
                BranchKind::Optional => format!("else optional {}", arm.label),
            };
            writeln!(out, "    {label}").unwrap();
        }
        for child in &arm.children {
            render_flow_node(out, caller_fqn, child, arm_call_depth, max_depth, truncated);
        }
    }
    writeln!(out, "    end").unwrap();
}

fn render_loop(
    out: &mut String,
    caller_fqn: &str,
    loop_node: &LoopNode,
    section_call_depth: usize,
    max_depth: usize,
    truncated: &mut bool,
) {
    writeln!(out, "    loop {}", loop_label(loop_node)).unwrap();
    render_children(
        out,
        caller_fqn,
        &loop_node.condition,
        section_call_depth,
        max_depth,
        truncated,
    );
    render_children(
        out,
        caller_fqn,
        &loop_node.body,
        section_call_depth,
        max_depth,
        truncated,
    );
    render_children(
        out,
        caller_fqn,
        &loop_node.update,
        section_call_depth,
        max_depth,
        truncated,
    );
    writeln!(out, "    end").unwrap();
}

fn render_children(
    out: &mut String,
    caller_fqn: &str,
    children: &[FlowNode],
    depth: usize,
    max_depth: usize,
    truncated: &mut bool,
) {
    for child in children {
        render_flow_node(out, caller_fqn, child, depth, max_depth, truncated);
    }
}

fn participants(flow: &Flow, max_depth: usize) -> Vec<String> {
    let mut participants = Vec::new();
    let mut seen = HashSet::new();
    push_participant(
        &mut participants,
        &mut seen,
        class_name(&flow.root.method_fqn.0),
    );
    for child in &flow.root.children {
        collect_participants(child, 1, max_depth, &mut participants, &mut seen);
    }
    participants
}

fn collect_participants(
    node: &FlowNode,
    depth: usize,
    max_depth: usize,
    participants: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match node {
        FlowNode::Call(call) => {
            if depth > max_depth {
                return;
            }

            push_participant(participants, seen, class_name(&call.method_fqn.0));
            if depth >= max_depth {
                return;
            }

            for child in &call.children {
                collect_participants(child, depth + 1, max_depth, participants, seen);
            }
        }
        FlowNode::Lambda(lambda) => {
            for child in &lambda.children {
                collect_participants(child, depth, max_depth, participants, seen);
            }
        }
        FlowNode::Branch(branch) => {
            for arm in &branch.arms {
                for child in &arm.children {
                    collect_participants(child, depth, max_depth, participants, seen);
                }
            }
        }
        FlowNode::Loop(loop_node) => {
            for child in loop_node
                .condition
                .iter()
                .chain(&loop_node.body)
                .chain(&loop_node.update)
            {
                collect_participants(child, depth, max_depth, participants, seen);
            }
        }
    }
}

fn push_participant(
    participants: &mut Vec<String>,
    seen: &mut HashSet<String>,
    participant: String,
) {
    if seen.insert(participant.clone()) {
        participants.push(participant);
    }
}

fn class_name(method_fqn: &str) -> String {
    method_fqn
        .split('#')
        .next()
        .unwrap_or(method_fqn)
        .rsplit('.')
        .next()
        .unwrap_or(method_fqn)
        .to_string()
}

fn method_signature(method_fqn: &str) -> String {
    let signature = method_fqn.split('#').nth(1).unwrap_or(method_fqn);
    if signature.starts_with("<init>(") {
        "<init>(...)".to_string()
    } else {
        single_line(signature)
    }
}

fn mermaid_condition(condition: &str) -> String {
    single_line(condition)
}

fn loop_label(loop_node: &LoopNode) -> String {
    format!(
        "{} {}",
        loop_kind_human_label(loop_node.kind),
        single_line(&loop_node.source)
    )
}
