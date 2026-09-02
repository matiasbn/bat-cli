use serde::{Deserialize, Serialize};
use std::fmt;

/// Solidity function visibility levels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvmVisibility {
    External,
    Public,
    Internal,
    Private,
}

/// Solidity state mutability specifiers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvmMutability {
    Pure,
    View,
    Payable,
    NonPayable,
}

/// Types of Solidity contracts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvmContractType {
    Contract,
    Interface,
    Abstract,
    Library,
}

/// Access control patterns detected in Solidity code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AccessControlType {
    /// `onlyOwner` modifier (OpenZeppelin Ownable)
    OnlyOwner,
    /// Role-based (`hasRole`, `onlyRole`) — OpenZeppelin AccessControl
    RoleBased { role: String },
    /// Inline `require(msg.sender == ...)` check
    RequireMsgSender { compared_to: String },
    /// Custom modifier with access control semantics
    CustomModifier { name: String },
    /// No access control detected
    None,
}

/// Represents a Solidity parameter (function param or return).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmParam {
    pub name: String,
    pub type_name: String,
    pub storage_location: Option<String>,
}

/// Storage variable info.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageVariable {
    pub name: String,
    pub type_name: String,
    pub visibility: EvmVisibility,
    pub is_constant: bool,
    pub is_immutable: bool,
    pub line: usize,
}

/// Event definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmEvent {
    pub name: String,
    pub params: Vec<EvmParam>,
    pub is_anonymous: bool,
    pub line: usize,
}

/// Modifier definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmModifierDef {
    pub name: String,
    pub params: Vec<EvmParam>,
    pub body_source: String,
    pub line: usize,
    #[serde(default)]
    pub end_line: usize,
    pub contract_name: String,
    /// Storage locations this modifier writes (e.g. OZ's `initializer` sets
    /// `$._initialized`). A modifier runs as part of the function it guards, so
    /// these are real state changes — the diagram marks the modifier node red.
    #[serde(default)]
    pub storage_writes: Vec<String>,
    /// Each write with the FILE line (1-based) it sits on, for the per-line marker.
    #[serde(default)]
    pub storage_write_sites: Vec<(String, usize)>,
}

/// A parsed Solidity function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmFunction {
    pub name: String,
    pub contract_name: String,
    pub visibility: EvmVisibility,
    pub mutability: EvmMutability,
    pub modifiers: Vec<String>,
    pub params: Vec<EvmParam>,
    pub returns: Vec<EvmParam>,
    pub body_source: String,
    pub line: usize,
    pub end_line: usize,
    pub is_constructor: bool,
    pub is_fallback: bool,
    pub is_receive: bool,
}

/// A parsed Solidity contract/interface/library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmContract {
    pub name: String,
    pub contract_type: EvmContractType,
    pub base_contracts: Vec<String>,
    pub functions: Vec<EvmFunction>,
    pub modifiers: Vec<EvmModifierDef>,
    pub storage_variables: Vec<StorageVariable>,
    pub events: Vec<EvmEvent>,
    pub file_path: String,
    pub line: usize,
    /// true if the contract comes from lib/ (external dependency)
    pub external: bool,
    /// Struct types declared inside this contract/library (with field types), used
    /// to resolve the type of a `structPointer.field` call receiver.
    #[serde(default)]
    pub structs: Vec<EvmStruct>,
    /// Structs and enums declared inside this contract, as locatable index entries.
    ///
    /// The file parser hoists these into the file's `file_items` so they are stamped
    /// with a path and reach the metadata like any file-level declaration.
    #[serde(default)]
    pub inner_items: Vec<EvmFileItem>,
}

/// A struct type and its fields (name + declared type), used for type inference of
/// storage-pointer field accesses like `$.borrowerOps`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmStruct {
    pub name: String,
    pub fields: Vec<EvmParam>,
}

/// Kinds of file-level declarations (not inside a contract).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvmFileItemKind {
    Struct,
    Enum,
    Error,
    TypeAlias,
    Constant,
    FreeFunction,
    Event,
}

impl fmt::Display for EvmFileItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Struct => write!(f, "struct"),
            Self::Enum => write!(f, "enum"),
            Self::Error => write!(f, "error"),
            Self::TypeAlias => write!(f, "type"),
            Self::Constant => write!(f, "constant"),
            Self::FreeFunction => write!(f, "fn"),
            Self::Event => write!(f, "event"),
        }
    }
}

/// A file-level declaration (struct, enum, error, type alias, constant, free function).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmFileItem {
    pub name: String,
    pub kind: EvmFileItemKind,
    pub file_path: String,
    pub line: usize,
    pub end_line: usize,
    pub external: bool,
    /// Contract, library or interface this was declared inside; empty when the
    /// declaration sits at file level and belongs to nobody.
    ///
    /// Solidity puts most structs and enums inside a contract, so without this the
    /// index only ever saw the handful declared at file level.
    #[serde(default)]
    pub owner: String,
}

/// A parsed .sol file containing one or more contracts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmFile {
    pub path: String,
    pub imports: Vec<EvmImport>,
    pub contracts: Vec<EvmContract>,
    pub file_items: Vec<EvmFileItem>,
    pub pragma: Option<String>,
}

/// An import statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmImport {
    pub path: String,
    pub symbols: Vec<ImportSymbol>,
    pub line: usize,
}

/// A single imported symbol (named import).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSymbol {
    pub name: String,
    pub alias: Option<String>,
}
