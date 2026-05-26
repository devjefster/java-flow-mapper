//! Method body extraction for calls, branches, loops, locals, and lambdas.

use std::collections::HashMap;
use tree_sitter::Node;

use crate::model::{
    BodyElement, BranchArmSyntax, BranchKind, BranchSyntax, CallSite, LambdaKind, LambdaSyntax,
    LoopKind, LoopLocal, LoopSyntax, ParamInfo, ReceiverKind, TypeRef,
};

use super::annotations::param_source;
use super::class::parse_type_ref;
use super::utils::{
    count_args, line, named_children, node_text, significant_tokens, split_top_level,
    starts_uppercase, strip_generics, text,
};

/// Collect renderable body elements from a Java syntax node.
pub fn collect_body_elements(node: Node<'_>, source: &str) -> Vec<BodyElement> {
    let mut elements = Vec::new();
    collect_body_elements_into(node, source, &mut elements);
    elements
}

/// Append body elements found under `node`.
pub fn collect_body_elements_into(node: Node<'_>, source: &str, elements: &mut Vec<BodyElement>) {
    if node.kind() == "lambda_expression" {
        // Lambdas are attached to their owning call argument, not flattened here.
        return;
    }

    if node.kind() == "if_statement" {
        elements.push(BodyElement::Branch(parse_if_statement(node, source)));
        return;
    }

    if matches!(node.kind(), "switch_statement" | "switch_expression") {
        elements.push(BodyElement::Branch(parse_switch_statement(node, source)));
        return;
    }

    if node.kind() == "ternary_expression" {
        elements.push(BodyElement::Branch(parse_ternary_expression(node, source)));
        return;
    }

    if node.kind() == "try_statement" {
        elements.push(BodyElement::Branch(parse_try_statement(node, source)));
        return;
    }

    if is_loop_statement(node.kind()) {
        elements.push(BodyElement::Loop(parse_loop_statement(node, source)));
        return;
    }

    for child in named_children(node) {
        collect_body_elements_into(child, source, elements);
    }

    match node.kind() {
        "method_invocation" => {
            if let Some(call) = parse_method_invocation(node, source) {
                elements.push(BodyElement::Call(call));
            }
        }
        "object_creation_expression" => {
            if let Some(call) = parse_constructor_invocation(node, source) {
                elements.push(BodyElement::Call(call));
            }
        }
        _ => {}
    }
}

/// Parse an `if_statement` into a branch syntax node.
pub fn parse_if_statement(node: Node<'_>, source: &str) -> BranchSyntax {
    let condition = node.child_by_field_name("condition");
    let consequence = node.child_by_field_name("consequence");
    let alternative = node.child_by_field_name("alternative");
    let then_arm = consequence
        .map(|consequence| collect_body_elements(consequence, source))
        .unwrap_or_default();
    let else_arm = alternative.map(|alternative| collect_body_elements(alternative, source));
    let then_terminates = consequence.is_some_and(arm_terminates);
    let else_terminates = alternative.is_some_and(arm_terminates);

    let mut arms = vec![BranchArmSyntax {
        label: "then".to_string(),
        body: then_arm.clone(),
        terminates: then_terminates,
    }];
    if let Some(else_arm) = &else_arm {
        arms.push(BranchArmSyntax {
            label: "else".to_string(),
            body: else_arm.clone(),
            terminates: else_terminates,
        });
    }

    BranchSyntax {
        kind: BranchKind::If,
        condition_src: condition
            .map(|condition| condition_source(condition, source))
            .unwrap_or_default(),
        condition_calls: condition
            .map(|condition| collect_body_elements(condition, source))
            .unwrap_or_default(),
        arms,
        then_arm,
        else_arm,
        then_terminates,
        else_terminates,
    }
}

/// Parse a `switch_statement` into a branch syntax node.
pub fn parse_switch_statement(node: Node<'_>, source: &str) -> BranchSyntax {
    let condition = node.child_by_field_name("condition");

    BranchSyntax {
        kind: BranchKind::Switch,
        condition_src: condition
            .map(|condition| condition_source(condition, source))
            .unwrap_or_else(|| header_source(node, source)),
        condition_calls: condition
            .map(|condition| collect_body_elements(condition, source))
            .unwrap_or_default(),
        arms: switch_arms(node, source),
        then_arm: Vec::new(),
        else_arm: None,
        then_terminates: false,
        else_terminates: false,
    }
}

fn switch_arms(node: Node<'_>, source: &str) -> Vec<BranchArmSyntax> {
    let Some(body) = node.child_by_field_name("body") else {
        return Vec::new();
    };
    let mut arms = Vec::new();
    for child in named_children(body) {
        match child.kind() {
            "switch_block_statement_group" | "switch_rule" => {
                arms.extend(switch_group_arms(child, source));
            }
            _ => {}
        }
    }
    arms
}

fn switch_group_arms(group: Node<'_>, source: &str) -> Vec<BranchArmSyntax> {
    let children = named_children(group);
    let labels = children
        .iter()
        .filter(|child| child.kind() == "switch_label")
        .map(|label| switch_label_text(*label, source))
        .collect::<Vec<_>>();
    let body = children
        .iter()
        .filter(|child| child.kind() != "switch_label")
        .flat_map(|child| collect_body_elements(*child, source))
        .collect::<Vec<_>>();
    let terminates = children
        .iter()
        .rev()
        .find(|child| child.kind() != "switch_label")
        .is_some_and(|child| arm_terminates(*child));

    labels
        .into_iter()
        .map(|label| BranchArmSyntax {
            label,
            body: body.clone(),
            terminates,
        })
        .collect()
}

fn switch_label_text(label: Node<'_>, source: &str) -> String {
    let value = text(label, source);
    let trimmed = value
        .trim()
        .trim_end_matches(':')
        .trim_end_matches("->")
        .trim();
    if trimmed == "default" {
        "default".to_string()
    } else {
        trimmed
            .strip_prefix("case")
            .unwrap_or(trimmed)
            .trim()
            .to_string()
    }
}

/// Parse a ternary expression into a two-arm branch syntax node.
pub fn parse_ternary_expression(node: Node<'_>, source: &str) -> BranchSyntax {
    let condition = node.child_by_field_name("condition");
    let consequence = node.child_by_field_name("consequence");
    let alternative = node.child_by_field_name("alternative");

    let then_arm = consequence
        .map(|consequence| collect_body_elements(consequence, source))
        .unwrap_or_default();
    let else_arm = alternative
        .map(|alternative| collect_body_elements(alternative, source))
        .unwrap_or_default();

    BranchSyntax {
        kind: BranchKind::Ternary,
        condition_src: condition
            .map(|condition| text(condition, source))
            .unwrap_or_default(),
        condition_calls: condition
            .map(|condition| collect_body_elements(condition, source))
            .unwrap_or_default(),
        arms: vec![
            BranchArmSyntax {
                label: "then".to_string(),
                body: then_arm.clone(),
                terminates: false,
            },
            BranchArmSyntax {
                label: "else".to_string(),
                body: else_arm.clone(),
                terminates: false,
            },
        ],
        then_arm,
        else_arm: Some(else_arm),
        then_terminates: false,
        else_terminates: false,
    }
}

/// Parse a try/catch/finally statement into labeled branch arms.
pub fn parse_try_statement(node: Node<'_>, source: &str) -> BranchSyntax {
    let mut arms = Vec::new();
    let body = node.child_by_field_name("body");
    if let Some(body) = body {
        arms.push(BranchArmSyntax {
            label: "try".to_string(),
            body: collect_body_elements(body, source),
            terminates: arm_terminates(body),
        });
    }

    for child in named_children(node) {
        match child.kind() {
            "catch_clause" => arms.push(catch_arm(child, source)),
            "finally_clause" => arms.push(finally_arm(child, source)),
            _ => {}
        }
    }

    BranchSyntax {
        kind: BranchKind::TryCatch,
        condition_src: "try".to_string(),
        condition_calls: try_resource_calls(node, source),
        arms,
        then_arm: body
            .map(|body| collect_body_elements(body, source))
            .unwrap_or_default(),
        else_arm: None,
        then_terminates: body.is_some_and(arm_terminates),
        else_terminates: false,
    }
}

fn catch_arm(catch: Node<'_>, source: &str) -> BranchArmSyntax {
    let body = catch.child_by_field_name("body").or_else(|| {
        named_children(catch)
            .into_iter()
            .find(|child| child.kind() == "block")
    });
    BranchArmSyntax {
        label: format!("catch {}", catch_label(catch, source)),
        body: body
            .map(|body| collect_body_elements(body, source))
            .unwrap_or_default(),
        terminates: body.is_some_and(arm_terminates),
    }
}

fn finally_arm(finally: Node<'_>, source: &str) -> BranchArmSyntax {
    let body = finally.child_by_field_name("body").or_else(|| {
        named_children(finally)
            .into_iter()
            .find(|child| child.kind() == "block")
    });
    BranchArmSyntax {
        label: "finally".to_string(),
        body: body
            .map(|body| collect_body_elements(body, source))
            .unwrap_or_default(),
        terminates: body.is_some_and(arm_terminates),
    }
}

fn catch_label(catch: Node<'_>, source: &str) -> String {
    named_children(catch)
        .into_iter()
        .find(|child| child.kind() != "block")
        .map(|child| text(child, source))
        .unwrap_or_else(|| "unknown".to_string())
}

fn try_resource_calls(node: Node<'_>, source: &str) -> Vec<BodyElement> {
    named_children(node)
        .into_iter()
        .filter(|child| !matches!(child.kind(), "block" | "catch_clause" | "finally_clause"))
        .flat_map(|child| collect_body_elements(child, source))
        .collect()
}

fn is_loop_statement(kind: &str) -> bool {
    matches!(
        kind,
        "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement"
    )
}

/// Parse a Java loop statement into a loop syntax node.
pub fn parse_loop_statement(node: Node<'_>, source: &str) -> LoopSyntax {
    match node.kind() {
        "for_statement" => parse_for_statement(node, source),
        "enhanced_for_statement" => parse_enhanced_for_statement(node, source),
        "while_statement" => parse_while_statement(node, source),
        "do_statement" => parse_do_statement(node, source),
        _ => LoopSyntax {
            kind: LoopKind::For,
            source: header_source(node, source),
            condition_calls: Vec::new(),
            body: Vec::new(),
            update_calls: Vec::new(),
            locals: Vec::new(),
        },
    }
}

fn parse_for_statement(node: Node<'_>, source: &str) -> LoopSyntax {
    let mut condition_calls = Vec::new();
    if let Some(init) = node.child_by_field_name("init") {
        condition_calls.extend(collect_body_elements(init, source));
    }
    if let Some(condition) = node.child_by_field_name("condition") {
        condition_calls.extend(collect_body_elements(condition, source));
    }

    LoopSyntax {
        kind: LoopKind::For,
        source: header_source(node, source),
        condition_calls,
        body: loop_body(node)
            .map(|body| collect_body_elements(body, source))
            .unwrap_or_default(),
        update_calls: node
            .child_by_field_name("update")
            .map(|update| collect_body_elements(update, source))
            .unwrap_or_default(),
        locals: Vec::new(),
    }
}

fn parse_enhanced_for_statement(node: Node<'_>, source: &str) -> LoopSyntax {
    let iterable = node
        .child_by_field_name("value")
        .or_else(|| node.child_by_field_name("iterable"));

    LoopSyntax {
        kind: LoopKind::EnhancedFor,
        source: header_source(node, source),
        condition_calls: iterable
            .map(|iterable| collect_body_elements(iterable, source))
            .unwrap_or_default(),
        body: loop_body(node)
            .map(|body| collect_body_elements(body, source))
            .unwrap_or_default(),
        update_calls: Vec::new(),
        locals: enhanced_for_local(node, source).into_iter().collect(),
    }
}

fn parse_while_statement(node: Node<'_>, source: &str) -> LoopSyntax {
    let condition = node.child_by_field_name("condition");

    LoopSyntax {
        kind: LoopKind::While,
        source: condition
            .map(|condition| condition_source(condition, source))
            .unwrap_or_else(|| header_source(node, source)),
        condition_calls: condition
            .map(|condition| collect_body_elements(condition, source))
            .unwrap_or_default(),
        body: loop_body(node)
            .map(|body| collect_body_elements(body, source))
            .unwrap_or_default(),
        update_calls: Vec::new(),
        locals: Vec::new(),
    }
}

fn parse_do_statement(node: Node<'_>, source: &str) -> LoopSyntax {
    let condition = node.child_by_field_name("condition");

    LoopSyntax {
        kind: LoopKind::DoWhile,
        source: condition
            .map(|condition| condition_source(condition, source))
            .unwrap_or_else(|| header_source(node, source)),
        condition_calls: condition
            .map(|condition| collect_body_elements(condition, source))
            .unwrap_or_default(),
        body: loop_body(node)
            .map(|body| collect_body_elements(body, source))
            .unwrap_or_default(),
        update_calls: Vec::new(),
        locals: Vec::new(),
    }
}

fn enhanced_for_local(node: Node<'_>, source: &str) -> Option<LoopLocal> {
    let name = node_text(node.child_by_field_name("name"), source);
    let ty = node
        .child_by_field_name("type")
        .map(|ty| text(ty, source))
        .or_else(|| enhanced_for_type_from_header(&header_source(node, source)));

    match (name, ty) {
        (Some(name), Some(ty)) => Some(LoopLocal {
            name,
            ty: parse_type_ref(&ty),
        }),
        _ => enhanced_for_local_from_header(&header_source(node, source)),
    }
}

fn enhanced_for_type_from_header(header: &str) -> Option<String> {
    header
        .split(':')
        .next()
        .and_then(|declaration| significant_tokens(declaration).iter().rev().nth(1).cloned())
}

fn enhanced_for_local_from_header(header: &str) -> Option<LoopLocal> {
    let declaration = header.split(':').next()?;
    let tokens = significant_tokens(declaration);
    let name = tokens.last()?.clone();
    let ty = tokens.iter().rev().nth(1)?.clone();
    Some(LoopLocal {
        name,
        ty: parse_type_ref(&ty),
    })
}

fn loop_body(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("body")
}

fn header_source(node: Node<'_>, source: &str) -> String {
    let value = text(node, source);
    let Some(start) = value.find('(') else {
        return value.lines().next().unwrap_or("").trim().to_string();
    };
    let mut depth = 0usize;
    for (idx, ch) in value[start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return value[start + 1..start + idx].trim().to_string();
                }
            }
            _ => {}
        }
    }
    value[start + 1..].trim().to_string()
}

fn condition_source(node: Node<'_>, source: &str) -> String {
    let value = text(node, source);
    let trimmed = value.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn arm_terminates(node: Node<'_>) -> bool {
    let statement = if node.kind() == "block" {
        named_children(node)
            .into_iter()
            .find(|child| child.kind() != "line_comment" && child.kind() != "block_comment")
    } else {
        Some(node)
    };

    statement
        .is_some_and(|statement| matches!(statement.kind(), "throw_statement" | "return_statement"))
}

/// Parse a Java method invocation into a call site.
pub fn parse_method_invocation(node: Node<'_>, source: &str) -> Option<CallSite> {
    let method_name = node_text(node.child_by_field_name("name"), source)?;
    let receiver = node
        .child_by_field_name("object")
        .or_else(|| node.child_by_field_name("scope"))
        .map(|receiver| receiver_kind(receiver, source))
        .unwrap_or(ReceiverKind::This);
    let lambdas = node
        .child_by_field_name("arguments")
        .map(|arguments| lambda_arguments(arguments, source))
        .unwrap_or_default();
    let arity = node
        .child_by_field_name("arguments")
        .map(|arguments| count_args(&text(arguments, source)))
        .unwrap_or(0);

    Some(CallSite {
        receiver,
        method_name,
        arity,
        lambdas,
        line: line(node),
    })
}

/// Parse an object creation expression into a constructor call site.
pub fn parse_constructor_invocation(node: Node<'_>, source: &str) -> Option<CallSite> {
    let ty = node
        .child_by_field_name("type")
        .map(|node| text(node, source))
        .or_else(|| constructor_type_from_text(&text(node, source)))?;
    let arity = node
        .child_by_field_name("arguments")
        .map(|arguments| count_args(&text(arguments, source)))
        .unwrap_or(0);

    Some(CallSite {
        receiver: ReceiverKind::Constructor(strip_generics(&ty).to_string()),
        method_name: "<init>".to_string(),
        arity,
        lambdas: Vec::new(),
        line: line(node),
    })
}

/// Collect local variable declarations visible within a method body.
pub fn collect_locals(node: Node<'_>, source: &str, locals: &mut HashMap<String, TypeRef>) {
    if node.kind() == "local_variable_declaration"
        && let Some((name, ty)) = parse_local_declaration(&text(node, source))
    {
        locals.insert(name, ty);
    }

    for child in named_children(node) {
        collect_locals(child, source, locals);
    }
}

/// Parse a local variable declaration into its name and type.
pub fn parse_local_declaration(declaration: &str) -> Option<(String, TypeRef)> {
    let before_assignment = declaration.split('=').next().unwrap_or(declaration);
    let before_semicolon = before_assignment.trim_end_matches(';').trim();
    let tokens = significant_tokens(before_semicolon);
    let name = tokens.last()?.clone();
    let ty = tokens.iter().rev().nth(1)?.clone();
    Some((name, parse_type_ref(&ty)))
}

fn receiver_kind(node: Node<'_>, source: &str) -> ReceiverKind {
    if node.kind() == "method_invocation"
        && let Some(call) = parse_method_invocation(node, source)
    {
        return ReceiverKind::Chain(Box::new(call));
    }

    let receiver = text(node, source);
    let trimmed = receiver.trim();
    // Complex receiver expressions are kept unresolved until a fuller type pass exists.
    if trimmed.contains('(') {
        return ReceiverKind::TypeName("Unknown".to_string());
    }

    if trimmed.contains('.') {
        let first = trimmed.split('.').next().unwrap_or(trimmed);
        // Uppercase leading segments are treated as static/class receivers.
        if starts_uppercase(first) {
            return ReceiverKind::TypeName(first.to_string());
        }
    }

    if starts_uppercase(trimmed) {
        ReceiverKind::TypeName(trimmed.to_string())
    } else {
        ReceiverKind::Local(trimmed.to_string())
    }
}

/// Extract lambda expressions and method references from call arguments.
pub fn lambda_arguments(arguments: Node<'_>, source: &str) -> Vec<LambdaSyntax> {
    let mut lambdas = Vec::new();
    collect_lambda_arguments(arguments, source, &mut lambdas);
    lambdas
}

fn collect_lambda_arguments(node: Node<'_>, source: &str, lambdas: &mut Vec<LambdaSyntax>) {
    match node.kind() {
        "lambda_expression" => {
            let body = node
                .child_by_field_name("body")
                .map(|body| collect_body_elements(body, source))
                .unwrap_or_default();
            lambdas.push(LambdaSyntax {
                kind: LambdaKind::Lambda,
                source: text(node, source),
                body,
            });
            return;
        }
        "method_reference" => {
            lambdas.push(LambdaSyntax {
                kind: LambdaKind::MethodRef,
                source: text(node, source),
                body: Vec::new(),
            });
            return;
        }
        _ => {}
    }

    for child in named_children(node) {
        collect_lambda_arguments(child, source, lambdas);
    }
}

/// Parse comma-separated method parameters.
pub fn parse_params(params: &str) -> Vec<ParamInfo> {
    split_top_level(params, ',')
        .into_iter()
        .filter_map(|param| {
            let tokens = significant_tokens(&param);
            let name = tokens.last()?.clone();
            let ty = tokens.iter().rev().nth(1)?.clone();
            Some(ParamInfo {
                name,
                ty,
                source: param_source(&param),
            })
        })
        .collect()
}

fn constructor_type_from_text(value: &str) -> Option<String> {
    let after_new = value.split("new ").nth(1)?.trim();
    Some(after_new.split('(').next()?.trim().to_string())
}
