//! Expansion from parsed method body syntax into flow graph nodes.

use std::collections::HashSet;

use crate::model::{
    Arm, BodyElement, BranchNode, CallNode, CallSite, ClassInfo, Confidence, FlowNode, Fqn,
    LambdaKind, LambdaNode, LambdaSyntax, LoopLocal, LoopNode, LoopSyntax, MethodInfo,
    ProjectIndex, ReceiverKind, UnresolvedRef,
};

use super::MAX_DEPTH;
use super::node::{starts_uppercase, unresolved_node};
use super::resolve::resolve_call;

pub(super) fn expand_method(
    index: &ProjectIndex,
    owner: &ClassInfo,
    method: &MethodInfo,
    confidence: Confidence,
    unresolved: &mut Vec<UnresolvedRef>,
    stack: &mut HashSet<Fqn>,
    depth: usize,
) -> CallNode {
    // This cap bounds graph construction; render-time depth limits are separate.
    if depth >= MAX_DEPTH {
        unresolved.push(UnresolvedRef {
            receiver_type: owner.simple_name.clone(),
            method_name: method.name.clone(),
            reason: format!("depth cap of {MAX_DEPTH} reached"),
        });
        return unresolved_node(
            method.fqn.clone(),
            Some(format!("depth cap of {MAX_DEPTH} reached")),
        );
    }

    if !stack.insert(method.fqn.clone()) {
        unresolved.push(UnresolvedRef {
            receiver_type: owner.simple_name.clone(),
            method_name: method.name.clone(),
            reason: "cycle guard truncated this call".to_string(),
        });
        return unresolved_node(method.fqn.clone(), Some("cycle guard".to_string()));
    }

    let children = expand_body(index, owner, method, &method.body, unresolved, stack, depth);
    stack.remove(&method.fqn);

    CallNode {
        method_fqn: method.fqn.clone(),
        confidence,
        external_kind: None,
        control_kind: None,
        scope: None,
        note: None,
        children,
    }
}

fn expand_body(
    index: &ProjectIndex,
    owner: &ClassInfo,
    caller: &MethodInfo,
    body: &[BodyElement],
    unresolved: &mut Vec<UnresolvedRef>,
    stack: &mut HashSet<Fqn>,
    caller_depth: usize,
) -> Vec<FlowNode> {
    let mut nodes = Vec::new();
    for element in body {
        match element {
            BodyElement::Call(call) => {
                nodes.push(FlowNode::Call(resolve_call(
                    index,
                    owner,
                    caller,
                    call,
                    unresolved,
                    stack,
                    caller_depth + 1,
                )));
            }
            BodyElement::Branch(branch) => {
                // Control structure wrappers do not consume call depth.
                nodes.extend(expand_body(
                    index,
                    owner,
                    caller,
                    &branch.condition_calls,
                    unresolved,
                    stack,
                    caller_depth,
                ));
                let arms = branch
                    .arms
                    .iter()
                    .map(|arm| Arm {
                        label: arm.label.clone(),
                        terminates: arm.terminates,
                        children: expand_body(
                            index,
                            owner,
                            caller,
                            &arm.body,
                            unresolved,
                            stack,
                            caller_depth,
                        ),
                    })
                    .collect();
                nodes.push(FlowNode::Branch(BranchNode {
                    kind: branch.kind,
                    condition_src: branch.condition_src.clone(),
                    arms,
                }));
            }
            BodyElement::Loop(loop_syntax) => {
                nodes.push(FlowNode::Loop(expand_loop(
                    index,
                    owner,
                    caller,
                    loop_syntax,
                    unresolved,
                    stack,
                    caller_depth,
                )));
            }
        }
    }
    nodes
}

fn expand_loop(
    index: &ProjectIndex,
    owner: &ClassInfo,
    caller: &MethodInfo,
    loop_syntax: &LoopSyntax,
    unresolved: &mut Vec<UnresolvedRef>,
    stack: &mut HashSet<Fqn>,
    caller_depth: usize,
) -> LoopNode {
    let scoped_caller = caller_with_loop_locals(caller, &loop_syntax.locals);

    LoopNode {
        kind: loop_syntax.kind,
        source: loop_syntax.source.clone(),
        condition: expand_body(
            index,
            owner,
            caller,
            &loop_syntax.condition_calls,
            unresolved,
            stack,
            caller_depth,
        ),
        body: expand_body(
            index,
            owner,
            &scoped_caller,
            &loop_syntax.body,
            unresolved,
            stack,
            caller_depth,
        ),
        update: expand_body(
            index,
            owner,
            &scoped_caller,
            &loop_syntax.update_calls,
            unresolved,
            stack,
            caller_depth,
        ),
    }
}

fn caller_with_loop_locals(caller: &MethodInfo, locals: &[LoopLocal]) -> MethodInfo {
    let mut caller = caller.clone();
    for local in locals {
        caller.locals.insert(local.name.clone(), local.ty.clone());
    }
    caller
}

pub(super) fn expand_lambdas(
    index: &ProjectIndex,
    owner: &ClassInfo,
    caller: &MethodInfo,
    lambdas: &[LambdaSyntax],
    unresolved: &mut Vec<UnresolvedRef>,
    stack: &mut HashSet<Fqn>,
    caller_depth: usize,
) -> Vec<FlowNode> {
    lambdas
        .iter()
        .map(|lambda| {
            FlowNode::Lambda(LambdaNode {
                kind: lambda.kind,
                source: lambda.source.clone(),
                children: match lambda.kind {
                    LambdaKind::Lambda => expand_body(
                        index,
                        owner,
                        caller,
                        &lambda.body,
                        unresolved,
                        stack,
                        caller_depth,
                    ),
                    LambdaKind::MethodRef => expand_method_ref(
                        index,
                        owner,
                        caller,
                        &lambda.source,
                        unresolved,
                        stack,
                        caller_depth + 1,
                    ),
                },
            })
        })
        .collect()
}

fn expand_method_ref(
    index: &ProjectIndex,
    owner: &ClassInfo,
    caller: &MethodInfo,
    source: &str,
    unresolved: &mut Vec<UnresolvedRef>,
    stack: &mut HashSet<Fqn>,
    depth: usize,
) -> Vec<FlowNode> {
    let Some((receiver, method_name)) = source.split_once("::") else {
        return Vec::new();
    };
    let receiver = if receiver.trim() == "this" {
        ReceiverKind::This
    } else if method_name.trim() == "new" && starts_uppercase(receiver.trim()) {
        ReceiverKind::Constructor(receiver.trim().to_string())
    } else {
        ReceiverKind::Local(receiver.trim().to_string())
    };
    let call = CallSite {
        receiver,
        method_name: if method_name.trim() == "new" {
            "<init>".to_string()
        } else {
            method_name.trim().to_string()
        },
        // Method references do not expose arity here, so matching accepts any arity.
        arity: usize::MAX,
        lambdas: Vec::new(),
        line: 0,
    };

    vec![FlowNode::Call(resolve_call(
        index, owner, caller, &call, unresolved, stack, depth,
    ))]
}
