use serde::{Deserialize, Serialize};

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
    pub contract_name: String,
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
}

/// A parsed .sol file containing one or more contracts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmFile {
    pub path: String,
    pub imports: Vec<EvmImport>,
    pub contracts: Vec<EvmContract>,
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
