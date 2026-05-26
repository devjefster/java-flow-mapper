mod annotations;
mod body;
mod class;
mod utils;
#[allow(dead_code)]
#[cfg(test)]
mod walker;

use std::fs;
use std::path::Path;
use tree_sitter::Node;
use walkdir::{DirEntry, WalkDir};

use anyhow::{Context, Result};
use tracing::debug;

use self::class::{collect_classes, extract_imports, extract_package};
use self::utils::join_paths;
use crate::model::{ClassInfo, Endpoint, ProjectIndex};

pub struct ParsedFile {
    pub classes: Vec<ClassInfo>,
    pub endpoints: Vec<Endpoint>,
}

pub fn index_project(root: &Path) -> Result<ProjectIndex> {
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
    parser
        .set_language(&language)
        .context("while loading tree-sitter-java language")?;

    let mut index = ProjectIndex::default();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| !is_skipped(entry))
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|ext| ext != "java")
        {
            continue;
        }

        let source = fs::read_to_string(entry.path())
            .with_context(|| format!("while reading {}", entry.path().display()))?;
        let tree = parser
            .parse(&source, None)
            .with_context(|| format!("while parsing {}", entry.path().display()))?;
        let parsed = parse_file(entry.path(), &source, tree.root_node());

        for class in parsed.classes {
            index
                .by_simple_name
                .entry(class.simple_name.clone())
                .or_default()
                .push(class.fqn.clone());
            index.classes.insert(class.fqn.clone(), class);
        }
        index.endpoints.extend(parsed.endpoints);
    }

    debug!(
        root = %root.display(),
        classes = index.classes.len(),
        endpoints = index.endpoints.len(),
        "indexed Java project"
    );

    Ok(index)
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

fn is_skipped(entry: &DirEntry) -> bool {
    let Some(name) = entry.file_name().to_str() else {
        return false;
    };
    matches!(
        name,
        "target" | "build" | "node_modules" | ".git" | ".idea" | ".mvn" | ".gradle"
    )
}
