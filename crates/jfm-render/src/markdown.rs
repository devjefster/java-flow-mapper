//! Markdown renderer for human-readable endpoint flow summaries.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;

use crate::model::{
    BranchKind, BranchNode, CallNode, Confidence, Flow, FlowNode, Fqn, LambdaKind, LambdaNode,
    LoopNode, ParamSource,
};

use super::common::{
    control_kind_human_label, external_kind_human_label, is_low_signal_human_call,
    loop_execution_label, loop_kind_human_label, max_remaining_depth, short_method, single_line,
    truncated_marker, truncated_note,
};

const DEFAULT_MAX_DEPTH: usize = 5;

/// Render a flow as Markdown.
pub fn render(flow: &Flow, max_depth: Option<usize>) -> String {
    let effective_max_depth = max_depth.unwrap_or(DEFAULT_MAX_DEPTH);
    let mut state = RenderState::default();
    let mut tree = String::new();
    render_call_node(
        &mut tree,
        &flow.root,
        0,
        0,
        true,
        effective_max_depth,
        &mut state,
    );

    let mut out = String::new();
    writeln!(out, "# {} {}", flow.endpoint.verb, flow.endpoint.path).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "**Controller**: `{}`", flow.endpoint.handler_fqn).unwrap();
    writeln!(
        out,
        "**File**: `{}:{}`",
        flow.endpoint.file.display(),
        flow.endpoint.line
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Call sequence").unwrap();
    writeln!(out).unwrap();
    out.push_str(&tree);
    writeln!(out).unwrap();
    writeln!(out, "## Inputs").unwrap();
    writeln!(out).unwrap();
    if flow.inputs.is_empty() {
        writeln!(out, "- None").unwrap();
    } else {
        for input in &flow.inputs {
            writeln!(
                out,
                "- `{}: {}` *({})*",
                input.name,
                input.ty,
                param_source_label(&input.source)
            )
            .unwrap();
        }
    }
    writeln!(out).unwrap();
    writeln!(out, "## Unresolved / external").unwrap();
    writeln!(out).unwrap();
    if state.external.is_empty() && state.unresolved.is_empty() {
        writeln!(out, "- None").unwrap();
    } else {
        for (method, label) in state.external {
            writeln!(out, "- `{}` - `external` ({})", method, label).unwrap();
        }
        for (method, reason) in state.unresolved {
            writeln!(out, "- `{}` - `unresolved` ({})", method, reason).unwrap();
        }
    }
    writeln!(out).unwrap();
    writeln!(out, "## Notes").unwrap();
    writeln!(out).unwrap();
    for note in &flow.notes {
        writeln!(out, "- {note}").unwrap();
    }
    if state.truncated {
        writeln!(out, "- {}", truncated_note(effective_max_depth)).unwrap();
    }
    if state.elided {
        writeln!(
            out,
            "- Repeated subtrees elided after first appearance (see above)."
        )
        .unwrap();
    }

    out
}

fn render_call_node(
    out: &mut String,
    node: &CallNode,
    indent_depth: usize,
    call_depth: usize,
    root: bool,
    max_depth: usize,
    state: &mut RenderState,
) {
    let indent = "  ".repeat(indent_depth);
    if !root && is_low_signal_human_call(node) {
        for input in &node.inputs {
            render_flow_node(out, input, indent_depth, call_depth, max_depth, state);
        }
        for child in &node.children {
            render_flow_node(out, child, indent_depth, call_depth, max_depth, state);
        }
        return;
    }

    write!(out, "{}- `{}`", indent, short_method(&node.method_fqn.0)).unwrap();
    if !root && !node.children.is_empty() && state.expanded_with_children.contains(&node.method_fqn)
    {
        writeln!(out, " *(see above)*").unwrap();
        state.elided = true;
        return;
    }

    if !root && let Some(label) = confidence_label(node) {
        write!(out, " *[{}]*", label).unwrap();
    }
    if let Some(note) = &node.note {
        write!(out, " - {note}").unwrap();
    }
    writeln!(out).unwrap();

    collect_rendered_reference(node, state);

    render_relation_section(
        out,
        "inputs",
        &node.inputs,
        indent_depth,
        call_depth,
        max_depth,
        state,
    );

    if !node.children.is_empty() && call_depth >= max_depth {
        state.truncated = true;
        if !(root && max_depth == 0) {
            writeln!(
                out,
                "{}  ... {}",
                indent,
                truncated_marker(max_remaining_depth(node))
            )
            .unwrap();
        }
        return;
    }

    if !node.children.is_empty() {
        state.expanded_with_children.insert(node.method_fqn.clone());
    }

    for child in &node.children {
        render_flow_node(
            out,
            child,
            indent_depth + 1,
            call_depth + 1,
            max_depth,
            state,
        );
    }
}

fn render_flow_node(
    out: &mut String,
    node: &FlowNode,
    indent_depth: usize,
    call_depth: usize,
    max_depth: usize,
    state: &mut RenderState,
) {
    match node {
        FlowNode::Call(call) => {
            render_call_node(out, call, indent_depth, call_depth, false, max_depth, state);
        }
        FlowNode::Lambda(lambda) => {
            render_lambda(out, lambda, indent_depth, call_depth, max_depth, state);
        }
        FlowNode::Branch(branch) => {
            render_branch(out, branch, indent_depth, call_depth, max_depth, state);
        }
        FlowNode::Loop(loop_node) => {
            render_loop(out, loop_node, indent_depth, call_depth, max_depth, state);
        }
    }
}

fn render_lambda(
    out: &mut String,
    lambda: &LambdaNode,
    indent_depth: usize,
    call_depth: usize,
    max_depth: usize,
    state: &mut RenderState,
) {
    let indent = "  ".repeat(indent_depth);
    let label = match lambda.kind {
        LambdaKind::Lambda => "lambda",
        LambdaKind::MethodRef => "method ref",
    };
    writeln!(out, "{indent}- {label} `{}`:", single_line(&lambda.source)).unwrap();
    for child in &lambda.children {
        render_flow_node(out, child, indent_depth + 1, call_depth, max_depth, state);
    }
}

fn render_branch(
    out: &mut String,
    branch: &BranchNode,
    indent_depth: usize,
    arm_call_depth: usize,
    max_depth: usize,
    state: &mut RenderState,
) {
    let indent = "  ".repeat(indent_depth);
    let mut rendered_condition = false;
    for arm in &branch.arms {
        let header = match (branch.kind, arm.label.as_str()) {
            (BranchKind::If, "then") => format!("if {}", branch.condition_src),
            (BranchKind::If, "else") => "else".to_string(),
            (BranchKind::Switch, "default") => "default".to_string(),
            (BranchKind::Switch, label) => format!("case {label}"),
            (BranchKind::Ternary, "then") => format!("ternary {}", branch.condition_src),
            (BranchKind::Ternary, "else") => "else".to_string(),
            (BranchKind::TryCatch, label) => label.to_string(),
            (BranchKind::Optional, label) => format!("optional {label}"),
            (BranchKind::If | BranchKind::Ternary, label) => label.to_string(),
        };
        write!(out, "{indent}- {header}:").unwrap();
        if arm.terminates {
            write!(out, " *(terminates)*").unwrap();
        }
        writeln!(out).unwrap();

        if !rendered_condition {
            render_relation_section(
                out,
                "condition",
                &branch.condition,
                indent_depth,
                arm_call_depth,
                max_depth,
                state,
            );
            rendered_condition = true;
        }

        for child in &arm.children {
            render_flow_node(
                out,
                child,
                indent_depth + 1,
                arm_call_depth,
                max_depth,
                state,
            );
        }
    }
}

fn render_relation_section(
    out: &mut String,
    label: &str,
    children: &[FlowNode],
    indent_depth: usize,
    call_depth: usize,
    max_depth: usize,
    state: &mut RenderState,
) {
    if children.is_empty() {
        return;
    }

    let indent = "  ".repeat(indent_depth + 1);
    writeln!(out, "{indent}- {label}:").unwrap();
    for child in children {
        render_flow_node(out, child, indent_depth + 2, call_depth, max_depth, state);
    }
}

fn render_loop(
    out: &mut String,
    loop_node: &LoopNode,
    indent_depth: usize,
    section_call_depth: usize,
    max_depth: usize,
    state: &mut RenderState,
) {
    let indent = "  ".repeat(indent_depth);
    writeln!(
        out,
        "{indent}- loop {} `{}`: *(may execute {} times)*",
        loop_kind_human_label(loop_node.kind),
        single_line(&loop_node.source),
        loop_execution_label(loop_node.execution)
    )
    .unwrap();

    render_loop_section(
        out,
        "init",
        &loop_node.init,
        indent_depth,
        section_call_depth,
        max_depth,
        state,
    );
    render_loop_section(
        out,
        "condition",
        &loop_node.condition,
        indent_depth,
        section_call_depth,
        max_depth,
        state,
    );
    for arm in &loop_node.arms {
        render_loop_section(
            out,
            &arm.label,
            &arm.children,
            indent_depth,
            section_call_depth,
            max_depth,
            state,
        );
    }
    render_loop_section(
        out,
        "update",
        &loop_node.update,
        indent_depth,
        section_call_depth,
        max_depth,
        state,
    );
}

fn render_loop_section(
    out: &mut String,
    label: &str,
    children: &[FlowNode],
    indent_depth: usize,
    section_call_depth: usize,
    max_depth: usize,
    state: &mut RenderState,
) {
    if children.is_empty() {
        return;
    }

    let indent = "  ".repeat(indent_depth + 1);
    writeln!(out, "{indent}- {label}:").unwrap();
    for child in children {
        render_flow_node(
            out,
            child,
            indent_depth + 2,
            section_call_depth,
            max_depth,
            state,
        );
    }
}

fn confidence_label(node: &CallNode) -> Option<String> {
    if let Some(kind) = &node.control_kind {
        return Some(format!("control - {}", control_kind_human_label(kind)));
    }

    match node.confidence {
        Confidence::Resolved => Some("resolved".to_string()),
        Confidence::External => Some(format!(
            "external - {}",
            external_kind_human_label(node.external_kind.as_ref())
        )),
        Confidence::Unresolved => Some("unresolved".to_string()),
        Confidence::SingleImpl => Some("single impl".to_string()),
        Confidence::Primary => Some("primary".to_string()),
        Confidence::Qualifier => Some("qualifier".to_string()),
        Confidence::Ambiguous => Some("ambiguous".to_string()),
    }
}

fn collect_rendered_reference(node: &CallNode, state: &mut RenderState) {
    if node.control_kind.is_some() {
        return;
    }

    match node.confidence {
        Confidence::External => {
            if is_low_signal_human_call(node) {
                return;
            }
            state.external.insert(
                short_method(&node.method_fqn.0),
                external_kind_human_label(node.external_kind.as_ref()).to_string(),
            );
        }
        Confidence::Unresolved => {
            state.unresolved.push((
                short_method(&node.method_fqn.0),
                node.note.as_deref().unwrap_or("unresolved").to_string(),
            ));
        }
        _ => {}
    }
}

#[derive(Default)]
struct RenderState {
    // Populated during rendering so truncated and elided children do not leak
    // into the flat roundup.
    external: BTreeMap<String, String>,
    unresolved: Vec<(String, String)>,
    expanded_with_children: HashSet<Fqn>,
    truncated: bool,
    elided: bool,
}

fn param_source_label(source: &ParamSource) -> &'static str {
    match source {
        ParamSource::Path => "path variable",
        ParamSource::Query => "query parameter",
        ParamSource::Body => "request body",
        ParamSource::Header => "request header",
        ParamSource::Unspecified => "unspecified",
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::model::{
        CallNode, Confidence, Endpoint, ExternalKind, Flow, FlowNode, Fqn, HttpVerb, ParamInfo,
        ParamSource,
    };

    use super::render;

    #[test]
    fn elides_repeated_subtree_siblings_after_first_expansion() {
        let repeated = node(
            "example.Repeated#helper()",
            Confidence::Resolved,
            None,
            None,
            vec![FlowNode::Call(node(
                "example.Leaf#work()",
                Confidence::Resolved,
                None,
                None,
                Vec::new(),
            ))],
        );
        let flow = flow_with_children(vec![
            FlowNode::Call(repeated.clone()),
            FlowNode::Call(repeated),
        ]);

        let rendered = render(&flow, None);

        assert!(
            rendered
                .contains("- `Repeated#helper()` *[resolved]*\n    - `Leaf#work()` *[resolved]*")
        );
        assert!(rendered.contains("- `Repeated#helper()` *(see above)*"));
        assert!(rendered.contains("Repeated subtrees elided after first appearance"));
    }

    #[test]
    fn elides_repeated_subtree_at_different_depths() {
        let repeated = node(
            "example.Repeated#helper()",
            Confidence::Resolved,
            None,
            None,
            vec![FlowNode::Call(node(
                "example.Leaf#work()",
                Confidence::Resolved,
                None,
                None,
                Vec::new(),
            ))],
        );
        let wrapper = node(
            "example.Wrapper#call()",
            Confidence::Resolved,
            None,
            None,
            vec![FlowNode::Call(repeated.clone())],
        );
        let flow = flow_with_children(vec![FlowNode::Call(wrapper), FlowNode::Call(repeated)]);

        let rendered = render(&flow, None);

        assert!(rendered.contains(
            "    - `Repeated#helper()` *[resolved]*\n      - `Leaf#work()` *[resolved]*"
        ));
        assert!(rendered.contains("- `Repeated#helper()` *(see above)*"));
    }

    #[test]
    fn does_not_elide_repeated_external_or_unresolved_leaves() {
        let external = node(
            "Optional#orElseThrow(Supplier)",
            Confidence::External,
            Some(ExternalKind::Jdk),
            None,
            Vec::new(),
        );
        let unresolved = node(
            "Unknown#missing()",
            Confidence::Unresolved,
            None,
            Some("receiver type unknown"),
            Vec::new(),
        );
        let flow = flow_with_children(vec![
            FlowNode::Call(external.clone()),
            FlowNode::Call(external),
            FlowNode::Call(unresolved.clone()),
            FlowNode::Call(unresolved),
        ]);

        let rendered = render(&flow, None);

        assert!(!rendered.contains("see above"));
        assert_eq!(
            rendered.matches("Optional#orElseThrow(Supplier)").count(),
            3
        );
        assert_eq!(rendered.matches("Unknown#missing()").count(), 4);
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
            }],
            root: node(
                "example.Controller#handle()",
                Confidence::Resolved,
                None,
                None,
                children,
            ),
            unresolved: Vec::new(),
            notes: vec!["note".to_string()],
        }
    }

    fn node(
        fqn: &str,
        confidence: Confidence,
        external_kind: Option<ExternalKind>,
        note: Option<&str>,
        children: Vec<FlowNode>,
    ) -> CallNode {
        CallNode {
            method_fqn: Fqn(fqn.to_string()),
            confidence,
            external_kind,
            control_kind: None,
            scope: None,
            note: note.map(str::to_string),
            inputs: Vec::new(),
            children,
        }
    }
}
