//! Spring Data repository recognition and inherited method synthesis.

use crate::model::{ClassInfo, ClassKind, TypeRef};
use tracing::debug;

/// Repository methods modeled as inherited Spring Data calls.
pub const JPA_INHERITED_METHODS: &[(&str, usize)] = &[
    ("save", 1),
    ("saveAll", 1),
    ("findById", 1),
    ("existsById", 1),
    ("findAll", 0),
    ("findAllById", 1),
    ("count", 0),
    ("deleteById", 1),
    ("delete", 1),
    ("deleteAllById", 1),
    ("deleteAll", 0),
    ("deleteAll", 1),
    ("flush", 0),
    ("saveAndFlush", 1),
    ("getReferenceById", 1),
];

const REPOSITORY_TYPES: &[&str] = &[
    "JpaRepository",
    "CrudRepository",
    "PagingAndSortingRepository",
    "Repository",
];

/// Return true when an interface extends a recognized Spring Data repository.
pub fn is_spring_data_repository(class: &ClassInfo) -> bool {
    if class.kind != ClassKind::Interface {
        return false;
    }

    let matched = class.extends.iter().any(|ty| {
        let simple = ty.raw.split('<').next().unwrap_or(&ty.raw).trim();
        REPOSITORY_TYPES.contains(&simple) && import_matches(class, simple)
    });

    if matched {
        debug!(repository = %class.fqn, "recognized Spring Data repository");
    }

    matched
}

/// Return true when a repository method is synthesized from Spring Data.
pub fn is_inherited_method(name: &str, arity: usize) -> bool {
    JPA_INHERITED_METHODS
        .iter()
        .any(|(method, method_arity)| *method == name && *method_arity == arity)
}

/// Infer display parameter types for a synthesized inherited repository call.
pub fn inherited_param_types(class: &ClassInfo, method_name: &str, arity: usize) -> Vec<String> {
    if arity == 0 {
        return Vec::new();
    }

    let (entity, id) = repository_generics(class);
    match method_name {
        "findById" | "existsById" | "deleteById" | "getReferenceById" => vec![id],
        "save" | "saveAndFlush" | "delete" => vec![entity],
        _ => vec!["_".to_string(); arity],
    }
}

/// Infer the return type for a synthesized inherited repository call.
pub fn inherited_return_type(
    class: &ClassInfo,
    method_name: &str,
    arity: usize,
) -> Option<TypeRef> {
    if !is_inherited_method(method_name, arity) {
        return None;
    }

    let (entity, _) = repository_generics(class);
    let raw = match method_name {
        "findById" => format!("Optional<{entity}>"),
        "findAll" => format!("List<{entity}>"),
        "save" | "saveAndFlush" | "getReferenceById" => entity,
        "existsById" => "boolean".to_string(),
        "count" => "long".to_string(),
        "deleteById" | "delete" | "deleteAllById" | "deleteAll" | "flush" => "void".to_string(),
        _ => "Object".to_string(),
    };

    Some(type_ref(&raw))
}

fn import_matches(class: &ClassInfo, simple: &str) -> bool {
    class
        .imports
        .get(simple)
        .is_some_and(|import| import.starts_with("org.springframework.data."))
}

fn repository_generics(class: &ClassInfo) -> (String, String) {
    // Spring Data repositories conventionally declare `<Entity, Id>`.
    let Some(extends) = class.extends.first() else {
        return ("_".to_string(), "_".to_string());
    };
    let entity = extends
        .generics
        .first()
        .cloned()
        .unwrap_or_else(|| "_".to_string());
    let id = extends
        .generics
        .get(1)
        .cloned()
        .unwrap_or_else(|| "_".to_string());
    (entity, id)
}

fn type_ref(raw: &str) -> TypeRef {
    let generics = raw
        .find('<')
        .and_then(|start| raw.rfind('>').map(|end| (start, end)))
        .map(|(start, end)| {
            raw[start + 1..end]
                .split(',')
                .map(|part| part.trim().to_string())
                .collect()
        })
        .unwrap_or_default();

    TypeRef {
        raw: raw.to_string(),
        generics,
    }
}
