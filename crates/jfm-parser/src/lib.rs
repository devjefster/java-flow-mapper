//! Java parsing and project indexing.

pub use jfm_model as model;

mod parser;

pub use parser::{ParsedFile, index_project, parse_file};
