//! Modeled external calls for common JDK, Optional, and Stream shapes.
//!
//! These tables intentionally cover the demo-oriented flow shapes; they are not
//! a general Java type inference engine.

use std::collections::HashSet;

use crate::model::{
    Arm, BranchKind, BranchNode, CallSite, ClassInfo, ExternalKind, FlowNode, Fqn, LoopArm,
    LoopExecution, LoopKind, LoopNode, MethodInfo, ProjectIndex, TypeRef, UnresolvedRef,
};

use super::expand::expand_lambdas;
use super::node::{external_node, unknown_params};

pub(super) struct ExpandContext<'a, 'b> {
    pub(super) index: &'a ProjectIndex,
    pub(super) owner: &'a ClassInfo,
    pub(super) caller: &'a MethodInfo,
    pub(super) unresolved: &'b mut Vec<UnresolvedRef>,
    pub(super) stack: &'b mut HashSet<Fqn>,
    pub(super) depth: usize,
}

pub(super) fn expand_external_call_children(
    ctx: &mut ExpandContext<'_, '_>,
    call: &CallSite,
    receiver_label: &str,
) -> Vec<FlowNode> {
    // Some JDK APIs carry lambdas that should render as control structure nodes.
    if receiver_label == "Optional"
        && let Some(branch) = optional_branch(ctx, call)
    {
        return vec![FlowNode::Branch(branch)];
    }

    if let Some(loop_node) = external_loop(ctx, call, receiver_label) {
        return vec![FlowNode::Loop(loop_node)];
    }

    expand_lambdas(
        ctx.index,
        ctx.owner,
        ctx.caller,
        &call.lambdas,
        ctx.unresolved,
        ctx.stack,
        ctx.depth,
    )
}

fn external_loop(
    ctx: &mut ExpandContext<'_, '_>,
    call: &CallSite,
    receiver_label: &str,
) -> Option<LoopNode> {
    let kind = match (receiver_label, call.method_name.as_str(), call.arity) {
        (
            "Stream",
            "map" | "flatMap" | "filter" | "peek" | "anyMatch" | "allMatch" | "noneMatch",
            1,
        ) => LoopKind::Stream,
        ("Stream", "forEach", 1) => LoopKind::ForEach,
        ("Iterable" | "List" | "Set", "forEach", 1) => LoopKind::ForEach,
        _ => return None,
    };

    Some(LoopNode {
        kind,
        source: format!("{receiver_label}.{}", call.method_name),
        execution: LoopExecution::ZeroOrMore,
        init: Vec::new(),
        condition: Vec::new(),
        arms: vec![LoopArm {
            label: "body".to_string(),
            children: lambda_child(ctx, call, 0),
        }],
        update: Vec::new(),
    })
}

fn optional_branch(ctx: &mut ExpandContext<'_, '_>, call: &CallSite) -> Option<BranchNode> {
    let arms = match (call.method_name.as_str(), call.arity) {
        ("ifPresent", 1) => optional_arms(vec![("present", lambda_child(ctx, call, 0), false)]),
        ("ifPresentOrElse", 2) => optional_arms(vec![
            ("present", lambda_child(ctx, call, 0), false),
            ("empty", lambda_child(ctx, call, 1), false),
        ]),
        ("map" | "flatMap", 1) => optional_arms(vec![
            ("present", lambda_child(ctx, call, 0), false),
            ("empty", Vec::new(), false),
        ]),
        ("filter", 1) => optional_arms(vec![
            ("present predicate", lambda_child(ctx, call, 0), false),
            ("empty or predicate false", Vec::new(), false),
        ]),
        ("or" | "orElseGet", 1) => optional_arms(vec![
            ("present", Vec::new(), false),
            ("empty", lambda_child(ctx, call, 0), false),
        ]),
        ("orElseThrow", 1) => optional_arms(vec![
            ("present", Vec::new(), false),
            ("empty", lambda_child(ctx, call, 0), true),
        ]),
        ("orElseThrow" | "get", 0) => optional_arms(vec![
            ("present", Vec::new(), false),
            ("empty", implicit_no_such_element(), true),
        ]),
        ("orElse", 1) => optional_arms(vec![
            ("present", Vec::new(), false),
            ("empty fallback", Vec::new(), false),
        ]),
        _ => return None,
    };

    Some(BranchNode {
        kind: BranchKind::Optional,
        condition_src: optional_condition(&call.method_name).to_string(),
        arms,
    })
}

fn optional_arms(arms: Vec<(&str, Vec<FlowNode>, bool)>) -> Vec<Arm> {
    arms.into_iter()
        .map(|(label, children, terminates)| Arm {
            label: label.to_string(),
            terminates,
            children,
        })
        .collect()
}

fn lambda_child(
    ctx: &mut ExpandContext<'_, '_>,
    call: &CallSite,
    lambda_index: usize,
) -> Vec<FlowNode> {
    call.lambdas
        .get(lambda_index)
        .map(|lambda| {
            expand_lambdas(
                ctx.index,
                ctx.owner,
                ctx.caller,
                std::slice::from_ref(lambda),
                ctx.unresolved,
                ctx.stack,
                ctx.depth,
            )
        })
        .unwrap_or_default()
}

fn implicit_no_such_element() -> Vec<FlowNode> {
    vec![FlowNode::Call(external_node(
        Fqn("NoSuchElementException#<init>()".to_string()),
        ExternalKind::JdkLibrary,
    ))]
}

fn optional_condition(method_name: &str) -> &str {
    match method_name {
        "filter" => "optional present and predicate matches",
        "or" | "orElse" | "orElseGet" | "orElseThrow" | "get" => "optional empty fallback",
        _ => "optional present",
    }
}

pub(super) fn external_kind_for(fqn: &str) -> ExternalKind {
    if fqn.starts_with("java.") || fqn.starts_with("javax.") || fqn.starts_with("jakarta.") {
        ExternalKind::Jdk
    } else {
        ExternalKind::ThirdParty
    }
}

pub(super) fn external_kind_for_call(
    kind: ExternalKind,
    receiver: &str,
    method_name: &str,
    arity: usize,
) -> ExternalKind {
    if kind == ExternalKind::Jdk && is_routine_jdk_library_call(receiver, method_name, arity) {
        ExternalKind::JdkLibrary
    } else {
        kind
    }
}

fn is_routine_jdk_library_call(receiver: &str, method_name: &str, arity: usize) -> bool {
    matches!(
        (receiver, method_name, arity),
        (
            "ArrayList" | "IllegalStateException" | "RuntimeException" | "NoSuchElementException",
            "<init>",
            _
        ) | ("Boolean", "equals", 1)
            | ("List" | "Set", "add", 1)
            | ("List" | "Set", "stream", 0)
            | ("String", "equalsIgnoreCase", 1)
            | ("String", "trim" | "toLowerCase", 0)
            | ("Stream", "toList", 0)
    )
}

pub(super) fn is_jdk_simple(raw: &str) -> bool {
    matches!(
        raw,
        "String"
            | "Long"
            | "Integer"
            | "Boolean"
            | "Double"
            | "Float"
            | "Short"
            | "Byte"
            | "Character"
            | "Object"
            | "Optional"
            | "Iterable"
            | "List"
            | "Stream"
            | "Set"
            | "Map"
            | "Supplier"
            | "IllegalStateException"
            | "RuntimeException"
    )
}

pub(super) fn external_params(receiver: &str, method_name: &str, arity: usize) -> String {
    if receiver == "Optional" {
        match (method_name, arity) {
            ("filter", 1) => return "Predicate".to_string(),
            ("flatMap" | "map", 1) => return "Function".to_string(),
            ("ifPresent", 1) => return "Consumer".to_string(),
            ("ifPresentOrElse", 2) => return "Consumer, Runnable".to_string(),
            ("or" | "orElseGet" | "orElseThrow", 1) => return "Supplier".to_string(),
            _ => {}
        }
    }
    if receiver == "Stream" {
        match (method_name, arity) {
            ("filter" | "anyMatch" | "allMatch" | "noneMatch", 1) => {
                return "Predicate".to_string();
            }
            ("flatMap" | "map", 1) => return "Function".to_string(),
            ("forEach" | "peek", 1) => return "Consumer".to_string(),
            _ => {}
        }
    }
    if matches!(receiver, "Iterable" | "List" | "Set") && method_name == "forEach" && arity == 1 {
        return "Consumer".to_string();
    }
    unknown_params(arity)
}

pub(super) fn jdk_return_type(receiver: &str, method_name: &str, arity: usize) -> Option<TypeRef> {
    let raw = match (receiver, method_name, arity) {
        ("Optional", "filter" | "flatMap" | "map" | "or", 1) => "Optional",
        ("Optional", "empty", 0) | ("Optional", "of" | "ofNullable", 1) => "Optional",
        ("Optional", "stream", 0) => "Stream",
        ("Optional", "get" | "orElseThrow", 0) => "Object",
        ("Optional", "orElse" | "orElseGet" | "orElseThrow", 1) => "Object",
        ("Optional", "isEmpty" | "isPresent", 0) => "Boolean",
        ("Optional", "equals", 1) => "Boolean",
        ("Optional", "hashCode", 0) => "Integer",
        ("Optional", "toString", 0) => "String",
        ("Optional", "ifPresent", 1) | ("Optional", "ifPresentOrElse", 2) => "void",
        ("List", "stream", 0) | ("Set", "stream", 0) => "Stream",
        ("Stream", "map" | "flatMap" | "filter" | "peek", 1) => "Stream",
        ("Stream", "anyMatch" | "allMatch" | "noneMatch", 1) => "Boolean",
        ("Stream", "forEach", 1) => "void",
        ("Stream", "toList", 0) => "List",
        ("Iterable" | "List" | "Set", "forEach", 1) => "void",
        ("String", "trim", 0) | ("String", "toLowerCase", 0) => "String",
        _ => return None,
    };

    Some(TypeRef {
        raw: raw.to_string(),
        generics: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;

    use crate::model::{
        BranchKind, ClassInfo, ClassKind, ExternalKind, FieldInfo, FlowNode, Fqn, LambdaKind,
        LambdaSyntax, LoopKind, MethodInfo, ParamInfo, ProjectIndex, ReceiverKind, TypeRef,
    };

    use super::*;

    #[test]
    fn optional_if_present_or_else_becomes_present_and_empty_branch() {
        let index = ProjectIndex::default();
        let owner = class();
        let caller = method();
        let call = optional_call(
            "ifPresentOrElse",
            2,
            vec![lambda("user -> send(user)"), lambda("() -> auditMissing()")],
        );
        let mut unresolved = Vec::new();
        let mut stack = HashSet::new();
        let mut ctx = ExpandContext {
            index: &index,
            owner: &owner,
            caller: &caller,
            unresolved: &mut unresolved,
            stack: &mut stack,
            depth: 0,
        };

        let branch =
            optional_branch(&mut ctx, &call).expect("ifPresentOrElse is optional flow control");

        assert_eq!(branch.kind, BranchKind::Optional);
        assert_eq!(branch.arms.len(), 2);
        assert_eq!(branch.arms[0].label, "present");
        assert_eq!(branch.arms[1].label, "empty");
        assert!(matches!(branch.arms[0].children[0], FlowNode::Lambda(_)));
        assert!(matches!(branch.arms[1].children[0], FlowNode::Lambda(_)));
    }

    #[test]
    fn optional_or_else_throw_without_supplier_gets_implicit_empty_throw_arm() {
        let index = ProjectIndex::default();
        let owner = class();
        let caller = method();
        let call = optional_call("orElseThrow", 0, Vec::new());
        let mut unresolved = Vec::new();
        let mut stack = HashSet::new();
        let mut ctx = ExpandContext {
            index: &index,
            owner: &owner,
            caller: &caller,
            unresolved: &mut unresolved,
            stack: &mut stack,
            depth: 0,
        };

        let branch =
            optional_branch(&mut ctx, &call).expect("orElseThrow() is optional flow control");

        assert_eq!(branch.arms[0].label, "present");
        assert_eq!(branch.arms[1].label, "empty");
        assert!(branch.arms[1].terminates);
        let FlowNode::Call(call) = &branch.arms[1].children[0] else {
            panic!("expected implicit throw constructor call");
        };
        assert_eq!(call.method_fqn.0, "NoSuchElementException#<init>()");
        assert_eq!(call.external_kind, Some(ExternalKind::JdkLibrary));
    }

    #[test]
    fn routine_jdk_calls_use_low_signal_external_kind() {
        assert_eq!(
            external_kind_for_call(ExternalKind::Jdk, "Boolean", "equals", 1),
            ExternalKind::JdkLibrary
        );
        assert_eq!(
            external_kind_for_call(ExternalKind::Jdk, "List", "add", 1),
            ExternalKind::JdkLibrary
        );
        assert_eq!(
            external_kind_for_call(ExternalKind::Jdk, "List", "stream", 0),
            ExternalKind::JdkLibrary
        );
        assert_eq!(
            external_kind_for_call(ExternalKind::Jdk, "Stream", "toList", 0),
            ExternalKind::JdkLibrary
        );
        assert_eq!(
            external_kind_for_call(ExternalKind::Jdk, "List", "forEach", 1),
            ExternalKind::Jdk
        );
        assert_eq!(
            external_kind_for_call(ExternalKind::ThirdParty, "Client", "send", 1),
            ExternalKind::ThirdParty
        );
    }

    #[test]
    fn optional_return_shapes_cover_control_methods() {
        assert_eq!(return_shape("map", 1), Some("Optional"));
        assert_eq!(return_shape("flatMap", 1), Some("Optional"));
        assert_eq!(return_shape("filter", 1), Some("Optional"));
        assert_eq!(return_shape("or", 1), Some("Optional"));
        assert_eq!(return_shape("orElseGet", 1), Some("Object"));
        assert_eq!(return_shape("orElseThrow", 0), Some("Object"));
        assert_eq!(return_shape("stream", 0), Some("Stream"));
    }

    #[test]
    fn stream_map_lambda_becomes_loop_body() {
        let index = ProjectIndex::default();
        let owner = class();
        let caller = method();
        let call = stream_call("map", vec![lambda("user -> toResponse(user)")]);
        let mut unresolved = Vec::new();
        let mut stack = HashSet::new();
        let mut ctx = ExpandContext {
            index: &index,
            owner: &owner,
            caller: &caller,
            unresolved: &mut unresolved,
            stack: &mut stack,
            depth: 0,
        };

        let loop_node = external_loop(&mut ctx, &call, "Stream").expect("map traverses stream");

        assert_eq!(loop_node.kind, LoopKind::Stream);
        assert_eq!(loop_node.source, "Stream.map");
        assert_eq!(loop_node.execution, LoopExecution::ZeroOrMore);
        assert!(loop_node.init.is_empty());
        assert!(loop_node.condition.is_empty());
        assert_eq!(loop_node.arms[0].label, "body");
        assert!(matches!(loop_node.arms[0].children[0], FlowNode::Lambda(_)));
        assert!(loop_node.update.is_empty());
    }

    #[test]
    fn iterable_for_each_lambda_becomes_loop_body() {
        let index = ProjectIndex::default();
        let owner = class();
        let caller = method();
        let call = list_call("forEach", vec![lambda("user -> audit(user)")]);
        let mut unresolved = Vec::new();
        let mut stack = HashSet::new();
        let mut ctx = ExpandContext {
            index: &index,
            owner: &owner,
            caller: &caller,
            unresolved: &mut unresolved,
            stack: &mut stack,
            depth: 0,
        };

        let loop_node = external_loop(&mut ctx, &call, "List").expect("forEach traverses list");

        assert_eq!(loop_node.kind, LoopKind::ForEach);
        assert_eq!(loop_node.source, "List.forEach");
        assert_eq!(loop_node.execution, LoopExecution::ZeroOrMore);
        assert_eq!(loop_node.arms[0].label, "body");
        assert!(matches!(loop_node.arms[0].children[0], FlowNode::Lambda(_)));
    }

    #[test]
    fn stream_return_shapes_cover_traversal_methods() {
        assert_eq!(jdk_shape("Stream", "map", 1), Some("Stream"));
        assert_eq!(jdk_shape("Stream", "flatMap", 1), Some("Stream"));
        assert_eq!(jdk_shape("Stream", "filter", 1), Some("Stream"));
        assert_eq!(jdk_shape("Stream", "peek", 1), Some("Stream"));
        assert_eq!(jdk_shape("Stream", "anyMatch", 1), Some("Boolean"));
        assert_eq!(jdk_shape("Stream", "forEach", 1), Some("void"));
        assert_eq!(jdk_shape("List", "forEach", 1), Some("void"));
    }

    fn return_shape(method: &str, arity: usize) -> Option<&'static str> {
        jdk_shape("Optional", method, arity)
    }

    fn jdk_shape(receiver: &str, method: &str, arity: usize) -> Option<&'static str> {
        jdk_return_type(receiver, method, arity).map(|ty| match ty.raw.as_str() {
            "Optional" => "Optional",
            "Object" => "Object",
            "Stream" => "Stream",
            "Boolean" => "Boolean",
            "void" => "void",
            other => panic!("unexpected return type {other}"),
        })
    }

    fn optional_call(method_name: &str, arity: usize, lambdas: Vec<LambdaSyntax>) -> CallSite {
        CallSite {
            receiver: ReceiverKind::TypeName("Optional".to_string()),
            method_name: method_name.to_string(),
            arity,
            lambdas,
            line: 1,
        }
    }

    fn stream_call(method_name: &str, lambdas: Vec<LambdaSyntax>) -> CallSite {
        CallSite {
            receiver: ReceiverKind::TypeName("Stream".to_string()),
            method_name: method_name.to_string(),
            arity: 1,
            lambdas,
            line: 1,
        }
    }

    fn list_call(method_name: &str, lambdas: Vec<LambdaSyntax>) -> CallSite {
        CallSite {
            receiver: ReceiverKind::TypeName("List".to_string()),
            method_name: method_name.to_string(),
            arity: 1,
            lambdas,
            line: 1,
        }
    }

    fn lambda(source: &str) -> LambdaSyntax {
        LambdaSyntax {
            kind: LambdaKind::Lambda,
            source: source.to_string(),
            body: Vec::new(),
        }
    }

    fn class() -> ClassInfo {
        ClassInfo {
            fqn: Fqn("Demo".to_string()),
            simple_name: "Demo".to_string(),
            package: String::new(),
            imports: HashMap::new(),
            kind: ClassKind::Class,
            annotations: Vec::new(),
            extends: Vec::new(),
            implements: Vec::new(),
            fields: Vec::<FieldInfo>::new(),
            methods: vec![method()],
            file: PathBuf::from("Demo.java"),
            line: 1,
        }
    }

    fn method() -> MethodInfo {
        MethodInfo {
            fqn: Fqn("Demo#caller()".to_string()),
            name: "caller".to_string(),
            params: Vec::<ParamInfo>::new(),
            return_type: TypeRef {
                raw: "void".to_string(),
                generics: Vec::new(),
            },
            annotations: Vec::new(),
            body: Vec::new(),
            locals: HashMap::new(),
            file: PathBuf::from("Demo.java"),
            line: 1,
        }
    }
}
