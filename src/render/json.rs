//! JSON renderer with DTOs kept separate from internal flow models.

use serde::Serialize;

use crate::model::{
    BranchKind, BranchNode, CallNode, Confidence, ControlKind, ExternalKind, Flow, FlowNode,
    HttpVerb, LambdaKind, LambdaNode, LoopKind, LoopNode, ParamInfo, ParamSource, Scope,
    UnresolvedRef,
};

use super::common::{max_remaining_depth, truncated_note};

/// Render a flow as pretty JSON.
pub fn render(flow: &Flow, max_depth: Option<usize>) -> String {
    let mut truncated = false;
    let call_sequence = if max_depth == Some(0) {
        truncated = !flow.root.children.is_empty();
        Vec::new()
    } else {
        flow.root
            .children
            .iter()
            .map(|node| flow_node_dto(node, 1, max_depth, &mut truncated))
            .collect()
    };
    let mut notes = flow.notes.clone();
    if truncated && let Some(max_depth) = max_depth {
        notes.push(truncated_note(max_depth));
    }

    let dto = FlowDto {
        endpoint: EndpointDto {
            verb: verb_label(&flow.endpoint.verb),
            path: &flow.endpoint.path,
        },
        controller: ControllerDto {
            method: &flow.endpoint.handler_fqn.0,
            file: flow.endpoint.file.display().to_string(),
            line: flow.endpoint.line,
        },
        inputs: flow.inputs.iter().map(InputDto::from).collect(),
        call_sequence,
        unresolved: flow.unresolved.iter().map(UnresolvedDto::from).collect(),
        notes,
    };

    let mut out = serde_json::to_string_pretty(&dto).expect("flow DTO is serializable");
    out.push('\n');
    out
}

#[derive(Serialize)]
struct FlowDto<'a> {
    endpoint: EndpointDto<'a>,
    controller: ControllerDto<'a>,
    inputs: Vec<InputDto<'a>>,
    call_sequence: Vec<FlowNodeDto<'a>>,
    unresolved: Vec<UnresolvedDto<'a>>,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct EndpointDto<'a> {
    verb: &'static str,
    path: &'a str,
}

#[derive(Serialize)]
struct ControllerDto<'a> {
    method: &'a str,
    file: String,
    line: u32,
}

#[derive(Serialize)]
struct InputDto<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    ty: &'a str,
    source: &'static str,
}

impl<'a> From<&'a ParamInfo> for InputDto<'a> {
    fn from(param: &'a ParamInfo) -> Self {
        Self {
            name: &param.name,
            ty: &param.ty,
            source: param_source_label(&param.source),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FlowNodeDto<'a> {
    Call(CallDto<'a>),
    Lambda(LambdaDto<'a>),
    Branch(BranchDto<'a>),
    Loop(LoopDto<'a>),
}

#[derive(Serialize)]
struct CallDto<'a> {
    method: &'a str,
    confidence: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncated: Option<usize>,
    calls: Vec<FlowNodeDto<'a>>,
}

#[derive(Serialize)]
struct BranchDto<'a> {
    kind: &'static str,
    condition_src: &'a str,
    arms: Vec<ArmDto<'a>>,
}

#[derive(Serialize)]
struct LambdaDto<'a> {
    kind: &'static str,
    source: &'a str,
    children: Vec<FlowNodeDto<'a>>,
}

#[derive(Serialize)]
struct LoopDto<'a> {
    kind: &'static str,
    source: &'a str,
    condition: Vec<FlowNodeDto<'a>>,
    body: Vec<FlowNodeDto<'a>>,
    update: Vec<FlowNodeDto<'a>>,
}

#[derive(Serialize)]
struct ArmDto<'a> {
    label: &'a str,
    terminates: bool,
    children: Vec<FlowNodeDto<'a>>,
}

fn flow_node_dto<'a>(
    node: &'a FlowNode,
    depth: usize,
    max_depth: Option<usize>,
    truncated: &mut bool,
) -> FlowNodeDto<'a> {
    match node {
        FlowNode::Call(call) => FlowNodeDto::Call(call_node_dto(call, depth, max_depth, truncated)),
        FlowNode::Lambda(lambda) => {
            FlowNodeDto::Lambda(lambda_node_dto(lambda, depth, max_depth, truncated))
        }
        FlowNode::Branch(branch) => {
            FlowNodeDto::Branch(branch_node_dto(branch, depth, max_depth, truncated))
        }
        FlowNode::Loop(loop_node) => {
            FlowNodeDto::Loop(loop_node_dto(loop_node, depth, max_depth, truncated))
        }
    }
}

fn lambda_node_dto<'a>(
    lambda: &'a LambdaNode,
    call_depth: usize,
    max_depth: Option<usize>,
    truncated: &mut bool,
) -> LambdaDto<'a> {
    LambdaDto {
        kind: lambda_kind_json_label(lambda.kind),
        source: &lambda.source,
        children: flow_node_dtos(&lambda.children, call_depth, max_depth, truncated),
    }
}

fn loop_node_dto<'a>(
    loop_node: &'a LoopNode,
    call_depth: usize,
    max_depth: Option<usize>,
    truncated: &mut bool,
) -> LoopDto<'a> {
    LoopDto {
        kind: loop_kind_json_label(loop_node.kind),
        source: &loop_node.source,
        condition: flow_node_dtos(&loop_node.condition, call_depth, max_depth, truncated),
        body: flow_node_dtos(&loop_node.body, call_depth, max_depth, truncated),
        update: flow_node_dtos(&loop_node.update, call_depth, max_depth, truncated),
    }
}

fn call_node_dto<'a>(
    node: &'a CallNode,
    depth: usize,
    max_depth: Option<usize>,
    truncated: &mut bool,
) -> CallDto<'a> {
    let should_truncate =
        max_depth.is_some_and(|max_depth| depth >= max_depth) && !node.children.is_empty();
    if should_truncate {
        *truncated = true;
    }

    CallDto {
        method: &node.method_fqn.0,
        confidence: if node.control_kind.is_some() {
            "control"
        } else {
            confidence_label(&node.confidence)
        },
        external_kind: if node.control_kind.is_some() {
            None
        } else {
            node.external_kind.as_ref().map(external_kind_json_label)
        },
        control_kind: node.control_kind.as_ref().map(control_kind_json_label),
        scope: node.scope.as_ref().map(scope_label),
        truncated: should_truncate.then(|| max_remaining_depth(node)),
        calls: if should_truncate {
            Vec::new()
        } else {
            flow_node_dtos(&node.children, depth + 1, max_depth, truncated)
        },
    }
}

fn branch_node_dto<'a>(
    branch: &'a BranchNode,
    arm_call_depth: usize,
    max_depth: Option<usize>,
    truncated: &mut bool,
) -> BranchDto<'a> {
    BranchDto {
        kind: branch_kind_json_label(branch.kind),
        condition_src: &branch.condition_src,
        arms: branch
            .arms
            .iter()
            .map(|arm| ArmDto {
                label: &arm.label,
                terminates: arm.terminates,
                children: flow_node_dtos(&arm.children, arm_call_depth, max_depth, truncated),
            })
            .collect(),
    }
}

fn flow_node_dtos<'a>(
    nodes: &'a [FlowNode],
    depth: usize,
    max_depth: Option<usize>,
    truncated: &mut bool,
) -> Vec<FlowNodeDto<'a>> {
    nodes
        .iter()
        .map(|node| flow_node_dto(node, depth, max_depth, truncated))
        .collect()
}

#[derive(Serialize)]
struct UnresolvedDto<'a> {
    receiver_type: &'a str,
    method_name: &'a str,
    reason: &'a str,
}

impl<'a> From<&'a UnresolvedRef> for UnresolvedDto<'a> {
    fn from(unresolved: &'a UnresolvedRef) -> Self {
        Self {
            receiver_type: &unresolved.receiver_type,
            method_name: &unresolved.method_name,
            reason: &unresolved.reason,
        }
    }
}

fn verb_label(verb: &HttpVerb) -> &'static str {
    match verb {
        HttpVerb::Get => "GET",
        HttpVerb::Post => "POST",
        HttpVerb::Put => "PUT",
        HttpVerb::Delete => "DELETE",
        HttpVerb::Patch => "PATCH",
    }
}

fn confidence_label(confidence: &Confidence) -> &'static str {
    match confidence {
        Confidence::Resolved => "resolved",
        Confidence::SingleImpl => "single_impl",
        Confidence::Primary => "primary",
        Confidence::Qualifier => "qualifier",
        Confidence::Ambiguous => "ambiguous",
        Confidence::External => "external",
        Confidence::Unresolved => "unresolved",
    }
}

fn external_kind_json_label(kind: &ExternalKind) -> &'static str {
    match kind {
        ExternalKind::Jdk => "jdk",
        ExternalKind::SpringData => "spring-data",
        ExternalKind::ThirdParty => "third-party",
        ExternalKind::Unknown => "unknown",
    }
}

fn control_kind_json_label(kind: &ControlKind) -> &'static str {
    match kind {
        ControlKind::Optional => "optional",
    }
}

fn scope_label(scope: &Scope) -> &'static str {
    match scope {
        Scope::IntraClass => "intra-class",
    }
}

fn branch_kind_json_label(kind: BranchKind) -> &'static str {
    match kind {
        BranchKind::If => "if",
        BranchKind::Switch => "switch",
        BranchKind::Optional => "optional",
    }
}

fn lambda_kind_json_label(kind: LambdaKind) -> &'static str {
    match kind {
        LambdaKind::Lambda => "lambda",
        LambdaKind::MethodRef => "method_ref",
    }
}

fn loop_kind_json_label(kind: LoopKind) -> &'static str {
    match kind {
        LoopKind::For => "for",
        LoopKind::EnhancedFor => "enhanced_for",
        LoopKind::While => "while",
        LoopKind::DoWhile => "do_while",
        LoopKind::ForEach => "for_each",
        LoopKind::Stream => "stream",
    }
}

fn param_source_label(source: &ParamSource) -> &'static str {
    match source {
        ParamSource::Path => "path",
        ParamSource::Query => "query",
        ParamSource::Body => "body",
        ParamSource::Header => "header",
        ParamSource::Unspecified => "unspecified",
    }
}
