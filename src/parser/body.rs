use std::collections::HashMap;
use tree_sitter::Node;

use crate::model::{
    BodyElement, CallSite, LambdaKind, LambdaSyntax, LoopKind, LoopLocal, ReceiverKind,
};

use super::utils::{named_children, node_text, text};

pub fn collect_body_elements(node: Node<'_>, source: &str) -> Vec<BodyElement> {
    let mut elements = Vec::new();
    collect_body_elements_into(node, source, &mut elements);
    elements
}

pub fn collect_body_elements_into(node: Node<'_>, source: &str, elements: &mut Vec<BodyElement>) {
    if node.kind() == "lambda_expression" {
        return;
    }

    if node.kind() == "if_statement" {
        elements.push(BodyElement::Branch(
            crate::parser::body::parse_if_statement(node, source),
        ));
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

pub fn parse_if_statement(node: Node<'_>, source: &str) -> crate::model::BranchSyntax {
    let condition = node.child_by_field_name("condition");
    let consequence = node.child_by_field_name("consequence");
    let alternative = node.child_by_field_name("alternative");

    crate::model::BranchSyntax {
        kind: crate::model::BranchKind::If,
        condition_src: condition
            .map(|condition| condition_source(condition, source))
            .unwrap_or_default(),
        condition_calls: condition
            .map(|condition| collect_body_elements(condition, source))
            .unwrap_or_default(),
        then_arm: consequence
            .map(|consequence| collect_body_elements(consequence, source))
            .unwrap_or_default(),
        else_arm: alternative.map(|alternative| collect_body_elements(alternative, source)),
        then_terminates: consequence.is_some_and(arm_terminates),
        else_terminates: alternative.is_some_and(arm_terminates),
    }
}

fn is_loop_statement(kind: &str) -> bool {
    matches!(
        kind,
        "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement"
    )
}

pub fn parse_loop_statement(node: Node<'_>, source: &str) -> crate::model::LoopSyntax {
    match node.kind() {
        "for_statement" => parse_for_statement(node, source),
        "enhanced_for_statement" => parse_enhanced_for_statement(node, source),
        "while_statement" => parse_while_statement(node, source),
        "do_statement" => parse_do_statement(node, source),
        _ => crate::model::LoopSyntax {
            kind: LoopKind::For,
            source: header_source(node, source),
            condition_calls: Vec::new(),
            body: Vec::new(),
            update_calls: Vec::new(),
            locals: Vec::new(),
        },
    }
}

fn parse_for_statement(node: Node<'_>, source: &str) -> crate::model::LoopSyntax {
    let mut condition_calls = Vec::new();
    if let Some(init) = node.child_by_field_name("init") {
        condition_calls.extend(collect_body_elements(init, source));
    }
    if let Some(condition) = node.child_by_field_name("condition") {
        condition_calls.extend(collect_body_elements(condition, source));
    }

    crate::model::LoopSyntax {
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

fn parse_enhanced_for_statement(node: Node<'_>, source: &str) -> crate::model::LoopSyntax {
    let iterable = node
        .child_by_field_name("value")
        .or_else(|| node.child_by_field_name("iterable"));

    crate::model::LoopSyntax {
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

fn parse_while_statement(node: Node<'_>, source: &str) -> crate::model::LoopSyntax {
    let condition = node.child_by_field_name("condition");

    crate::model::LoopSyntax {
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

fn parse_do_statement(node: Node<'_>, source: &str) -> crate::model::LoopSyntax {
    let condition = node.child_by_field_name("condition");

    crate::model::LoopSyntax {
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
            ty: crate::parser::class::parse_type_ref(&ty),
        }),
        _ => enhanced_for_local_from_header(&header_source(node, source)),
    }
}

fn enhanced_for_type_from_header(header: &str) -> Option<String> {
    header.split(':').next().and_then(|declaration| {
        crate::parser::utils::significant_tokens(declaration)
            .iter()
            .rev()
            .nth(1)
            .cloned()
    })
}

fn enhanced_for_local_from_header(header: &str) -> Option<LoopLocal> {
    let declaration = header.split(':').next()?;
    let tokens = crate::parser::utils::significant_tokens(declaration);
    let name = tokens.last()?.clone();
    let ty = tokens.iter().rev().nth(1)?.clone();
    Some(LoopLocal {
        name,
        ty: crate::parser::class::parse_type_ref(&ty),
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
        .map(|arguments| crate::parser::utils::count_args(&text(arguments, source)))
        .unwrap_or(0);

    Some(CallSite {
        receiver,
        method_name,
        arity,
        lambdas,
        line: crate::parser::utils::line(node),
    })
}

pub fn parse_constructor_invocation(node: Node<'_>, source: &str) -> Option<CallSite> {
    let ty = node
        .child_by_field_name("type")
        .map(|node| text(node, source))
        .or_else(|| constructor_type_from_text(&text(node, source)))?;
    let arity = node
        .child_by_field_name("arguments")
        .map(|arguments| crate::parser::utils::count_args(&text(arguments, source)))
        .unwrap_or(0);

    Some(CallSite {
        receiver: ReceiverKind::Constructor(crate::parser::utils::strip_generics(&ty).to_string()),
        method_name: "<init>".to_string(),
        arity,
        lambdas: Vec::new(),
        line: crate::parser::utils::line(node),
    })
}

pub fn collect_locals(
    node: Node<'_>,
    source: &str,
    locals: &mut HashMap<String, crate::model::TypeRef>,
) {
    if node.kind() == "local_variable_declaration"
        && let Some((name, ty)) = parse_local_declaration(&text(node, source))
    {
        locals.insert(name, ty);
    }

    for child in named_children(node) {
        collect_locals(child, source, locals);
    }
}

pub fn parse_local_declaration(declaration: &str) -> Option<(String, crate::model::TypeRef)> {
    let before_assignment = declaration.split('=').next().unwrap_or(declaration);
    let before_semicolon = before_assignment.trim_end_matches(';').trim();
    let tokens = crate::parser::utils::significant_tokens(before_semicolon);
    let name = tokens.last()?.clone();
    let ty = tokens.iter().rev().nth(1)?.clone();
    Some((name, crate::parser::class::parse_type_ref(&ty)))
}

fn receiver_kind(node: Node<'_>, source: &str) -> ReceiverKind {
    if node.kind() == "method_invocation"
        && let Some(call) = parse_method_invocation(node, source)
    {
        return ReceiverKind::Chain(Box::new(call));
    }

    let receiver = text(node, source);
    let trimmed = receiver.trim();
    if trimmed.contains('(') {
        return ReceiverKind::TypeName("Unknown".to_string());
    }

    if trimmed.contains('.') {
        let first = trimmed.split('.').next().unwrap_or(trimmed);
        if crate::parser::utils::starts_uppercase(first) {
            return ReceiverKind::TypeName(first.to_string());
        }
    }

    if crate::parser::utils::starts_uppercase(trimmed) {
        ReceiverKind::TypeName(trimmed.to_string())
    } else {
        ReceiverKind::Local(trimmed.to_string())
    }
}

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
                .map(|body| crate::parser::body::collect_body_elements(body, source))
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

pub fn parse_params(params: &str) -> Vec<crate::model::ParamInfo> {
    crate::parser::utils::split_top_level(params, ',')
        .into_iter()
        .filter_map(|param| {
            let tokens = crate::parser::utils::significant_tokens(&param);
            let name = tokens.last()?.clone();
            let ty = tokens.iter().rev().nth(1)?.clone();
            Some(crate::model::ParamInfo {
                name,
                ty,
                source: crate::parser::annotations::param_source(&param),
            })
        })
        .collect()
}

fn constructor_type_from_text(value: &str) -> Option<String> {
    let after_new = value.split("new ").nth(1)?.trim();
    Some(after_new.split('(').next()?.trim().to_string())
}
