//! Mermaid flowchart renderer.

use std::fmt::Write;

use crate::model::{
    BranchKind, BranchNode, CallNode, Confidence, Flow, FlowNode, LambdaKind, LambdaNode, LoopNode,
};

use super::common::{
    control_kind_human_label, external_kind_human_label, is_low_signal_human_call,
    loop_execution_label, loop_kind_human_label, max_remaining_depth, short_method, single_line,
    truncated_marker, truncated_note,
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
        call_shape(&flow.root),
    );

    render_sequence(
        &mut out,
        &mut state,
        &[root_id],
        None,
        &flow.root.children,
        1,
        effective_max_depth,
    );

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

fn render_sequence(
    out: &mut String,
    state: &mut RenderState,
    entry_ids: &[String],
    first_edge_label: Option<&str>,
    nodes: &[FlowNode],
    depth: usize,
    max_depth: usize,
) -> Vec<String> {
    let mut exits = entry_ids.to_vec();
    let mut edge_label = first_edge_label;
    for node in nodes {
        if exits.is_empty() {
            break;
        }
        exits = render_flow_node(out, state, &exits, edge_label, node, depth, max_depth);
        edge_label = None;
    }
    exits
}

fn render_flow_node(
    out: &mut String,
    state: &mut RenderState,
    entry_ids: &[String],
    edge_label: Option<&str>,
    node: &FlowNode,
    depth: usize,
    max_depth: usize,
) -> Vec<String> {
    match node {
        FlowNode::Call(call) => {
            render_call_node(out, state, entry_ids, edge_label, call, depth, max_depth)
        }
        FlowNode::Lambda(lambda) => {
            render_lambda_node(out, state, entry_ids, edge_label, lambda, depth, max_depth)
        }
        FlowNode::Branch(branch) => {
            render_branch_node(out, state, entry_ids, edge_label, branch, depth, max_depth)
        }
        FlowNode::Loop(loop_node) => render_loop_node(
            out, state, entry_ids, edge_label, loop_node, depth, max_depth,
        ),
    }
}

fn render_call_node(
    out: &mut String,
    state: &mut RenderState,
    entry_ids: &[String],
    edge_label: Option<&str>,
    node: &CallNode,
    depth: usize,
    max_depth: usize,
) -> Vec<String> {
    if is_low_signal_human_call(node) {
        let mut exits = render_sequence(
            out,
            state,
            entry_ids,
            edge_label,
            &node.inputs,
            depth,
            max_depth,
        );
        if node.children.is_empty() && edge_label.is_some_and(|label| label.contains("terminates"))
        {
            render_terminal_nodes(out, state, &exits, Some("terminates"));
            return Vec::new();
        }

        if !node.children.is_empty() {
            exits = render_sequence(out, state, &exits, None, &node.children, depth, max_depth);
        }
        return exits;
    }

    if depth > max_depth {
        state.truncated = true;
        return vec![render_truncated_node(
            out,
            state,
            entry_ids,
            edge_label,
            1 + max_remaining_depth(node),
        )];
    }

    let node_id = state.next_node_id();
    write_node(out, &node_id, &call_label(node), call_shape(node));
    if node.inputs.is_empty() {
        write_edges(out, entry_ids, &node_id, edge_label);
    } else {
        let mut rendered_input = false;
        for input in &node.inputs {
            let input_ids =
                render_flow_node(out, state, entry_ids, edge_label, input, depth, max_depth);
            for input_id in input_ids {
                rendered_input = true;
                write_edge(out, &input_id, &node_id, Some("input"));
            }
        }
        if !rendered_input {
            write_edges(out, entry_ids, &node_id, edge_label);
        }
    }

    if !node.children.is_empty() && depth >= max_depth {
        state.truncated = true;
        render_truncated_node(
            out,
            state,
            std::slice::from_ref(&node_id),
            None,
            max_remaining_depth(node),
        );
        return vec![node_id];
    }

    if node.children.is_empty() {
        vec![node_id]
    } else {
        render_sequence(
            out,
            state,
            &[node_id],
            None,
            &node.children,
            depth + 1,
            max_depth,
        )
    }
}

fn render_lambda_node(
    out: &mut String,
    state: &mut RenderState,
    entry_ids: &[String],
    edge_label: Option<&str>,
    lambda: &LambdaNode,
    depth: usize,
    max_depth: usize,
) -> Vec<String> {
    let node_id = state.next_node_id();
    write_node(out, &node_id, &lambda_label(lambda), Shape::Process);
    write_edges(out, entry_ids, &node_id, edge_label);

    if lambda.children.is_empty() {
        vec![node_id]
    } else {
        render_sequence(
            out,
            state,
            &[node_id],
            None,
            &lambda.children,
            depth,
            max_depth,
        )
    }
}

fn render_branch_node(
    out: &mut String,
    state: &mut RenderState,
    entry_ids: &[String],
    edge_label: Option<&str>,
    branch: &BranchNode,
    arm_call_depth: usize,
    max_depth: usize,
) -> Vec<String> {
    let node_id = state.next_node_id();
    write_node(out, &node_id, &branch_label(branch), Shape::Decision);
    if branch.condition.is_empty() {
        write_edges(out, entry_ids, &node_id, edge_label);
    } else {
        let condition_ids = render_sequence(
            out,
            state,
            entry_ids,
            edge_label,
            &branch.condition,
            arm_call_depth,
            max_depth,
        );
        if condition_ids.is_empty() {
            write_edges(out, entry_ids, &node_id, edge_label);
        } else {
            write_edges(out, &condition_ids, &node_id, Some("condition"));
        }
    }

    let mut exits = Vec::new();
    for arm in &branch.arms {
        let label = arm_label(branch.kind, &arm.label, arm.terminates);
        if arm.children.is_empty() {
            if arm.terminates {
                render_terminal_nodes(out, state, std::slice::from_ref(&node_id), Some(&label));
            } else {
                push_exit(&mut exits, node_id.clone());
            }
            continue;
        }

        let arm_exits = render_sequence(
            out,
            state,
            std::slice::from_ref(&node_id),
            Some(&label),
            &arm.children,
            arm_call_depth,
            max_depth,
        );
        if arm.terminates {
            render_terminal_nodes(out, state, &arm_exits, Some("terminates"));
        } else {
            for exit in arm_exits {
                push_exit(&mut exits, exit);
            }
        }
    }

    if has_implicit_fallthrough(branch) {
        push_exit(&mut exits, node_id);
    }

    exits
}

fn render_loop_node(
    out: &mut String,
    state: &mut RenderState,
    entry_ids: &[String],
    edge_label: Option<&str>,
    loop_node: &LoopNode,
    section_call_depth: usize,
    max_depth: usize,
) -> Vec<String> {
    let node_id = state.next_node_id();
    write_node(out, &node_id, &loop_label(loop_node), Shape::Loop);
    write_edges(out, entry_ids, &node_id, edge_label);

    let init_exits = render_sequence(
        out,
        state,
        std::slice::from_ref(&node_id),
        Some("init"),
        &loop_node.init,
        section_call_depth,
        max_depth,
    );
    let loop_entry = if init_exits.is_empty() {
        vec![node_id.clone()]
    } else {
        init_exits
    };
    let condition_exits = render_sequence(
        out,
        state,
        &loop_entry,
        Some("condition"),
        &loop_node.condition,
        section_call_depth,
        max_depth,
    );
    let body_entries = if condition_exits.is_empty() {
        vec![node_id.clone()]
    } else {
        condition_exits
    };
    for arm in &loop_node.arms {
        let body_exits = render_sequence(
            out,
            state,
            &body_entries,
            Some(&arm.label),
            &arm.children,
            section_call_depth,
            max_depth,
        );
        let update_exits = render_sequence(
            out,
            state,
            &body_exits,
            Some("update"),
            &loop_node.update,
            section_call_depth,
            max_depth,
        );
        write_edges(out, &update_exits, &node_id, Some("next"));
    }
    vec![node_id]
}

fn render_truncated_node(
    out: &mut String,
    state: &mut RenderState,
    entry_ids: &[String],
    edge_label: Option<&str>,
    remaining_levels: usize,
) -> String {
    let node_id = state.next_node_id();
    write_node(
        out,
        &node_id,
        &truncated_marker(remaining_levels),
        Shape::Process,
    );
    write_edges(out, entry_ids, &node_id, edge_label);
    node_id
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

fn call_shape(node: &CallNode) -> Shape {
    if node.control_kind.is_some() {
        return Shape::Control;
    }

    match node.confidence {
        Confidence::Resolved
        | Confidence::SingleImpl
        | Confidence::Primary
        | Confidence::Qualifier => Shape::Subroutine,
        Confidence::External => Shape::External,
        Confidence::Ambiguous | Confidence::Unresolved => Shape::Process,
    }
}

fn has_implicit_fallthrough(branch: &BranchNode) -> bool {
    match branch.kind {
        BranchKind::If => !branch.arms.iter().any(|arm| arm.label == "else"),
        BranchKind::Switch => !branch.arms.iter().any(|arm| arm.label == "default"),
        BranchKind::Ternary | BranchKind::TryCatch | BranchKind::Optional => false,
    }
}

fn render_terminal_nodes(
    out: &mut String,
    state: &mut RenderState,
    entry_ids: &[String],
    edge_label: Option<&str>,
) {
    for entry_id in entry_ids {
        let terminal_id = state.next_node_id();
        write_node(out, &terminal_id, "terminates", Shape::Terminator);
        write_edge(out, entry_id, &terminal_id, edge_label);
    }
}

fn push_exit(exits: &mut Vec<String>, exit: String) {
    if !exits.contains(&exit) {
        exits.push(exit);
    }
}

fn write_node(out: &mut String, id: &str, label: &str, shape: Shape) {
    let label = escape_mermaid_text(label);
    match shape {
        Shape::Process => writeln!(out, "    {id}[\"{label}\"]").unwrap(),
        Shape::Subroutine => writeln!(out, "    {id}[[\"{label}\"]]").unwrap(),
        Shape::External => writeln!(out, "    {id}([\"{label}\"])").unwrap(),
        Shape::Control | Shape::Loop => writeln!(out, "    {id}{{{{\"{label}\"}}}}").unwrap(),
        Shape::Decision => writeln!(out, "    {id}{{\"{label}\"}}").unwrap(),
        Shape::Terminator => writeln!(out, "    {id}([\"{label}\"])").unwrap(),
    }
}

fn write_edges(out: &mut String, from_ids: &[String], to: &str, label: Option<&str>) {
    for from in from_ids {
        write_edge(out, from, to, label);
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
    Process,
    Subroutine,
    External,
    Control,
    Loop,
    Decision,
    Terminator,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::model::{
        Arm, BranchKind, BranchNode, CallNode, Confidence, Endpoint, ExternalKind, Flow, FlowNode,
        Fqn, HttpVerb, ParamInfo, ParamSource,
    };

    use super::render;

    #[test]
    fn sibling_calls_render_in_source_order() {
        let flow = flow_with_children(vec![
            FlowNode::Call(node("example.Service#first()", Vec::new())),
            FlowNode::Call(node("example.Service#second()", Vec::new())),
        ]);

        let rendered = render(&flow, None);

        assert!(rendered.contains("    n0 --> n1"));
        assert!(rendered.contains("    n1 --> n2"));
    }

    #[test]
    fn terminating_if_arm_does_not_flow_into_next_sibling() {
        let flow = flow_with_children(vec![
            FlowNode::Branch(BranchNode {
                kind: BranchKind::If,
                condition_src: "age < 18".to_string(),
                condition: Vec::new(),
                arms: vec![Arm {
                    label: "then".to_string(),
                    terminates: true,
                    children: vec![FlowNode::Call(node(
                        "example.BusinessException#<init>()",
                        Vec::new(),
                    ))],
                }],
            }),
            FlowNode::Call(node("example.Service#persist()", Vec::new())),
        ]);

        let rendered = render(&flow, None);

        assert!(rendered.contains("    n1 -->|\"then terminates\"| n2"));
        assert!(rendered.contains("    n2 -->|\"terminates\"| n3"));
        assert!(rendered.contains("    n1 --> n4"));
        assert!(!rendered.contains("    n2 --> n4"));
    }

    fn flow_with_children(children: Vec<FlowNode>) -> Flow {
        Flow {
            endpoint: Endpoint {
                verb: HttpVerb::Get,
                path: "/test".to_string(),
                handler_fqn: Fqn("example.Controller#handle()".to_string()),
                file: PathBuf::from("Controller.java"),
                line: 1,
            },
            inputs: vec![ParamInfo {
                name: "id".to_string(),
                ty: "Long".to_string(),
                source: ParamSource::Path,
                annotations: Vec::new(),
                validation: Vec::new(),
            }],
            root: node("example.Controller#handle()", children),
            unresolved: Vec::new(),
            notes: vec!["note".to_string()],
        }
    }

    fn node(fqn: &str, children: Vec<FlowNode>) -> CallNode {
        CallNode {
            method_fqn: Fqn(fqn.to_string()),
            confidence: Confidence::Resolved,
            external_kind: Some(ExternalKind::Unknown),
            control_kind: None,
            scope: None,
            note: None,
            inputs: Vec::new(),
            children,
        }
    }
}
