use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use crate::model::{
    BodyElement, BranchKind, BranchSyntax, CallSite, ClassInfo, ClassKind, Endpoint, FieldInfo,
    Fqn, LambdaKind, LambdaSyntax, LoopKind, LoopLocal, LoopSyntax, MethodInfo, ParamInfo,
    ReceiverKind, TypeRef,
};

use super::annotations;

pub struct ParsedFile {
    pub classes: Vec<ClassInfo>,
    pub endpoints: Vec<Endpoint>,
}

pub fn parse_file(path: &Path, source: &str, root: Node<'_>) -> ParsedFile {
    let package = extract_package(root, source);
    let imports = extract_imports(root, source);
    let mut classes = Vec::new();
    collect_classes(root, source, path, &package, &imports, &mut classes);

    let mut endpoints = Vec::new();
    for class in &classes {
        let class_path = annotations::request_mapping_path(&class.annotations);
        for method in &class.methods {
            if let Some((verb, method_path)) = annotations::http_mapping(&method.annotations) {
                endpoints.push(Endpoint {
                    verb,
                    path: join_paths(&class_path, &method_path),
                    handler_fqn: method.fqn.clone(),
                    file: method.file.clone(),
                    line: method.line,
                });
            }
        }
    }

    ParsedFile { classes, endpoints }
}

fn extract_package(root: Node<'_>, source: &str) -> String {
    for child in named_children(root) {
        if child.kind() == "package_declaration" {
            return child
                .utf8_text(source.as_bytes())
                .unwrap_or_default()
                .trim()
                .trim_start_matches("package")
                .trim()
                .trim_end_matches(';')
                .trim()
                .to_string();
        }
    }
    String::new()
}

fn extract_imports(root: Node<'_>, source: &str) -> HashMap<String, String> {
    let mut imports = HashMap::new();
    for child in named_children(root) {
        if child.kind() != "import_declaration" {
            continue;
        }
        let import = child
            .utf8_text(source.as_bytes())
            .unwrap_or_default()
            .trim()
            .trim_start_matches("import")
            .trim()
            .trim_start_matches("static")
            .trim()
            .trim_end_matches(';')
            .trim();
        if import.ends_with(".*") {
            continue;
        }
        if let Some(simple) = import.rsplit('.').next() {
            imports.insert(simple.to_string(), import.to_string());
        }
    }
    imports
}

fn collect_classes(
    node: Node<'_>,
    source: &str,
    path: &Path,
    package: &str,
    imports: &HashMap<String, String>,
    classes: &mut Vec<ClassInfo>,
) {
    match node.kind() {
        "class_declaration" | "interface_declaration" => {
            classes.push(parse_class(node, source, path, package, imports));
        }
        _ => {
            for child in named_children(node) {
                collect_classes(child, source, path, package, imports, classes);
            }
        }
    }
}

fn parse_class(
    node: Node<'_>,
    source: &str,
    path: &Path,
    package: &str,
    imports: &HashMap<String, String>,
) -> ClassInfo {
    let simple_name = node_text(node.child_by_field_name("name"), source)
        .unwrap_or_else(|| declaration_name(node, source).unwrap_or_else(|| "Unknown".to_string()));
    let fqn = if package.is_empty() {
        Fqn(simple_name.clone())
    } else {
        Fqn(format!("{package}.{simple_name}"))
    };
    let kind = if node.kind() == "interface_declaration" {
        ClassKind::Interface
    } else {
        ClassKind::Class
    };
    let text = text(node, source);
    let annotations = annotations_from_declaration(&text);
    let extends = parse_extends(&text, &kind);
    let implements = parse_implements(&text);

    let mut fields = Vec::new();
    let mut methods = Vec::new();
    if let Some(body) = class_body(node) {
        for child in named_children(body) {
            match child.kind() {
                "field_declaration" => fields.push(parse_field(child, source)),
                "method_declaration" => {
                    methods.push(parse_method(child, source, path, &fqn, &simple_name, false))
                }
                "constructor_declaration" => {
                    methods.push(parse_method(child, source, path, &fqn, &simple_name, true))
                }
                _ => {}
            }
        }
    }

    ClassInfo {
        fqn,
        simple_name,
        package: package.to_string(),
        imports: imports.clone(),
        kind,
        annotations,
        extends,
        implements,
        fields,
        methods,
        file: path.to_path_buf(),
        line: line(node),
    }
}

fn parse_field(node: Node<'_>, source: &str) -> FieldInfo {
    let declaration = text(node, source);
    let before_assignment = declaration.split('=').next().unwrap_or(&declaration);
    let before_semicolon = before_assignment.trim_end_matches(';').trim();
    let tokens = significant_tokens(before_semicolon);
    let name = tokens.last().cloned().unwrap_or_default();
    let ty = tokens
        .iter()
        .rev()
        .nth(1)
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string());

    FieldInfo {
        name,
        ty: parse_type_ref(&ty),
        annotations: annotations_from_declaration(&declaration),
    }
}

fn parse_method(
    node: Node<'_>,
    source: &str,
    path: &Path,
    class_fqn: &Fqn,
    class_name: &str,
    is_constructor: bool,
) -> MethodInfo {
    let declaration = text(node, source);
    let signature = signature_text(&declaration);
    let annotations = annotations_from_declaration(&declaration);
    let params = parse_params(params_text(&signature).as_deref().unwrap_or_default());
    let name = if is_constructor {
        "<init>".to_string()
    } else {
        method_name(&signature).unwrap_or_else(|| "unknown".to_string())
    };
    let return_type = if is_constructor {
        parse_type_ref(class_name)
    } else {
        parse_type_ref(&return_type(&signature).unwrap_or_else(|| "void".to_string()))
    };
    let param_types = params
        .iter()
        .map(|param| param.ty.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let fqn = Fqn(format!("{}#{}({})", class_fqn.0, name, param_types));
    let body_node = node.child_by_field_name("body");
    let body = body_node
        .map(|body| collect_body_elements(body, source))
        .unwrap_or_default();
    let locals = body_node
        .map(|body| {
            let mut locals = HashMap::new();
            collect_locals(body, source, &mut locals);
            locals
        })
        .unwrap_or_default();

    MethodInfo {
        fqn,
        name,
        params,
        return_type,
        annotations,
        body,
        locals,
        file: PathBuf::from(path),
        line: line(node),
    }
}

fn collect_body_elements(node: Node<'_>, source: &str) -> Vec<BodyElement> {
    let mut elements = Vec::new();
    collect_body_elements_into(node, source, &mut elements);
    elements
}

fn collect_body_elements_into(node: Node<'_>, source: &str, elements: &mut Vec<BodyElement>) {
    if node.kind() == "lambda_expression" {
        return;
    }

    if node.kind() == "if_statement" {
        elements.push(BodyElement::Branch(parse_if_statement(node, source)));
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

fn parse_if_statement(node: Node<'_>, source: &str) -> BranchSyntax {
    let condition = node.child_by_field_name("condition");
    let consequence = node.child_by_field_name("consequence");
    let alternative = node.child_by_field_name("alternative");

    BranchSyntax {
        kind: BranchKind::If,
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

fn parse_loop_statement(node: Node<'_>, source: &str) -> LoopSyntax {
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

fn parse_method_invocation(node: Node<'_>, source: &str) -> Option<CallSite> {
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

fn parse_constructor_invocation(node: Node<'_>, source: &str) -> Option<CallSite> {
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

fn collect_locals(node: Node<'_>, source: &str, locals: &mut HashMap<String, TypeRef>) {
    if node.kind() == "local_variable_declaration"
        && let Some((name, ty)) = parse_local_declaration(&text(node, source))
    {
        locals.insert(name, ty);
    }

    for child in named_children(node) {
        collect_locals(child, source, locals);
    }
}

fn parse_local_declaration(declaration: &str) -> Option<(String, TypeRef)> {
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
    if trimmed.contains('(') {
        return ReceiverKind::TypeName("Unknown".to_string());
    }

    if trimmed.contains('.') {
        let first = trimmed.split('.').next().unwrap_or(trimmed);
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

fn lambda_arguments(arguments: Node<'_>, source: &str) -> Vec<LambdaSyntax> {
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

fn parse_params(params: &str) -> Vec<ParamInfo> {
    split_top_level(params, ',')
        .into_iter()
        .filter_map(|param| {
            let tokens = significant_tokens(&param);
            let name = tokens.last()?.clone();
            let ty = tokens.iter().rev().nth(1)?.clone();
            Some(ParamInfo {
                name,
                ty,
                source: annotations::param_source(&param),
            })
        })
        .collect()
}

fn parse_type_ref(raw: &str) -> TypeRef {
    let raw = raw.trim().trim_end_matches("...").to_string();
    let generics = raw
        .find('<')
        .and_then(|start| raw.rfind('>').map(|end| (start, end)))
        .map(|(start, end)| split_top_level(&raw[start + 1..end], ','))
        .unwrap_or_default();

    TypeRef { raw, generics }
}

fn parse_extends(text: &str, kind: &ClassKind) -> Vec<TypeRef> {
    let header = header_text(text);
    let Some(after_extends) = header.split(" extends ").nth(1) else {
        return Vec::new();
    };
    let end_marker = if *kind == ClassKind::Class {
        " implements "
    } else {
        "{"
    };
    let extends = after_extends
        .split(end_marker)
        .next()
        .unwrap_or(after_extends);
    split_top_level(extends, ',')
        .into_iter()
        .map(|ty| parse_type_ref(&ty))
        .collect()
}

fn parse_implements(text: &str) -> Vec<TypeRef> {
    let header = header_text(text);
    let Some(after_implements) = header.split(" implements ").nth(1) else {
        return Vec::new();
    };
    split_top_level(after_implements, ',')
        .into_iter()
        .map(|ty| parse_type_ref(&ty))
        .collect()
}

fn significant_tokens(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .filter(|token| {
            !token.starts_with('@')
                && !matches!(
                    *token,
                    "public"
                        | "private"
                        | "protected"
                        | "final"
                        | "static"
                        | "abstract"
                        | "default"
                        | "transient"
                        | "volatile"
                )
        })
        .map(|token| token.trim_matches(',').to_string())
        .collect()
}

fn annotations_from_declaration(declaration: &str) -> Vec<String> {
    declaration
        .lines()
        .map(str::trim)
        .take_while(|line| line.starts_with('@') || line.is_empty())
        .filter(|line| line.starts_with('@'))
        .map(ToString::to_string)
        .collect()
}

fn method_name(signature: &str) -> Option<String> {
    let before_params = signature.split('(').next()?.trim();
    significant_tokens(before_params).last().cloned()
}

fn return_type(signature: &str) -> Option<String> {
    let before_params = signature.split('(').next()?.trim();
    significant_tokens(before_params)
        .iter()
        .rev()
        .nth(1)
        .cloned()
}

fn params_text(signature: &str) -> Option<String> {
    let start = signature.find('(')? + 1;
    let end = signature.rfind(')')?;
    Some(signature[start..end].to_string())
}

fn signature_text(declaration: &str) -> String {
    let mut signature = String::new();
    let mut started = false;

    for line in declaration.lines().map(str::trim) {
        if !started && (line.is_empty() || line.starts_with('@')) {
            continue;
        }
        started = true;
        if !signature.is_empty() {
            signature.push(' ');
        }
        signature.push_str(line);
        if line.contains('{') || line.ends_with(';') {
            break;
        }
    }

    let end = signature
        .find('{')
        .or_else(|| signature.find(';'))
        .unwrap_or(signature.len());
    signature[..end].trim().to_string()
}

fn declaration_name(node: Node<'_>, source: &str) -> Option<String> {
    let declaration = text(node, source);
    let keyword = if node.kind() == "interface_declaration" {
        "interface"
    } else {
        "class"
    };
    let after = declaration.split(keyword).nth(1)?.trim();
    after.split_whitespace().next().map(ToString::to_string)
}

fn class_body(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("body").or_else(|| {
        named_children(node)
            .into_iter()
            .find(|child| child.kind().ends_with("_body"))
    })
}

fn constructor_type_from_text(value: &str) -> Option<String> {
    let after_new = value.split("new ").nth(1)?.trim();
    Some(after_new.split('(').next()?.trim().to_string())
}

fn count_args(arguments: &str) -> usize {
    let trimmed = arguments
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    if trimmed.is_empty() {
        0
    } else {
        split_top_level(trimmed, ',').len()
    }
}

fn split_top_level(value: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth_angle = 0usize;
    let mut depth_paren = 0usize;
    let mut depth_brace = 0usize;
    let mut start = 0usize;

    for (idx, ch) in value.char_indices() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle = depth_angle.saturating_sub(1),
            '(' => depth_paren += 1,
            ')' => depth_paren = depth_paren.saturating_sub(1),
            '{' => depth_brace += 1,
            '}' => depth_brace = depth_brace.saturating_sub(1),
            ch if ch == delimiter && depth_angle == 0 && depth_paren == 0 && depth_brace == 0 => {
                push_part(&mut parts, &value[start..idx]);
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    push_part(&mut parts, &value[start..]);
    parts
}

fn push_part(parts: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        parts.push(value.to_string());
    }
}

fn strip_generics(value: &str) -> &str {
    value.split('<').next().unwrap_or(value).trim()
}

fn header_text(text: &str) -> &str {
    text.split('{').next().unwrap_or(text)
}

fn join_paths(base: &str, child: &str) -> String {
    let base = normalize_path(base);
    let child = normalize_path(child);
    match (base.as_str(), child.as_str()) {
        ("", "") => "/".to_string(),
        ("", child) => child.to_string(),
        (base, "") => base.to_string(),
        (base, child) => format!(
            "{}/{}",
            base.trim_end_matches('/'),
            child.trim_start_matches('/')
        ),
    }
}

fn normalize_path(path: &str) -> String {
    if path.is_empty() {
        String::new()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn starts_uppercase(value: &str) -> bool {
    value.chars().next().is_some_and(char::is_uppercase)
}

fn text(node: Node<'_>, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or_default()
        .to_string()
}

fn node_text(node: Option<Node<'_>>, source: &str) -> Option<String> {
    node.map(|node| text(node, source))
}

fn line(node: Node<'_>) -> u32 {
    (node.start_position().row + 1)
        .try_into()
        .unwrap_or(u32::MAX)
}

fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::model::{BodyElement, LoopKind, MethodInfo};

    use super::parse_file;

    #[test]
    fn parses_if_without_else_and_marks_throwing_then_arm() {
        let methods = parse_methods(
            r#"
            class Demo {
                void guard(User user) {
                    if (user == null) {
                        throw new IllegalArgumentException("missing");
                    }
                    save(user);
                }
            }
            "#,
        );
        let method = method(&methods, "guard");

        assert_eq!(method.body.len(), 2);
        let BodyElement::Branch(branch) = &method.body[0] else {
            panic!("expected first body element to be a branch");
        };
        assert_eq!(branch.condition_src, "user == null");
        assert!(branch.then_terminates);
        assert!(branch.else_arm.is_none());
        assert_eq!(
            call_name(&branch.then_arm[0]),
            "IllegalArgumentException#<init>"
        );
        assert_eq!(call_name(&method.body[1]), "save");
    }

    #[test]
    fn parses_if_else_with_both_arms_populated() {
        let methods = parse_methods(
            r#"
            class Demo {
                void choose() {
                    if (ready()) {
                        yes();
                    } else {
                        no();
                    }
                }
            }
            "#,
        );
        let method = method(&methods, "choose");

        let BodyElement::Branch(branch) = &method.body[0] else {
            panic!("expected branch");
        };
        assert_eq!(call_name(&branch.condition_calls[0]), "ready");
        assert_eq!(call_name(&branch.then_arm[0]), "yes");
        assert_eq!(call_name(&branch.else_arm.as_ref().unwrap()[0]), "no");
    }

    #[test]
    fn preserves_nested_if_structure_inside_arm() {
        let methods = parse_methods(
            r#"
            class Demo {
                void nested() {
                    if (outer()) {
                        if (inner()) {
                            work();
                        }
                    }
                }
            }
            "#,
        );
        let method = method(&methods, "nested");

        let BodyElement::Branch(outer) = &method.body[0] else {
            panic!("expected outer branch");
        };
        let BodyElement::Branch(inner) = &outer.then_arm[0] else {
            panic!("expected nested branch");
        };
        assert_eq!(call_name(&outer.condition_calls[0]), "outer");
        assert_eq!(call_name(&inner.condition_calls[0]), "inner");
        assert_eq!(call_name(&inner.then_arm[0]), "work");
    }

    #[test]
    fn separates_calls_inside_condition_from_then_arm() {
        let methods = parse_methods(
            r#"
            class Demo {
                void conditionCalls(User user) {
                    if (Boolean.TRUE.equals(user.getActive())) {
                        delete(user);
                    }
                }
            }
            "#,
        );
        let method = method(&methods, "conditionCalls");

        let BodyElement::Branch(branch) = &method.body[0] else {
            panic!("expected branch");
        };
        let condition_call_names = branch
            .condition_calls
            .iter()
            .map(call_name)
            .collect::<Vec<_>>();
        assert_eq!(
            branch.condition_src,
            "Boolean.TRUE.equals(user.getActive())"
        );
        assert_eq!(condition_call_names, vec!["getActive", "equals"]);
        assert_eq!(call_name(&branch.then_arm[0]), "delete");
    }

    #[test]
    fn skips_lambda_body_inside_condition() {
        let methods = parse_methods(
            r#"
            import java.util.List;

            class Demo {
                void lambdaCondition(List<User> users) {
                    if (users.stream().anyMatch(user -> check(user))) {
                        found();
                    }
                }
            }
            "#,
        );
        let method = method(&methods, "lambdaCondition");

        let BodyElement::Branch(branch) = &method.body[0] else {
            panic!("expected branch");
        };
        let condition_call_names = branch
            .condition_calls
            .iter()
            .map(call_name)
            .collect::<Vec<_>>();
        assert!(condition_call_names.contains(&"stream"));
        assert!(condition_call_names.contains(&"anyMatch"));
        assert!(!condition_call_names.contains(&"check"));
    }

    #[test]
    fn parses_while_loop_with_condition_and_body() {
        let methods = parse_methods(
            r#"
            class Demo {
                void poll() {
                    while (ready()) {
                        work();
                    }
                }
            }
            "#,
        );
        let method = method(&methods, "poll");

        let BodyElement::Loop(loop_node) = &method.body[0] else {
            panic!("expected loop");
        };
        assert_eq!(loop_node.kind, LoopKind::While);
        assert_eq!(loop_node.source, "ready()");
        assert_eq!(call_name(&loop_node.condition_calls[0]), "ready");
        assert_eq!(call_name(&loop_node.body[0]), "work");
    }

    #[test]
    fn parses_do_while_loop_with_body_before_condition() {
        let methods = parse_methods(
            r#"
            class Demo {
                void retry() {
                    do {
                        work();
                    } while (again());
                }
            }
            "#,
        );
        let method = method(&methods, "retry");

        let BodyElement::Loop(loop_node) = &method.body[0] else {
            panic!("expected loop");
        };
        assert_eq!(loop_node.kind, LoopKind::DoWhile);
        assert_eq!(loop_node.source, "again()");
        assert_eq!(call_name(&loop_node.body[0]), "work");
        assert_eq!(call_name(&loop_node.condition_calls[0]), "again");
    }

    #[test]
    fn parses_classic_for_loop_header_body_and_update_calls() {
        let methods = parse_methods(
            r#"
            class Demo {
                void count() {
                    for (int i = start(); i < limit(); i = next(i)) {
                        work();
                    }
                }
            }
            "#,
        );
        let method = method(&methods, "count");

        let BodyElement::Loop(loop_node) = &method.body[0] else {
            panic!("expected loop");
        };
        let condition_call_names = loop_node
            .condition_calls
            .iter()
            .map(call_name)
            .collect::<Vec<_>>();
        assert_eq!(loop_node.kind, LoopKind::For);
        assert!(condition_call_names.contains(&"start"));
        assert!(condition_call_names.contains(&"limit"));
        assert_eq!(call_name(&loop_node.body[0]), "work");
        assert_eq!(call_name(&loop_node.update_calls[0]), "next");
    }

    #[test]
    fn parses_enhanced_for_loop_with_loop_local() {
        let methods = parse_methods(
            r#"
            import java.util.List;

            class Demo {
                void each(List<User> users) {
                    for (User user : users) {
                        user.getEmail();
                    }
                }
            }
            "#,
        );
        let method = method(&methods, "each");

        let BodyElement::Loop(loop_node) = &method.body[0] else {
            panic!("expected loop");
        };
        assert_eq!(loop_node.kind, LoopKind::EnhancedFor);
        assert_eq!(loop_node.source, "User user : users");
        assert_eq!(loop_node.locals[0].name, "user");
        assert_eq!(loop_node.locals[0].ty.raw, "User");
        assert_eq!(call_name(&loop_node.body[0]), "getEmail");
    }

    #[test]
    fn preserves_nested_branch_inside_loop() {
        let methods = parse_methods(
            r#"
            class Demo {
                void guarded() {
                    while (ready()) {
                        if (allowed()) {
                            work();
                        }
                    }
                }
            }
            "#,
        );
        let method = method(&methods, "guarded");

        let BodyElement::Loop(loop_node) = &method.body[0] else {
            panic!("expected loop");
        };
        let BodyElement::Branch(branch) = &loop_node.body[0] else {
            panic!("expected branch inside loop");
        };
        assert_eq!(call_name(&branch.condition_calls[0]), "allowed");
        assert_eq!(call_name(&branch.then_arm[0]), "work");
    }

    #[test]
    fn preserves_loop_inside_branch_arm() {
        let methods = parse_methods(
            r#"
            class Demo {
                void branchLoop() {
                    if (enabled()) {
                        while (ready()) {
                            work();
                        }
                    }
                }
            }
            "#,
        );
        let method = method(&methods, "branchLoop");

        let BodyElement::Branch(branch) = &method.body[0] else {
            panic!("expected branch");
        };
        let BodyElement::Loop(loop_node) = &branch.then_arm[0] else {
            panic!("expected loop inside branch");
        };
        assert_eq!(loop_node.kind, LoopKind::While);
        assert_eq!(call_name(&loop_node.body[0]), "work");
    }

    #[test]
    #[ignore]
    fn parses_switch_statement_branches() {
        unimplemented!("switch_statement parsing is planned for PR #6");
    }

    #[test]
    #[ignore]
    fn parses_switch_expression_branches() {
        unimplemented!("switch_expression parsing is planned for PR #6");
    }

    #[test]
    #[ignore]
    fn parses_ternary_expression_branches() {
        unimplemented!("ternary_expression parsing is planned for PR #6");
    }

    fn parse_methods(source: &str) -> Vec<MethodInfo> {
        let mut parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let parsed = parse_file(Path::new("Demo.java"), source, tree.root_node());
        parsed.classes.into_iter().next().unwrap().methods
    }

    fn method<'a>(methods: &'a [MethodInfo], name: &str) -> &'a MethodInfo {
        methods.iter().find(|method| method.name == name).unwrap()
    }

    fn call_name(element: &BodyElement) -> &str {
        match element {
            BodyElement::Call(call) if call.method_name == "<init>" => match &call.receiver {
                crate::model::ReceiverKind::Constructor(ty)
                    if ty.ends_with("IllegalArgumentException") =>
                {
                    "IllegalArgumentException#<init>"
                }
                _ => "<init>",
            },
            BodyElement::Call(call) => &call.method_name,
            BodyElement::Branch(_) => "branch",
            BodyElement::Loop(_) => "loop",
        }
    }
}
