#![allow(dead_code)]
// PR #1 defines the output contract up front; some variants become live in later slices.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Format {
    Markdown,
    Json,
    Mermaid,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Fqn(pub String);

impl fmt::Display for Fqn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Confidence {
    Resolved,
    SingleImpl,
    Primary,
    Qualifier,
    Ambiguous,
    External,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalKind {
    Jdk,
    SpringData,
    ThirdParty,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlKind {
    Optional,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub verb: HttpVerb,
    pub path: String,
    pub handler_fqn: Fqn,
    pub file: PathBuf,
    pub line: u32,
}

#[derive(Clone, Debug)]
pub struct ParamInfo {
    pub name: String,
    pub ty: String,
    pub source: ParamSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParamSource {
    Path,
    Query,
    Body,
    Header,
    Unspecified,
}

#[derive(Clone, Debug)]
pub struct ClassInfo {
    pub fqn: Fqn,
    pub simple_name: String,
    pub package: String,
    pub imports: HashMap<String, String>,
    pub kind: ClassKind,
    pub annotations: Vec<String>,
    pub extends: Vec<TypeRef>,
    pub implements: Vec<TypeRef>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub file: PathBuf,
    pub line: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClassKind {
    Class,
    Interface,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRef {
    pub raw: String,
    pub generics: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct FieldInfo {
    pub name: String,
    pub ty: TypeRef,
    pub annotations: Vec<String>,
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub enum BodyElement {
    Call(CallSite),
    Branch(BranchSyntax),
    Loop(LoopSyntax),
}

#[derive(Clone, Debug)]
pub struct LambdaSyntax {
    pub kind: LambdaKind,
    pub source: String,
    pub body: Vec<BodyElement>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LambdaKind {
    Lambda,
    MethodRef,
}

#[derive(Clone, Debug)]
pub struct BranchSyntax {
    pub kind: BranchKind,
    pub condition_src: String,
    pub condition_calls: Vec<BodyElement>,
    pub then_arm: Vec<BodyElement>,
    pub else_arm: Option<Vec<BodyElement>>,
    pub then_terminates: bool,
    pub else_terminates: bool,
}

#[derive(Clone, Debug)]
pub struct LoopSyntax {
    pub kind: LoopKind,
    pub source: String,
    pub condition_calls: Vec<BodyElement>,
    pub body: Vec<BodyElement>,
    pub update_calls: Vec<BodyElement>,
    pub locals: Vec<LoopLocal>,
}

#[derive(Clone, Debug)]
pub struct LoopLocal {
    pub name: String,
    pub ty: TypeRef,
}

#[derive(Clone, Debug)]
pub struct CallSite {
    pub receiver: ReceiverKind,
    pub method_name: String,
    pub arity: usize,
    pub lambdas: Vec<LambdaSyntax>,
    pub line: u32,
}

#[derive(Clone, Debug)]
pub enum ReceiverKind {
    This,
    Field(String),
    Local(String),
    TypeName(String),
    Constructor(String),
    Chain(Box<CallSite>),
}

#[derive(Clone, Debug, Default)]
pub struct ProjectIndex {
    pub classes: HashMap<Fqn, ClassInfo>,
    pub by_simple_name: HashMap<String, Vec<Fqn>>,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Clone, Debug)]
pub struct Flow {
    pub endpoint: Endpoint,
    pub inputs: Vec<ParamInfo>,
    pub root: CallNode,
    pub unresolved: Vec<UnresolvedRef>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct UnresolvedRef {
    pub receiver_type: String,
    pub method_name: String,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct CallNode {
    pub method_fqn: Fqn,
    pub confidence: Confidence,
    pub external_kind: Option<ExternalKind>,
    pub control_kind: Option<ControlKind>,
    pub scope: Option<Scope>,
    pub note: Option<String>,
    pub children: Vec<FlowNode>,
}

#[derive(Clone, Debug)]
pub enum FlowNode {
    Call(CallNode),
    Lambda(LambdaNode),
    Branch(BranchNode),
    Loop(LoopNode),
}

#[derive(Clone, Debug)]
pub struct LambdaNode {
    pub kind: LambdaKind,
    pub source: String,
    pub children: Vec<FlowNode>,
}

#[derive(Clone, Debug)]
pub struct BranchNode {
    pub kind: BranchKind,
    pub condition_src: String,
    pub arms: Vec<Arm>,
}

#[derive(Clone, Debug)]
pub struct LoopNode {
    pub kind: LoopKind,
    pub source: String,
    pub condition: Vec<FlowNode>,
    pub body: Vec<FlowNode>,
    pub update: Vec<FlowNode>,
}

#[derive(Clone, Debug)]
pub struct Arm {
    pub label: String,
    pub terminates: bool,
    pub children: Vec<FlowNode>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchKind {
    If,
    Optional,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopKind {
    For,
    EnhancedFor,
    While,
    DoWhile,
    ForEach,
    Stream,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Scope {
    IntraClass,
}
