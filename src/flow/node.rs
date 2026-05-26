use crate::model::{
    CallNode, CallSite, Confidence, ExternalKind, Fqn, ReceiverKind, Scope, TypeRef,
};

pub(super) fn external_node(method_fqn: Fqn, kind: ExternalKind) -> CallNode {
    CallNode {
        method_fqn,
        confidence: Confidence::External,
        external_kind: Some(kind),
        control_kind: None,
        scope: None,
        note: None,
        children: Vec::new(),
    }
}

pub(super) fn unresolved_node(method_fqn: Fqn, note: Option<String>) -> CallNode {
    CallNode {
        method_fqn,
        confidence: Confidence::Unresolved,
        external_kind: None,
        control_kind: None,
        scope: None,
        note,
        children: Vec::new(),
    }
}

pub(super) fn scoped(call: &CallSite, mut node: CallNode) -> CallNode {
    if matches!(call.receiver, ReceiverKind::This) {
        node.scope = Some(Scope::IntraClass);
    }
    node
}

pub(super) fn strip_generics(raw: &str) -> &str {
    raw.split('<').next().unwrap_or(raw).trim()
}

pub(super) fn short_type(raw: &str) -> &str {
    raw.rsplit('.').next().unwrap_or(raw)
}

pub(super) fn starts_uppercase(value: &str) -> bool {
    value.chars().next().is_some_and(char::is_uppercase)
}

pub(super) fn unknown_params(arity: usize) -> String {
    if arity == usize::MAX {
        return "_".to_string();
    }
    vec!["_"; arity].join(", ")
}

pub(super) fn static_type_ref(raw: &str) -> TypeRef {
    TypeRef {
        raw: raw.to_string(),
        generics: Vec::new(),
    }
}
