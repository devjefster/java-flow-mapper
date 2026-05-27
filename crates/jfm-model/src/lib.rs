//! Shared data contracts for parsing, resolving, and rendering Java flows.
//!
//! Parser modules populate source-level syntax types, the flow resolver expands
//! them into graph nodes, and renderers consume the resulting `Flow`.

#![allow(dead_code)]
// PR #1 defines the output contract up front; some variants become live in later slices.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Output format selected by the CLI.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, clap::ValueEnum)]
pub enum Format {
    /// Human-readable Markdown.
    Markdown,
    /// Structured JSON contract.
    Json,
    /// Mermaid sequence diagram.
    Mermaid,
}

/// Mermaid diagram shape selected by the CLI.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, clap::ValueEnum)]
pub enum Diagram {
    /// Message-oriented Mermaid sequence diagram.
    Sequence,
    /// Graph-oriented Mermaid flowchart.
    Flowchart,
}

/// Fully qualified symbol name used as a stable key across indexes and flows.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Fqn(pub String);

impl fmt::Display for Fqn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// How confidently a call target was resolved.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Confidence {
    /// A concrete project method matched directly.
    Resolved,
    /// A single implementation was selected for an interface target.
    SingleImpl,
    /// A primary bean resolved a DI ambiguity.
    Primary,
    /// A qualifier resolved a DI ambiguity.
    Qualifier,
    /// Multiple project targets remain possible.
    Ambiguous,
    /// The target is outside the indexed project.
    External,
    /// The target could not be resolved.
    Unresolved,
}

/// External target classification used by rendered notes and JSON.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ExternalKind {
    /// Java, javax, or Jakarta APIs.
    Jdk,
    /// Low-signal Java library calls that are usually implementation detail.
    JdkLibrary,
    /// Synthesized Spring Data repository behavior.
    SpringData,
    /// Imported non-JDK code outside the indexed project.
    ThirdParty,
    /// External target with unknown origin.
    Unknown,
}

/// Known external calls that behave like control flow.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ControlKind {
    /// Java `Optional` methods with present/empty arms.
    Optional,
    /// Java collection or stream traversal methods with callback bodies.
    Traversal,
}

/// HTTP verb for a Spring endpoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HttpVerb {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl std::str::FromStr for HttpVerb {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "DELETE" => Ok(Self::Delete),
            "PATCH" => Ok(Self::Patch),
            other => Err(format!("unsupported HTTP verb `{other}`")),
        }
    }
}

impl fmt::Display for HttpVerb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Patch => "PATCH",
        })
    }
}

/// Spring MVC endpoint discovered from mapping annotations.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Endpoint {
    pub verb: HttpVerb,
    pub path: String,
    pub handler_fqn: Fqn,
    pub file: PathBuf,
    pub line: u32,
}

/// Method parameter with the request source, when known.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ParamInfo {
    pub name: String,
    pub ty: String,
    pub source: ParamSource,
    #[serde(default)]
    pub annotations: Vec<String>,
    #[serde(default)]
    pub validation: Vec<ValidationField>,
}

/// Request binding source for a method parameter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ParamSource {
    Path,
    Query,
    Body,
    Header,
    Unspecified,
}

/// Bean Validation constraints attached to a field of an endpoint input DTO.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ValidationField {
    pub field: String,
    pub ty: String,
    pub constraints: Vec<ValidationConstraint>,
}

/// Bean Validation constraint annotation, preserving source spelling for output.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ValidationConstraint {
    pub annotation: String,
    pub raw: String,
    #[serde(default)]
    pub custom_validator: Option<String>,
}

/// Parsed Java class or interface.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClassInfo {
    pub fqn: Fqn,
    pub simple_name: String,
    pub package: String,
    pub imports: HashMap<String, String>,
    pub kind: ClassKind,
    pub annotations: Vec<String>,
    #[serde(default)]
    pub validation: Vec<ValidationConstraint>,
    pub extends: Vec<TypeRef>,
    pub implements: Vec<TypeRef>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub file: PathBuf,
    pub line: u32,
}

/// Java type declaration kind supported by the parser.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ClassKind {
    Class,
    Interface,
    Enum,
    Annotation,
}

/// Java type reference, preserving the raw spelling and top-level generics.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TypeRef {
    pub raw: String,
    pub generics: Vec<String>,
}

/// Parsed field used for receiver/type resolution.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FieldInfo {
    pub name: String,
    pub ty: TypeRef,
    pub annotations: Vec<String>,
    #[serde(default)]
    pub validation: Vec<ValidationConstraint>,
}

/// Parsed method or constructor with body elements and local variable types.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MethodInfo {
    pub fqn: Fqn,
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub return_type: TypeRef,
    pub annotations: Vec<String>,
    pub body: Vec<BodyElement>,
    pub locals: HashMap<String, TypeRef>,
    pub file: PathBuf,
    pub line: u32,
}

/// Source-level method body element before flow expansion.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum BodyElement {
    /// A method or constructor call.
    Call(CallSite),
    /// A parsed branch such as `if`.
    Branch(BranchSyntax),
    /// A parsed loop.
    Loop(LoopSyntax),
}

/// Lambda or method reference captured from call arguments.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LambdaSyntax {
    pub kind: LambdaKind,
    pub source: String,
    pub body: Vec<BodyElement>,
}

/// Java inline function syntax carried by a call argument.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LambdaKind {
    /// Lambda expression such as `x -> work(x)`.
    Lambda,
    /// Method reference such as `this::work`.
    MethodRef,
}

/// Parsed branch structure before call targets are resolved.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BranchSyntax {
    pub kind: BranchKind,
    pub condition_src: String,
    pub condition_calls: Vec<BodyElement>,
    pub arms: Vec<BranchArmSyntax>,
    pub then_arm: Vec<BodyElement>,
    pub else_arm: Option<Vec<BodyElement>>,
    pub then_terminates: bool,
    pub else_terminates: bool,
}

/// Parsed branch arm before call targets are resolved.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BranchArmSyntax {
    pub label: String,
    pub body: Vec<BodyElement>,
    pub terminates: bool,
}

/// Parsed loop structure before call targets are resolved.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoopSyntax {
    pub kind: LoopKind,
    pub source: String,
    pub execution: LoopExecution,
    pub init_calls: Vec<BodyElement>,
    pub condition_calls: Vec<BodyElement>,
    pub arms: Vec<LoopArmSyntax>,
    pub update_calls: Vec<BodyElement>,
    pub locals: Vec<LoopLocal>,
}

/// Parsed loop arm before call targets are resolved.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoopArmSyntax {
    pub label: String,
    pub body: Vec<BodyElement>,
}

/// Loop-scoped local variable, currently used for enhanced-for variables.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoopLocal {
    pub name: String,
    pub ty: TypeRef,
}

/// Parsed call site with enough receiver information for resolution.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CallSite {
    pub receiver: ReceiverKind,
    pub method_name: String,
    pub arity: usize,
    pub inputs: Vec<BodyElement>,
    pub lambdas: Vec<LambdaSyntax>,
    pub line: u32,
}

/// Receiver shape extracted from Java call syntax.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ReceiverKind {
    /// Implicit or explicit call on the current class.
    This,
    /// Receiver identified as a field name.
    Field(String),
    /// Receiver identified as a local or parameter name.
    Local(String),
    /// Static-style receiver or class literal.
    TypeName(String),
    /// Constructor target from `new`.
    Constructor(String),
    /// Receiver is another call whose return type must be inferred.
    Chain(Box<CallSite>),
}

/// In-memory index of parsed project symbols and endpoints.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProjectIndex {
    pub classes: HashMap<Fqn, ClassInfo>,
    pub by_simple_name: HashMap<String, Vec<Fqn>>,
    pub endpoints: Vec<Endpoint>,
}

/// Resolved call flow for one endpoint.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Flow {
    pub endpoint: Endpoint,
    pub inputs: Vec<ParamInfo>,
    pub root: CallNode,
    pub unresolved: Vec<UnresolvedRef>,
    pub notes: Vec<String>,
}

/// Call that could not be resolved, with a human-readable reason.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UnresolvedRef {
    pub receiver_type: String,
    pub method_name: String,
    pub reason: String,
}

/// Resolved call node in the flow graph.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CallNode {
    pub method_fqn: Fqn,
    pub confidence: Confidence,
    pub external_kind: Option<ExternalKind>,
    pub control_kind: Option<ControlKind>,
    pub scope: Option<Scope>,
    pub note: Option<String>,
    pub inputs: Vec<FlowNode>,
    pub children: Vec<FlowNode>,
}

/// Renderable flow graph node.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum FlowNode {
    /// A method, constructor, external, or unresolved call.
    Call(CallNode),
    /// Lambda wrapper preserving argument syntax.
    Lambda(LambdaNode),
    /// Branch with one or more labeled arms.
    Branch(BranchNode),
    /// Loop with condition/body/update sections.
    Loop(LoopNode),
}

/// Expanded lambda or method reference in the flow graph.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LambdaNode {
    pub kind: LambdaKind,
    pub source: String,
    pub children: Vec<FlowNode>,
}

/// Expanded branch in the flow graph.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BranchNode {
    pub kind: BranchKind,
    pub condition_src: String,
    pub condition: Vec<FlowNode>,
    pub arms: Vec<Arm>,
}

/// Expanded loop in the flow graph.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoopNode {
    pub kind: LoopKind,
    pub source: String,
    pub execution: LoopExecution,
    pub init: Vec<FlowNode>,
    pub condition: Vec<FlowNode>,
    pub arms: Vec<LoopArm>,
    pub update: Vec<FlowNode>,
}

/// Expanded loop arm in the flow graph.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoopArm {
    pub label: String,
    pub children: Vec<FlowNode>,
}

/// Labeled branch arm with termination metadata.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Arm {
    pub label: String,
    pub terminates: bool,
    pub children: Vec<FlowNode>,
}

/// Branch source modeled by the flow graph.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BranchKind {
    /// Source-level `if`.
    If,
    /// Source-level `switch`.
    Switch,
    /// Source-level ternary expression.
    Ternary,
    /// Source-level `try` / `catch` / `finally` block.
    TryCatch,
    /// Synthetic branch for `Optional` present/empty behavior.
    Optional,
}

/// Loop source modeled by the flow graph.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LoopKind {
    For,
    EnhancedFor,
    While,
    DoWhile,
    ForEach,
    Stream,
}

/// Conservative execution cardinality for loop bodies.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LoopExecution {
    ZeroOrMore,
    OneOrMore,
}

/// Scope metadata for calls that need renderer-visible context.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Scope {
    /// Call resolved to another method on the same class.
    IntraClass,
}
