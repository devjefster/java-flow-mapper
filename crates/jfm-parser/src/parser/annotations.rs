//! Small Spring annotation extractors used while indexing Java source.

use crate::model::{HttpVerb, ParamSource, ValidationConstraint};

/// Return the class-level `@RequestMapping` path, if present.
pub fn request_mapping_path(annotations: &[String]) -> String {
    annotations
        .iter()
        .find(|annotation| annotation_name(annotation) == "RequestMapping")
        .and_then(|annotation| first_string_literal(annotation))
        .unwrap_or_default()
}

/// Return the HTTP verb and method-level path from supported mapping annotations.
pub fn http_mapping(annotations: &[String]) -> Option<(HttpVerb, String)> {
    for annotation in annotations {
        let name = annotation_name(annotation);
        let path = first_string_literal(annotation).unwrap_or_default();
        let verb = match name {
            "GetMapping" => HttpVerb::Get,
            "PostMapping" => HttpVerb::Post,
            "PutMapping" => HttpVerb::Put,
            "DeleteMapping" => HttpVerb::Delete,
            "PatchMapping" => HttpVerb::Patch,
            "RequestMapping" if annotation.contains("RequestMethod.GET") => HttpVerb::Get,
            "RequestMapping" if annotation.contains("RequestMethod.POST") => HttpVerb::Post,
            "RequestMapping" if annotation.contains("RequestMethod.PUT") => HttpVerb::Put,
            "RequestMapping" if annotation.contains("RequestMethod.DELETE") => HttpVerb::Delete,
            "RequestMapping" if annotation.contains("RequestMethod.PATCH") => HttpVerb::Patch,
            _ => continue,
        };
        return Some((verb, path));
    }

    None
}

/// Classify a method parameter's Spring request binding annotation.
pub fn param_source(param: &str) -> ParamSource {
    if param.contains("@PathVariable") {
        ParamSource::Path
    } else if param.contains("@RequestParam") {
        ParamSource::Query
    } else if param.contains("@RequestBody") {
        ParamSource::Body
    } else if param.contains("@RequestHeader") {
        ParamSource::Header
    } else {
        ParamSource::Unspecified
    }
}

/// Return raw annotation strings found in Java source text.
pub fn extract_annotations(value: &str) -> Vec<String> {
    let bytes = value.as_bytes();
    let mut annotations = Vec::new();
    let mut idx = 0;

    while idx < bytes.len() {
        if bytes[idx] != b'@' {
            idx += 1;
            continue;
        }

        let start = idx;
        idx += 1;
        while idx < bytes.len() && is_annotation_name_byte(bytes[idx]) {
            idx += 1;
        }
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }

        if idx < bytes.len() && bytes[idx] == b'(' {
            idx = balanced_end(value, idx).unwrap_or(idx + 1);
        }

        annotations.push(value[start..idx].trim().to_string());
    }

    annotations
}

/// Strip annotations from declaration text before token-based parsing.
pub fn strip_annotations(value: &str) -> String {
    let mut stripped = value.to_string();
    for annotation in extract_annotations(value) {
        stripped = stripped.replace(&annotation, " ");
    }
    stripped
}

/// Return validation constraints from a list of raw annotations.
pub fn builtin_validation_constraints(annotations: &[String]) -> Vec<ValidationConstraint> {
    annotations
        .iter()
        .filter_map(|annotation| {
            let name = annotation_name(annotation).to_string();
            is_builtin_validation_constraint(&name).then(|| ValidationConstraint {
                annotation: name,
                raw: annotation.clone(),
                custom_validator: None,
            })
        })
        .collect()
}

/// Return true for Bean Validation annotations supported in v0.3.
pub fn is_builtin_validation_constraint(name: &str) -> bool {
    matches!(
        name,
        "NotBlank" | "Email" | "Min" | "Size" | "Pattern" | "NotNull" | "Max"
    )
}

pub fn annotation_name(annotation: &str) -> &str {
    let trimmed = annotation.trim().trim_start_matches('@');
    let head = trimmed
        .split(|ch: char| ch == '(' || ch.is_whitespace())
        .next()
        .unwrap_or(trimmed);
    head.rsplit('.').next().unwrap_or(head)
}

fn first_string_literal(value: &str) -> Option<String> {
    let start = value.find('"')? + 1;
    let rest = &value[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn is_annotation_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'.')
}

fn balanced_end(value: &str, open_idx: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in value[open_idx..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open_idx + idx + ch.len_utf8());
                }
            }
            _ => {}
        }
    }

    None
}
