//! Flow graph construction from an indexed Java project.
//!
//! The resolver starts at a Spring endpoint handler and expands reachable calls
//! into a bounded graph that renderers can present in multiple formats.

use std::collections::HashSet;

use thiserror::Error;

pub use jfm_model as model;
pub use jfm_spring as spring;

use crate::model::{Confidence, Flow, Fqn, HttpVerb, ProjectIndex};

use self::expand::expand_method;
use self::resolve::find_method;

mod expand;
mod external;
mod node;
mod resolve;

/// Resolver-time recursion cap. This protects graph construction and is
/// independent of render-time `--max-depth`, which only trims output.
const MAX_DEPTH: usize = 8;

/// Errors returned while selecting or expanding an endpoint flow.
#[derive(Debug, Error)]
pub enum FlowError {
    /// No endpoints found for the given verb and path.
    #[error("no endpoints found while looking for {verb} {path}")]
    NoEndpointsFound { verb: HttpVerb, path: String },
    /// The requested endpoint was not found.
    #[error("endpoint not found for {verb} {path}")]
    EndpointNotFound { verb: HttpVerb, path: String },
    /// The handler method for the endpoint is missing from the project index.
    #[error("handler method `{0}` was not found in the project index")]
    HandlerMissing(Fqn),
}

/// Build a resolved flow graph for one HTTP endpoint.
pub fn build_flow(index: &ProjectIndex, verb: HttpVerb, path: &str) -> Result<Flow, FlowError> {
    if index.endpoints.is_empty() {
        return Err(FlowError::NoEndpointsFound {
            verb,
            path: path.to_string(),
        });
    }

    let endpoint = index
        .endpoints
        .iter()
        .find(|endpoint| endpoint.verb == verb && endpoint.path == path)
        .cloned()
        .ok_or_else(|| FlowError::EndpointNotFound {
            verb,
            path: path.to_string(),
        })?;
    let (owner, method) = find_method(index, &endpoint.handler_fqn)
        .ok_or_else(|| FlowError::HandlerMissing(endpoint.handler_fqn.clone()))?;
    let mut unresolved = Vec::new();
    let mut stack = HashSet::new();
    let root = expand_method(
        index,
        owner,
        method,
        Confidence::Resolved,
        &mut unresolved,
        &mut stack,
        0,
    );

    Ok(Flow {
        endpoint,
        inputs: method.params.clone(),
        root,
        unresolved,
        notes: vec![
            "Stream, Optional, and common JDK chain operators use a small hardcoded return-shape table; this is not a general type inferencer.".to_string(),
            "Optional.orElseThrow return type is flattened to Object; generic-aware unwrapping is intentionally deferred.".to_string(),
            "Optional present/empty behavior is rendered structurally, but JFM does not predict which arm runs for a request.".to_string(),
            "AOP, @Transactional, @ControllerAdvice, DI qualifiers, Lombok, and Bean Validation are not modeled.".to_string(),
        ],
    })
}
