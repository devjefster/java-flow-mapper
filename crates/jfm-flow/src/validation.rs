//! Bean Validation metadata enrichment for endpoint inputs.

use crate::model::{
    ClassInfo, ClassKind, Fqn, ParamInfo, ProjectIndex, ValidationConstraint, ValidationField,
};

/// Attach DTO field constraints to controller inputs reached through `@Valid`.
pub(super) fn enrich_inputs(
    index: &ProjectIndex,
    owner: &ClassInfo,
    inputs: &[ParamInfo],
) -> Vec<ParamInfo> {
    inputs
        .iter()
        .map(|input| {
            let mut input = input.clone();
            if has_annotation(&input.annotations, "Valid") {
                input.validation = validation_fields(index, owner, &input.ty);
            }
            input
        })
        .collect()
}

fn validation_fields(index: &ProjectIndex, owner: &ClassInfo, ty: &str) -> Vec<ValidationField> {
    let Some(dto) = resolve_class(index, owner, ty) else {
        return Vec::new();
    };

    dto.fields
        .iter()
        .filter_map(|field| {
            let mut constraints = field.validation.clone();
            constraints.extend(custom_constraints(index, owner, &field.annotations));
            (!constraints.is_empty()).then(|| ValidationField {
                field: field.name.clone(),
                ty: field.ty.raw.clone(),
                constraints,
            })
        })
        .collect()
}

fn custom_constraints(
    index: &ProjectIndex,
    owner: &ClassInfo,
    annotations: &[String],
) -> Vec<ValidationConstraint> {
    annotations
        .iter()
        .filter_map(|annotation| {
            let name = annotation_name(annotation);
            if is_builtin_validation_constraint(name) {
                return None;
            }
            let declaration = resolve_annotation(index, owner, name)?;
            let constraint = declaration
                .annotations
                .iter()
                .find(|annotation| annotation_name(annotation) == "Constraint")?;
            Some(ValidationConstraint {
                annotation: name.to_string(),
                raw: annotation.clone(),
                custom_validator: validator_from_constraint(constraint),
            })
        })
        .collect()
}

fn resolve_annotation<'a>(
    index: &'a ProjectIndex,
    owner: &ClassInfo,
    name: &str,
) -> Option<&'a ClassInfo> {
    resolve_class(index, owner, name).filter(|class| class.kind == ClassKind::Annotation)
}

fn resolve_class<'a>(
    index: &'a ProjectIndex,
    owner: &ClassInfo,
    ty: &str,
) -> Option<&'a ClassInfo> {
    let raw = strip_generics(ty).trim_end_matches("...").trim();
    if raw.contains('.') {
        return index.classes.get(&Fqn(raw.to_string()));
    }
    if let Some(imported) = owner.imports.get(raw)
        && let Some(class) = index.classes.get(&Fqn(imported.clone()))
    {
        return Some(class);
    }
    let same_package = if owner.package.is_empty() {
        raw.to_string()
    } else {
        format!("{}.{}", owner.package, raw)
    };
    if let Some(class) = index.classes.get(&Fqn(same_package)) {
        return Some(class);
    }
    index
        .by_simple_name
        .get(raw)
        .and_then(|fqns| fqns.first())
        .and_then(|fqn| index.classes.get(fqn))
}

fn has_annotation(annotations: &[String], expected: &str) -> bool {
    annotations
        .iter()
        .any(|annotation| annotation_name(annotation) == expected)
}

fn is_builtin_validation_constraint(name: &str) -> bool {
    matches!(
        name,
        "NotBlank" | "Email" | "Min" | "Size" | "Pattern" | "NotNull" | "Max"
    )
}

fn annotation_name(annotation: &str) -> &str {
    let trimmed = annotation.trim().trim_start_matches('@');
    let head = trimmed
        .split(|ch: char| ch == '(' || ch.is_whitespace())
        .next()
        .unwrap_or(trimmed);
    head.rsplit('.').next().unwrap_or(head)
}

fn validator_from_constraint(annotation: &str) -> Option<String> {
    let after = annotation.split("validatedBy").nth(1)?;
    let after_equals = after.split('=').nth(1)?.trim();
    let value = after_equals
        .trim_start_matches('{')
        .split([',', '}', ')'])
        .next()?
        .trim()
        .trim_end_matches(".class")
        .trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn strip_generics(value: &str) -> &str {
    value.split('<').next().unwrap_or(value).trim()
}
