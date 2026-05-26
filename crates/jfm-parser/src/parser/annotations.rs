//! Small Spring annotation extractors used while indexing Java source.

use crate::model::{HttpVerb, ParamSource};

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

fn annotation_name(annotation: &str) -> &str {
    let trimmed = annotation.trim().trim_start_matches('@');
    trimmed
        .split(|ch: char| ch == '(' || ch.is_whitespace())
        .next()
        .unwrap_or(trimmed)
}

fn first_string_literal(value: &str) -> Option<String> {
    let start = value.find('"')? + 1;
    let rest = &value[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
