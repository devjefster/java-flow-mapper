//! Shared string and tree-sitter helpers for Java parsing.

use tree_sitter::Node;

/// Return declaration tokens with annotations and common Java modifiers removed.
pub fn significant_tokens(value: &str) -> Vec<String> {
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

/// Split on a delimiter only when it is not nested in generics or expressions.
pub fn split_top_level(value: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth_angle = 0usize;
    let mut depth_paren = 0usize;
    let mut depth_brace = 0usize;
    let mut start = 0usize;

    // Parameter lists and generic arguments can contain nested delimiters.
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

/// Push a trimmed non-empty segment into `parts`.
pub fn push_part(parts: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        parts.push(value.to_string());
    }
}

/// Remove generic arguments from a type spelling.
pub fn strip_generics(value: &str) -> &str {
    value.split('<').next().unwrap_or(value).trim()
}

/// Return the declaration header before the body starts.
pub fn header_text(text: &str) -> &str {
    text.split('{').next().unwrap_or(text)
}

/// Join class-level and method-level Spring paths.
pub fn join_paths(base: &str, child: &str) -> String {
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

/// Normalize a Spring path to either empty or slash-prefixed.
pub fn normalize_path(path: &str) -> String {
    if path.is_empty() {
        String::new()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

/// Return true when a Java identifier begins with an uppercase character.
pub fn starts_uppercase(value: &str) -> bool {
    value.chars().next().is_some_and(char::is_uppercase)
}

/// Extract UTF-8 source text for a syntax node.
pub fn text(node: Node<'_>, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or_default()
        .to_string()
}

/// Extract source text for an optional syntax node.
pub fn node_text(node: Option<Node<'_>>, source: &str) -> Option<String> {
    node.map(|node| text(node, source))
}

/// Return a 1-based source line number.
pub fn line(node: Node<'_>) -> u32 {
    (node.start_position().row + 1)
        .try_into()
        .unwrap_or(u32::MAX)
}

/// Collect named tree-sitter children for a node.
pub fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

/// Count top-level call arguments.
pub fn count_args(arguments: &str) -> usize {
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
