//! Command-line entry point for Java Flow Mapper.
//!
//! The binary keeps setup thin: initialize tracing, then delegate CLI parsing
//! and command execution to `cli`.

mod cli;
mod flow;
mod model;
mod parser;
mod render;
mod spring;

use anyhow::Result;
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> Result<()> {
    init_tracing();
    cli::run()
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("off"));
    let _ = fmt().with_env_filter(filter).without_time().try_init();
}
