//! Receiver/type resolution for parsed call sites.
//!
//! Resolution favors project symbols, then imported externals, same-package
//! classes, fully qualified names, and finally known JDK simple types.

use std::collections::HashSet;

use crate::model::{
    BranchKind, CallNode, CallSite, ClassInfo, ClassKind, Confidence, ControlKind, ExternalKind,
    FlowNode, Fqn, MethodInfo, ProjectIndex, ReceiverKind, TypeRef, UnresolvedRef,
};
use crate::spring::jpa;

use super::expand::{expand_body, expand_lambdas, expand_method};
use super::external::{
    ExpandContext, expand_external_call_children, external_kind_for, external_kind_for_call,
    external_params, is_jdk_simple, jdk_return_type,
};
use super::node::{
    external_node, scoped, short_type, static_type_ref, strip_generics, unknown_params,
    unresolved_node,
};

pub(super) fn resolve_call(
    index: &ProjectIndex,
    owner: &ClassInfo,
    caller: &MethodInfo,
    call: &CallSite,
    unresolved: &mut Vec<UnresolvedRef>,
    stack: &mut HashSet<Fqn>,
    depth: usize,
) -> CallNode {
    let resolved_type = match receiver_type(index, owner, caller, call) {
        Some(resolved_type) => resolved_type,
        None => {
            unresolved.push(UnresolvedRef {
                receiver_type: "unknown".to_string(),
                method_name: call.method_name.clone(),
                reason: format!("could not determine receiver at line {}", call.line),
            });
            let mut node = unresolved_node(
                Fqn(format!(
                    "Unknown#{}({})",
                    call.method_name,
                    unknown_params(call.arity)
                )),
                Some("receiver type unknown".to_string()),
            );
            let mut ctx = ExpandContext {
                index,
                owner,
                caller,
                unresolved,
                stack,
                depth,
            };
            attach_inputs(&mut node, call, &mut ctx);
            return node;
        }
    };

    match resolved_type {
        ResolvedType::Project(receiver_fqn) => {
            let Some(receiver_class) = index.classes.get(&receiver_fqn) else {
                return external_node(
                    Fqn(format!(
                        "{}#{}({})",
                        short_type(&receiver_fqn.0),
                        call.method_name,
                        unknown_params(call.arity)
                    )),
                    ExternalKind::Unknown,
                );
            };

            if let Some(target) =
                find_matching_method(receiver_class, &call.method_name, call.arity)
            {
                if receiver_class.kind == ClassKind::Interface
                    && jpa::is_spring_data_repository(receiver_class)
                {
                    let mut node = external_node(target.fqn.clone(), ExternalKind::SpringData);
                    let mut ctx = ExpandContext {
                        index,
                        owner,
                        caller,
                        unresolved,
                        stack,
                        depth,
                    };
                    attach_inputs(&mut node, call, &mut ctx);
                    attach_lambdas(&mut node, call, &mut ctx);
                    return node;
                }
                let mut node = expand_method(
                    index,
                    receiver_class,
                    target,
                    Confidence::Resolved,
                    unresolved,
                    stack,
                    depth,
                );
                let mut ctx = ExpandContext {
                    index,
                    owner,
                    caller,
                    unresolved,
                    stack,
                    depth,
                };
                attach_inputs(&mut node, call, &mut ctx);
                attach_lambdas(&mut node, call, &mut ctx);
                return scoped(call, node);
            }

            if receiver_class.kind == ClassKind::Interface
                && jpa::is_spring_data_repository(receiver_class)
                && jpa::is_inherited_method(&call.method_name, call.arity)
            {
                let params =
                    jpa::inherited_param_types(receiver_class, &call.method_name, call.arity);
                let mut node = external_node(
                    Fqn(format!(
                        "{}#{}({})",
                        receiver_class.fqn.0,
                        call.method_name,
                        params.join(", ")
                    )),
                    ExternalKind::SpringData,
                );
                let mut ctx = ExpandContext {
                    index,
                    owner,
                    caller,
                    unresolved,
                    stack,
                    depth,
                };
                attach_inputs(&mut node, call, &mut ctx);
                attach_lambdas(&mut node, call, &mut ctx);
                return node;
            }

            unresolved.push(UnresolvedRef {
                receiver_type: receiver_class.simple_name.clone(),
                method_name: call.method_name.clone(),
                reason: "no method matched by name and arity".to_string(),
            });
            let mut node = unresolved_node(
                Fqn(format!(
                    "{}#{}({})",
                    receiver_class.fqn.0,
                    call.method_name,
                    unknown_params(call.arity)
                )),
                Some("no method matched".to_string()),
            );
            let mut ctx = ExpandContext {
                index,
                owner,
                caller,
                unresolved,
                stack,
                depth,
            };
            attach_inputs(&mut node, call, &mut ctx);
            node
        }
        ResolvedType::External { label, kind } => {
            let kind = external_kind_for_call(kind, &label, &call.method_name, call.arity);
            let mut node = external_node(
                Fqn(format!(
                    "{}#{}({})",
                    label,
                    call.method_name,
                    external_params(&label, &call.method_name, call.arity)
                )),
                kind,
            );
            let mut ctx = ExpandContext {
                index,
                owner,
                caller,
                unresolved,
                stack,
                depth,
            };
            attach_inputs(&mut node, call, &mut ctx);
            node.children = expand_external_call_children(&mut ctx, call, &label);
            if label == "Optional"
                && node
                    .children
                    .iter()
                    .any(|child| matches!(child, FlowNode::Branch(branch) if branch.kind == BranchKind::Optional))
            {
                node.control_kind = Some(ControlKind::Optional);
            } else if node
                .children
                .iter()
                .any(|child| matches!(child, FlowNode::Loop(_)))
            {
                node.control_kind = Some(ControlKind::Traversal);
            }
            node
        }
    }
}

fn attach_inputs(node: &mut CallNode, call: &CallSite, ctx: &mut ExpandContext<'_, '_>) {
    node.inputs = expand_body(
        ctx.index,
        ctx.owner,
        ctx.caller,
        &call.inputs,
        ctx.unresolved,
        ctx.stack,
        ctx.depth.saturating_sub(1),
    );
}

fn attach_lambdas(node: &mut CallNode, call: &CallSite, ctx: &mut ExpandContext<'_, '_>) {
    node.children.extend(expand_lambdas(
        ctx.index,
        ctx.owner,
        ctx.caller,
        &call.lambdas,
        ctx.unresolved,
        ctx.stack,
        ctx.depth,
    ));
}

fn receiver_type(
    index: &ProjectIndex,
    owner: &ClassInfo,
    caller: &MethodInfo,
    call: &CallSite,
) -> Option<ResolvedType> {
    match &call.receiver {
        ReceiverKind::This => Some(ResolvedType::Project(owner.fqn.clone())),
        ReceiverKind::Constructor(name) => resolve_type(
            index,
            owner,
            &TypeRef {
                raw: name.clone(),
                generics: Vec::new(),
            },
        ),
        ReceiverKind::TypeName(name) => resolve_type(
            index,
            owner,
            &TypeRef {
                raw: name.clone(),
                generics: Vec::new(),
            },
        ),
        ReceiverKind::Field(name) | ReceiverKind::Local(name) => {
            // Locals shadow params, and params shadow fields in Java method scope.
            if let Some(ty) = caller.locals.get(name) {
                return resolve_type(index, owner, ty);
            }
            if let Some(param) = caller.params.iter().find(|param| param.name == *name) {
                let ty = static_type_ref(&param.ty);
                return resolve_type(index, owner, &ty);
            }
            owner
                .fields
                .iter()
                .find(|field| field.name == *name)
                .and_then(|field| resolve_type(index, owner, &field.ty))
        }
        ReceiverKind::Chain(inner) => call_return_type(index, owner, caller, inner)
            .and_then(|return_type| resolve_type(index, owner, &return_type)),
    }
}

fn call_return_type(
    index: &ProjectIndex,
    owner: &ClassInfo,
    caller: &MethodInfo,
    call: &CallSite,
) -> Option<TypeRef> {
    let receiver = receiver_type(index, owner, caller, call)?;
    match receiver {
        ResolvedType::Project(receiver_fqn) => {
            let receiver_class = index.classes.get(&receiver_fqn)?;
            if let Some(method) =
                find_matching_method(receiver_class, &call.method_name, call.arity)
            {
                return Some(method.return_type.clone());
            }
            if receiver_class.kind == ClassKind::Interface
                && jpa::is_spring_data_repository(receiver_class)
            {
                return jpa::inherited_return_type(receiver_class, &call.method_name, call.arity);
            }
            None
        }
        ResolvedType::External { label, .. } => {
            jdk_return_type(&label, &call.method_name, call.arity)
        }
    }
}

fn resolve_type(index: &ProjectIndex, owner: &ClassInfo, ty: &TypeRef) -> Option<ResolvedType> {
    let raw = strip_generics(&ty.raw);
    if raw.is_empty() || raw == "void" {
        return None;
    }

    // Keep this order aligned with Java lookup needs in the demo fixture.
    if let Some(fqn) = index.by_simple_name.get(raw).and_then(|fqns| fqns.first()) {
        return Some(ResolvedType::Project(fqn.clone()));
    }

    if let Some(import) = owner.imports.get(raw) {
        if let Some(class) = index.classes.get(&Fqn(import.clone())) {
            return Some(ResolvedType::Project(class.fqn.clone()));
        }
        return Some(ResolvedType::External {
            label: raw.to_string(),
            kind: external_kind_for(import),
        });
    }

    let same_package = if owner.package.is_empty() {
        raw.to_string()
    } else {
        format!("{}.{}", owner.package, raw)
    };
    if index.classes.contains_key(&Fqn(same_package.clone())) {
        return Some(ResolvedType::Project(Fqn(same_package)));
    }

    if raw.contains('.') {
        return Some(ResolvedType::External {
            label: short_type(raw).to_string(),
            kind: external_kind_for(raw),
        });
    }

    if is_jdk_simple(raw) {
        return Some(ResolvedType::External {
            label: raw.to_string(),
            kind: ExternalKind::Jdk,
        });
    }

    None
}

pub(super) fn find_method<'a>(
    index: &'a ProjectIndex,
    fqn: &Fqn,
) -> Option<(&'a ClassInfo, &'a MethodInfo)> {
    let class_fqn = fqn.0.split('#').next()?;
    let class = index.classes.get(&Fqn(class_fqn.to_string()))?;
    let method = class.methods.iter().find(|method| method.fqn == *fqn)?;
    Some((class, method))
}

fn find_matching_method<'a>(
    class: &'a ClassInfo,
    name: &str,
    arity: usize,
) -> Option<&'a MethodInfo> {
    class
        .methods
        .iter()
        .find(|method| method.name == name && (method.params.len() == arity || arity == usize::MAX))
}

enum ResolvedType {
    Project(Fqn),
    External { label: String, kind: ExternalKind },
}
