use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Node;

use super::body::parse_params;
use super::utils::{header_text, line, named_children, node_text, text};
use crate::model::{ClassInfo, ClassKind, Fqn, TypeRef};

pub fn extract_package(root: Node<'_>, source: &str) -> String {
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

pub fn extract_imports(root: Node<'_>, source: &str) -> HashMap<String, String> {
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

pub fn collect_classes(
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

pub fn parse_class(
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

pub fn parse_field(node: Node<'_>, source: &str) -> crate::model::FieldInfo {
    let declaration = text(node, source);
    let before_assignment = declaration.split('=').next().unwrap_or(&declaration);
    let before_semicolon = before_assignment.trim_end_matches(';').trim();
    let tokens = crate::parser::utils::significant_tokens(before_semicolon);
    let name = tokens.last().cloned().unwrap_or_default();
    let ty = tokens
        .iter()
        .rev()
        .nth(1)
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string());

    crate::model::FieldInfo {
        name,
        ty: parse_type_ref(&ty),
        annotations: annotations_from_declaration(&declaration),
    }
}

pub fn parse_method(
    node: Node<'_>,
    source: &str,
    path: &Path,
    class_fqn: &Fqn,
    class_name: &str,
    is_constructor: bool,
) -> crate::model::MethodInfo {
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
        .map(|body| crate::parser::body::collect_body_elements(body, source))
        .unwrap_or_default();
    let locals = body_node
        .map(|body| {
            let mut locals = HashMap::new();
            crate::parser::body::collect_locals(body, source, &mut locals);
            locals
        })
        .unwrap_or_default();

    crate::model::MethodInfo {
        fqn,
        name,
        params,
        return_type,
        annotations,
        body,
        locals,
        file: std::path::PathBuf::from(path),
        line: line(node),
    }
}

pub fn parse_type_ref(raw: &str) -> TypeRef {
    let raw = raw.trim().trim_end_matches("...").to_string();
    let generics = raw
        .find('<')
        .and_then(|start| raw.rfind('>').map(|end| (start, end)))
        .map(|(start, end)| crate::parser::utils::split_top_level(&raw[start + 1..end], ','))
        .unwrap_or_default();

    TypeRef { raw, generics }
}

pub fn parse_extends(text: &str, kind: &ClassKind) -> Vec<TypeRef> {
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
    crate::parser::utils::split_top_level(extends, ',')
        .into_iter()
        .map(|ty| parse_type_ref(&ty))
        .collect()
}

pub fn parse_implements(text: &str) -> Vec<TypeRef> {
    let header = header_text(text);
    let Some(after_implements) = header.split(" implements ").nth(1) else {
        return Vec::new();
    };
    crate::parser::utils::split_top_level(after_implements, ',')
        .into_iter()
        .map(|ty| parse_type_ref(&ty))
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
    crate::parser::utils::significant_tokens(before_params)
        .last()
        .cloned()
}

fn return_type(signature: &str) -> Option<String> {
    let before_params = signature.split('(').next()?.trim();
    crate::parser::utils::significant_tokens(before_params)
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
