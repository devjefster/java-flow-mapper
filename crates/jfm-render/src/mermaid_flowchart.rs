//! Mermaid flowchart renderer.

use std::collections::{HashMap, HashSet};
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
    let mut state = RenderState {
        reusable_counts: count_reusable_nodes(flow, effective_max_depth),
        ..RenderState::default()
    };
    let mut body = String::new();

    let root_id = state.next_node_id();
    write_unique_node(
        &mut body,
        &mut state,
        &root_id,
        &short_method(&flow.root.method_fqn.0),
        call_shape(&flow.root),
    );

    render_sequence(
        &mut body,
        &mut state,
        &[root_id],
        None,
        &flow.root.children,
        1,
        effective_max_depth,
    );

    let mut out = String::new();
    writeln!(out, "```mermaid").unwrap();
    writeln!(out, "flowchart TD").unwrap();
    out.push_str(&state.global_declarations);
    out.push_str(&body);
    out.push_str(&state.global_node_declarations);
    if !state.exception_style_nodes.is_empty() {
        writeln!(
            out,
            "    classDef exceptionFlow fill:#fde2e2,stroke:#dc2626,color:#7f1d1d"
        )
        .unwrap();
        writeln!(
            out,
            "    class {} exceptionFlow",
            state.exception_style_nodes.join(",")
        )
        .unwrap();
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

fn render_dependency_nodes(
    out: &mut String,
    state: &mut RenderState,
    owner_id: &str,
    edge_label: &str,
    nodes: &[FlowNode],
    depth: usize,
    max_depth: usize,
) {
    for node in nodes {
        render_flow_node(
            out,
            state,
            &[owner_id.to_string()],
            Some(edge_label),
            node,
            depth,
            max_depth,
        );
    }
}

fn count_reusable_nodes(flow: &Flow, max_depth: usize) -> HashMap<NodeKey, usize> {
    let mut counts = HashMap::new();
    for child in &flow.root.children {
        count_flow_node(child, 1, max_depth, &mut counts);
    }
    counts
}

fn count_flow_node(
    node: &FlowNode,
    depth: usize,
    max_depth: usize,
    counts: &mut HashMap<NodeKey, usize>,
) {
    match node {
        FlowNode::Call(call) => count_call_node(call, depth, max_depth, counts),
        FlowNode::Lambda(lambda) => {
            for child in &lambda.children {
                count_flow_node(child, depth, max_depth, counts);
            }
        }
        FlowNode::Branch(branch) => {
            for child in &branch.condition {
                count_flow_node(child, depth, max_depth, counts);
            }
            for arm in &branch.arms {
                for child in &arm.children {
                    count_flow_node(child, depth, max_depth, counts);
                }
            }
        }
        FlowNode::Loop(loop_node) => {
            for child in loop_node
                .init
                .iter()
                .chain(&loop_node.condition)
                .chain(loop_node.arms.iter().flat_map(|arm| &arm.children))
                .chain(&loop_node.update)
            {
                count_flow_node(child, depth, max_depth, counts);
            }
        }
    }
}

fn count_call_node(
    call: &CallNode,
    depth: usize,
    max_depth: usize,
    counts: &mut HashMap<NodeKey, usize>,
) {
    if is_low_signal_human_call(call) {
        for input in &call.inputs {
            count_flow_node(input, depth, max_depth, counts);
        }
        for child in &call.children {
            count_flow_node(child, depth, max_depth, counts);
        }
        return;
    }

    if depth > max_depth {
        return;
    }

    if call.control_kind.is_none() && should_reuse_call_node(call) {
        let label = call_label(call);
        count_reusable_key(counts, &label, call_shape(call));
    }

    for input in &call.inputs {
        count_flow_node(input, depth, max_depth, counts);
    }

    if depth >= max_depth {
        return;
    }

    for child in &call.children {
        count_flow_node(child, depth + 1, max_depth, counts);
    }
}

fn count_reusable_key(counts: &mut HashMap<NodeKey, usize>, label: &str, shape: Shape) {
    *counts
        .entry(NodeKey {
            label: label.to_string(),
            shape,
        })
        .or_insert(0) += 1;
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

    let label = call_label(node);
    let shape = call_shape(node);
    let reuse_node = should_reuse_call_node(node);
    let node_id = if node.control_kind.is_some() || !reuse_node {
        state.next_node_id()
    } else {
        reusable_node_id(state, &label, shape)
    };

    let render_subgraph = should_render_subgraph(node, depth, max_depth);
    let render_exception_subgraph = should_render_exception_subgraph(node, depth, max_depth);
    let render_shared_subgraph = render_subgraph && is_shared_reusable_node(state, &label, shape);
    if !render_subgraph && !render_exception_subgraph {
        if node.control_kind.is_some() || !reuse_node {
            write_unique_node(out, state, &node_id, &label, shape);
        } else {
            declare_reusable_node(out, state, &node_id, &label, shape);
        }
    }

    if render_exception_subgraph {
        write_exception_entry_edges(out, state, entry_ids, &node_id, edge_label);
        render_exception_subgraph_once(state, &node_id, &label, shape, node, depth, max_depth);
        return Vec::new();
    }

    if render_shared_subgraph {
        render_shared_call_subgraph_once(state, &node_id, &label, shape, node, depth, max_depth);
    }

    let direction = if state.indent > 1
        && edge_label.is_none()
        && node.inputs.is_empty()
        && node.confidence == Confidence::External
    {
        EdgeDirection::Bidirectional
    } else {
        EdgeDirection::Forward
    };
    write_edges(out, state, entry_ids, &node_id, edge_label, direction);

    for input in &node.inputs {
        render_flow_node(
            out,
            state,
            std::slice::from_ref(&node_id),
            Some("input"),
            input,
            depth,
            max_depth,
        );
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

    if node.children.is_empty() || render_shared_subgraph {
        vec![node_id]
    } else if render_subgraph {
        write_subgraph_start(out, state, &subgraph_title(node));
        if node.control_kind.is_some() || !reuse_node {
            write_unique_node(out, state, &node_id, &label, shape);
        } else {
            declare_reusable_node(out, state, &node_id, &label, shape);
        }
        render_sequence(
            out,
            state,
            std::slice::from_ref(&node_id),
            None,
            &node.children,
            depth + 1,
            max_depth,
        );
        write_subgraph_end(out, state);
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
    write_unique_node(out, state, &node_id, &lambda_label(lambda), Shape::Process);
    write_edges(
        out,
        state,
        entry_ids,
        &node_id,
        edge_label,
        EdgeDirection::Forward,
    );

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
    if branch.kind == BranchKind::Optional {
        return render_optional_branch_node(
            out,
            state,
            entry_ids,
            edge_label,
            branch,
            arm_call_depth,
            max_depth,
        );
    }

    let node_id = state.next_node_id();
    write_unique_node(out, state, &node_id, &branch_label(branch), Shape::Decision);
    write_edges(
        out,
        state,
        entry_ids,
        &node_id,
        edge_label,
        EdgeDirection::Forward,
    );
    if !branch.condition.is_empty() {
        render_dependency_nodes(
            out,
            state,
            &node_id,
            "condition",
            &branch.condition,
            arm_call_depth,
            max_depth,
        );
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

fn render_optional_branch_node(
    out: &mut String,
    state: &mut RenderState,
    entry_ids: &[String],
    edge_label: Option<&str>,
    branch: &BranchNode,
    arm_call_depth: usize,
    max_depth: usize,
) -> Vec<String> {
    let mut exits = Vec::new();
    let mut branch_entries = entry_ids.to_vec();
    if !branch.condition.is_empty() {
        branch_entries = render_sequence(
            out,
            state,
            entry_ids,
            edge_label,
            &branch.condition,
            arm_call_depth,
            max_depth,
        );
    }

    for arm in &branch.arms {
        let label = if arm.label == "empty" {
            Some("empty")
        } else {
            None
        };
        if arm.children.is_empty() {
            if !arm.terminates {
                for entry in &branch_entries {
                    push_exit(&mut exits, entry.clone());
                }
            }
            continue;
        }

        let arm_exits = render_sequence(
            out,
            state,
            &branch_entries,
            label,
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
    write_unique_node(out, state, &node_id, &loop_label(loop_node), Shape::Loop);
    write_edges(
        out,
        state,
        entry_ids,
        &node_id,
        edge_label,
        EdgeDirection::Forward,
    );

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
        write_edges(
            out,
            state,
            &update_exits,
            &node_id,
            Some("next"),
            EdgeDirection::Forward,
        );
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
    write_unique_node(
        out,
        state,
        &node_id,
        &truncated_marker(remaining_levels),
        Shape::Process,
    );
    write_edges(
        out,
        state,
        entry_ids,
        &node_id,
        edge_label,
        EdgeDirection::Forward,
    );
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

fn should_reuse_call_node(node: &CallNode) -> bool {
    node.control_kind.is_none()
        && (!node.children.is_empty()
            || is_exception_constructor(&node.method_fqn.0)
            || node.confidence == Confidence::External)
}

fn should_render_subgraph(node: &CallNode, depth: usize, max_depth: usize) -> bool {
    !node.children.is_empty()
        && depth > 1
        && depth < max_depth
        && node.control_kind.is_none()
        && matches!(
            node.confidence,
            Confidence::Resolved
                | Confidence::SingleImpl
                | Confidence::Primary
                | Confidence::Qualifier
        )
        && !node.method_fqn.0.contains("#<init>(")
}

fn should_render_exception_subgraph(node: &CallNode, depth: usize, max_depth: usize) -> bool {
    !node.children.is_empty()
        && depth < max_depth
        && node.control_kind.is_none()
        && is_exception_constructor(&node.method_fqn.0)
}

fn is_exception_constructor(fqn: &str) -> bool {
    let Some((class, method)) = fqn.rsplit_once('#') else {
        return false;
    };
    method.starts_with("<init>(")
        && class
            .rsplit('.')
            .next()
            .is_some_and(|simple| simple.ends_with("Exception"))
}

fn render_exception_subgraph_once(
    state: &mut RenderState,
    node_id: &str,
    label: &str,
    shape: Shape,
    node: &CallNode,
    depth: usize,
    max_depth: usize,
) {
    if !state
        .rendered_exception_subgraphs
        .insert(node_id.to_string())
    {
        return;
    }

    let previous_indent = state.indent;
    let previous_force_local = state.force_local_declarations;
    let previous_exception_nodes = std::mem::take(&mut state.current_exception_nodes);
    state.indent = 1;
    state.force_local_declarations = true;

    let mut subgraph = String::new();
    write_subgraph_start(
        &mut subgraph,
        state,
        &exception_subgraph_title(&node.method_fqn.0),
    );
    declare_reusable_node(&mut subgraph, state, node_id, label, shape);
    let exits = render_sequence(
        &mut subgraph,
        state,
        &[node_id.to_string()],
        None,
        &node.children,
        depth + 1,
        max_depth,
    );
    render_terminal_nodes(&mut subgraph, state, &exits, Some("terminates"));
    write_subgraph_end(&mut subgraph, state);
    if previous_indent > 1 {
        state.deferred_global_declarations.push_str(&subgraph);
    } else {
        state.global_declarations.push_str(&subgraph);
    }

    for node in std::mem::take(&mut state.current_exception_nodes) {
        if !state.exception_style_nodes.contains(&node) {
            state.exception_style_nodes.push(node);
        }
    }
    state.current_exception_nodes = previous_exception_nodes;
    state.force_local_declarations = previous_force_local;
    state.indent = previous_indent;
}

fn render_shared_call_subgraph_once(
    state: &mut RenderState,
    node_id: &str,
    label: &str,
    shape: Shape,
    node: &CallNode,
    depth: usize,
    max_depth: usize,
) {
    if !state.rendered_call_subgraphs.insert(node_id.to_string()) {
        return;
    }

    let previous_indent = state.indent;
    let previous_force_local = state.force_local_declarations;
    let previous_exception_nodes = std::mem::take(&mut state.current_exception_nodes);
    state.indent = 1;
    state.force_local_declarations = true;

    let mut subgraph = String::new();
    write_subgraph_start(&mut subgraph, state, &subgraph_title(node));
    declare_reusable_node(&mut subgraph, state, node_id, label, shape);
    render_sequence(
        &mut subgraph,
        state,
        &[node_id.to_string()],
        None,
        &node.children,
        depth + 1,
        max_depth,
    );
    write_subgraph_end(&mut subgraph, state);
    state.global_subgraph_declarations.push_str(&subgraph);

    state.force_local_declarations = previous_force_local;
    state.current_exception_nodes = previous_exception_nodes;
    state.indent = previous_indent;
}

fn write_exception_entry_edges(
    out: &mut String,
    state: &mut RenderState,
    entry_ids: &[String],
    node_id: &str,
    edge_label: Option<&str>,
) {
    if state.indent <= 1 {
        write_edges(
            out,
            state,
            entry_ids,
            node_id,
            edge_label,
            EdgeDirection::Forward,
        );
        return;
    }

    let previous_indent = state.indent;
    let mut edges = String::new();
    state.indent = 1;
    write_edges(
        &mut edges,
        state,
        entry_ids,
        node_id,
        edge_label,
        EdgeDirection::Forward,
    );
    state.indent = previous_indent;
    state.deferred_global_declarations.push_str(&edges);
}

fn exception_subgraph_title(fqn: &str) -> String {
    fqn.rsplit_once('#')
        .and_then(|(class, _)| class.rsplit('.').next())
        .unwrap_or("Exception")
        .to_string()
}

fn subgraph_title(node: &CallNode) -> String {
    let short = short_method(&node.method_fqn.0);
    short
        .split('#')
        .nth(1)
        .and_then(|method| method.split('(').next())
        .filter(|method| !method.is_empty())
        .unwrap_or(short.as_str())
        .to_string()
}

fn write_subgraph_start(out: &mut String, state: &mut RenderState, title: &str) {
    let id = format!("sg{}", state.next_subgraph);
    state.next_subgraph += 1;
    let padding = "    ".repeat(state.indent);
    writeln!(
        out,
        "{padding}subgraph {id}[\"{}\"]",
        escape_mermaid_text(title)
    )
    .unwrap();
    state.subgraph_stack.push(id);
    state.indent += 1;
    let padding = "    ".repeat(state.indent);
    writeln!(out, "{padding}direction LR").unwrap();
}

fn write_subgraph_end(out: &mut String, state: &mut RenderState) {
    state.indent -= 1;
    let padding = "    ".repeat(state.indent);
    writeln!(out, "{padding}end").unwrap();
    state.subgraph_stack.pop();
    if state.indent == 1 && !state.deferred_global_declarations.is_empty() {
        out.push_str(&state.deferred_global_declarations);
        state.deferred_global_declarations.clear();
    }
    if state.indent == 1 && !state.global_subgraph_declarations.is_empty() {
        out.push_str(&state.global_subgraph_declarations);
        state.global_subgraph_declarations.clear();
    }
}

fn is_shared_reusable_node(state: &RenderState, label: &str, shape: Shape) -> bool {
    let key = NodeKey {
        label: label.to_string(),
        shape,
    };
    state.reusable_counts.get(&key).copied().unwrap_or(0) > 1
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
        write_unique_node(out, state, &terminal_id, "terminates", Shape::Terminator);
        if state.force_local_declarations
            && !state
                .current_exception_nodes
                .iter()
                .any(|node| node == &terminal_id)
        {
            state.current_exception_nodes.push(terminal_id.clone());
        }
        write_edge(
            out,
            state,
            entry_id,
            &terminal_id,
            edge_label,
            EdgeDirection::Forward,
        );
    }
}

fn push_exit(exits: &mut Vec<String>, exit: String) {
    if !exits.contains(&exit) {
        exits.push(exit);
    }
}

fn reusable_node_id(state: &mut RenderState, label: &str, shape: Shape) -> String {
    let key = NodeKey {
        label: label.to_string(),
        shape,
    };
    if let Some(id) = state.reusable_nodes.get(&key) {
        return id.clone();
    }

    let node_id = state.next_node_id();
    state.reusable_nodes.insert(key, node_id.clone());
    node_id
}

fn declare_reusable_node(
    out: &mut String,
    state: &mut RenderState,
    id: &str,
    label: &str,
    shape: Shape,
) {
    if !state.declared_nodes.insert(id.to_string()) {
        return;
    }

    if state.force_local_declarations {
        write_node_at_indent(out, state.indent, id, label, shape);
        record_node_owner(state, id);
        if !state.current_exception_nodes.iter().any(|node| node == id) {
            state.current_exception_nodes.push(id.to_string());
        }
    } else if is_shared_reusable_node(state, label, shape) {
        write_node_at_indent(&mut state.global_node_declarations, 1, id, label, shape);
    } else {
        write_node_at_indent(out, state.indent, id, label, shape);
        record_node_owner(state, id);
    }
}

fn write_unique_node(
    out: &mut String,
    state: &mut RenderState,
    id: &str,
    label: &str,
    shape: Shape,
) {
    write_node_at_indent(out, state.indent, id, label, shape);
    record_node_owner(state, id);
    if state.force_local_declarations
        && !state.current_exception_nodes.iter().any(|node| node == id)
    {
        state.current_exception_nodes.push(id.to_string());
    }
}

fn record_node_owner(state: &mut RenderState, id: &str) {
    if let Some(subgraph_id) = state.subgraph_stack.last() {
        state
            .node_subgraphs
            .insert(id.to_string(), subgraph_id.clone());
    }
}

fn write_node_at_indent(out: &mut String, indent: usize, id: &str, label: &str, shape: Shape) {
    let label = escape_mermaid_text(label);
    let padding = "    ".repeat(indent);
    match shape {
        Shape::Process => writeln!(out, "{padding}{id}[\"{label}\"]").unwrap(),
        Shape::Subroutine => writeln!(out, "{padding}{id}[[\"{label}\"]]").unwrap(),
        Shape::External => writeln!(out, "{padding}{id}([\"{label}\"])").unwrap(),
        Shape::Control | Shape::Loop => writeln!(out, "{padding}{id}{{{{\"{label}\"}}}}").unwrap(),
        Shape::Decision => writeln!(out, "{padding}{id}{{\"{label}\"}}").unwrap(),
        Shape::Terminator => writeln!(out, "{padding}{id}([\"{label}\"])").unwrap(),
    }
}

fn write_edges(
    out: &mut String,
    state: &mut RenderState,
    from_ids: &[String],
    to: &str,
    label: Option<&str>,
    direction: EdgeDirection,
) {
    for from in from_ids {
        write_edge(out, state, from, to, label, direction);
    }
}

fn write_edge(
    out: &mut String,
    state: &mut RenderState,
    from: &str,
    to: &str,
    label: Option<&str>,
    direction: EdgeDirection,
) {
    if from == to {
        return;
    }

    let key = EdgeKey {
        from: from.to_string(),
        to: to.to_string(),
        label: label.map(str::to_string),
        direction,
    };
    if !state.rendered_edges.insert(key) {
        return;
    }

    if should_defer_cross_subgraph_edge(state, from, to) {
        write_edge_at_indent(
            &mut state.deferred_global_declarations,
            1,
            from,
            to,
            label,
            direction,
        );
        return;
    }

    write_edge_at_indent(out, state.indent, from, to, label, direction);
}

fn should_defer_cross_subgraph_edge(state: &RenderState, from: &str, to: &str) -> bool {
    if state.indent <= 1 {
        return false;
    }

    let Some(current_subgraph) = state.subgraph_stack.last() else {
        return false;
    };

    let from_subgraph = state.node_subgraphs.get(from);
    let to_subgraph = state.node_subgraphs.get(to);
    from_subgraph.is_some_and(|owner| owner != current_subgraph)
        || to_subgraph.is_some_and(|owner| owner != current_subgraph)
}

fn write_edge_at_indent(
    out: &mut String,
    indent: usize,
    from: &str,
    to: &str,
    label: Option<&str>,
    direction: EdgeDirection,
) {
    let arrow = match direction {
        EdgeDirection::Forward => "-->",
        EdgeDirection::Bidirectional => "<-->",
    };
    let padding = "    ".repeat(indent);
    if let Some(label) = label {
        writeln!(
            out,
            "{padding}{from} {arrow}|\"{}\"| {to}",
            escape_mermaid_text(label)
        )
        .unwrap();
    } else {
        writeln!(out, "{padding}{from} {arrow} {to}").unwrap();
    }
}

fn escape_mermaid_text(value: &str) -> String {
    value
        .replace('\n', " ")
        .replace('"', "'")
        .replace('[', "(")
        .replace(']', ")")
}

struct RenderState {
    next_node: usize,
    next_subgraph: usize,
    indent: usize,
    truncated: bool,
    global_declarations: String,
    global_subgraph_declarations: String,
    global_node_declarations: String,
    deferred_global_declarations: String,
    reusable_counts: HashMap<NodeKey, usize>,
    reusable_nodes: HashMap<NodeKey, String>,
    declared_nodes: HashSet<String>,
    rendered_edges: HashSet<EdgeKey>,
    subgraph_stack: Vec<String>,
    node_subgraphs: HashMap<String, String>,
    rendered_call_subgraphs: HashSet<String>,
    rendered_exception_subgraphs: HashSet<String>,
    force_local_declarations: bool,
    current_exception_nodes: Vec<String>,
    exception_style_nodes: Vec<String>,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            next_node: 0,
            next_subgraph: 0,
            indent: 1,
            truncated: false,
            global_declarations: String::new(),
            global_subgraph_declarations: String::new(),
            global_node_declarations: String::new(),
            deferred_global_declarations: String::new(),
            reusable_counts: HashMap::new(),
            reusable_nodes: HashMap::new(),
            declared_nodes: HashSet::new(),
            rendered_edges: HashSet::new(),
            subgraph_stack: Vec::new(),
            node_subgraphs: HashMap::new(),
            rendered_call_subgraphs: HashSet::new(),
            rendered_exception_subgraphs: HashSet::new(),
            force_local_declarations: false,
            current_exception_nodes: Vec::new(),
            exception_style_nodes: Vec::new(),
        }
    }
}

impl RenderState {
    fn next_node_id(&mut self) -> String {
        let id = format!("n{}", self.next_node);
        self.next_node += 1;
        id
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct NodeKey {
    label: String,
    shape: Shape,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct EdgeKey {
    from: String,
    to: String,
    label: Option<String>,
    direction: EdgeDirection,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum EdgeDirection {
    Forward,
    Bidirectional,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
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
        Arm, BranchKind, BranchNode, CallNode, Confidence, ControlKind, Endpoint, ExternalKind,
        Flow, FlowNode, Fqn, HttpVerb, LambdaKind, LambdaNode, ParamInfo, ParamSource,
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

    #[test]
    fn repeated_call_nodes_reuse_the_same_flowchart_node() {
        let exception = || {
            FlowNode::Call(node(
                "example.BusinessException#<init>()",
                vec![FlowNode::Call(node(
                    "example.ErrorCode#getDefaultMessage()",
                    Vec::new(),
                ))],
            ))
        };
        let flow = flow_with_children(vec![FlowNode::Branch(BranchNode {
            kind: BranchKind::If,
            condition_src: "invalid".to_string(),
            condition: Vec::new(),
            arms: vec![
                Arm {
                    label: "then".to_string(),
                    terminates: true,
                    children: vec![exception()],
                },
                Arm {
                    label: "else".to_string(),
                    terminates: true,
                    children: vec![exception()],
                },
            ],
        })]);

        let rendered = render(&flow, None);

        assert_eq!(
            rendered
                .matches("[[\"BusinessException#<init>()\"]]")
                .count(),
            1
        );
        assert_eq!(
            rendered
                .matches("[[\"ErrorCode#getDefaultMessage()\"]]")
                .count(),
            1
        );
        assert!(rendered.contains("    n1 -->|\"then terminates\"| n2"));
        assert!(rendered.contains("    n1 -->|\"else terminates\"| n2"));
        assert_eq!(rendered.matches("    n2 --> n3").count(), 1);
    }

    #[test]
    fn optional_branches_render_without_synthetic_optional_node() {
        let flow = flow_with_children(vec![FlowNode::Call(control_node(
            "java.util.Optional#orElseThrow(java.util.function.Supplier)",
            vec![FlowNode::Branch(BranchNode {
                kind: BranchKind::Optional,
                condition_src: String::new(),
                condition: Vec::new(),
                arms: vec![
                    Arm {
                        label: "present".to_string(),
                        terminates: false,
                        children: Vec::new(),
                    },
                    Arm {
                        label: "empty".to_string(),
                        terminates: true,
                        children: vec![FlowNode::Lambda(LambdaNode {
                            kind: LambdaKind::Lambda,
                            source: "() -> fail()".to_string(),
                            children: Vec::new(),
                        })],
                    },
                ],
            })],
        ))]);

        let rendered = render(&flow, None);

        assert!(!rendered.contains("{\"optional\"}"));
        assert!(rendered.contains("    n1 -->|\"empty\"| n2"));
        assert!(rendered.contains("lambda: () -> fail()"));
    }

    #[test]
    fn exception_constructor_children_render_as_styled_subgraph() {
        let flow = flow_with_children(vec![FlowNode::Branch(BranchNode {
            kind: BranchKind::If,
            condition_src: "invalid".to_string(),
            condition: Vec::new(),
            arms: vec![Arm {
                label: "then".to_string(),
                terminates: true,
                children: vec![FlowNode::Call(node(
                    "example.BusinessException#<init>()",
                    vec![FlowNode::Call(node(
                        "example.ErrorCode#getDefaultMessage()",
                        Vec::new(),
                    ))],
                ))],
            }],
        })]);

        let rendered = render(&flow, None);

        assert!(rendered.contains("    subgraph sg0[\"BusinessException\"]"));
        assert!(rendered.contains("        n2[[\"BusinessException#<init>()\"]]"));
        assert!(rendered.contains("        n2 --> n3"));
        assert!(rendered.contains("        n3 -->|\"terminates\"| n4"));
        assert!(
            rendered
                .contains("    classDef exceptionFlow fill:#fde2e2,stroke:#dc2626,color:#7f1d1d")
        );
        assert!(rendered.contains("    class n2,n3,n4 exceptionFlow"));
    }

    #[test]
    fn nested_resolved_calls_render_as_subgraphs() {
        let flow = flow_with_children(vec![FlowNode::Call(node(
            "example.Service#outer()",
            vec![FlowNode::Call(node(
                "example.Service#inner()",
                vec![FlowNode::Call(external_node("example.Repository#find()"))],
            ))],
        ))]);

        let rendered = render(&flow, None);

        assert!(rendered.contains("    n1 --> n2"));
        assert!(rendered.contains("    subgraph sg0[\"inner\"]"));
        assert!(rendered.contains("        n2[[\"Service#inner()\"]]"));
        assert!(rendered.contains("        n2 <--> n3"));
        assert!(rendered.contains("    end"));
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

    fn external_node(fqn: &str) -> CallNode {
        CallNode {
            confidence: Confidence::External,
            ..node(fqn, Vec::new())
        }
    }

    fn control_node(fqn: &str, children: Vec<FlowNode>) -> CallNode {
        CallNode {
            control_kind: Some(ControlKind::Optional),
            children,
            ..node(fqn, Vec::new())
        }
    }
}
