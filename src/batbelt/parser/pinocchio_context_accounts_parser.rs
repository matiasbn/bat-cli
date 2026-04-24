use crate::batbelt::parser::context_accounts_parser::CAAccountParser;
use crate::batbelt::parser::solana_account_parser::SolanaAccountType;
use crate::batbelt::parser::syn_context_accounts_parser::{
    ParsedAccount, ParsedAccountAttributes, ParsedAccountsStruct,
};
use crate::batbelt::parser::ParserError;
use error_stack::{Report, Result};
use std::fs;

/// Parse Pinocchio-style context accounts from a file.
///
/// Detects structs whose fields are all `&'a AccountView` and pairs them
/// with `impl TryFrom<&[AccountView]> for StructName` blocks.
///
/// Check inference is **heuristic-based**: any `SomeType::check(field)` or
/// `SomeType::check_with_program(field, ...)` call in the TryFrom body is
/// captured. The check-type name is then matched against common keywords
/// (signer, writable, mint, token, system, program) to infer account semantics.
/// Unknown checks are stored verbatim as validations.
pub fn parse_pinocchio_context_accounts_from_file(
    file_path: &str,
) -> Result<Vec<ParsedAccountsStruct>, ParserError> {
    let content = fs::read_to_string(file_path).map_err(|e| {
        Report::new(ParserError)
            .attach_printable(format!("Failed to read file {}: {}", file_path, e))
    })?;
    parse_pinocchio_context_accounts_from_source(&content)
}

pub fn parse_pinocchio_context_accounts_from_source(
    source: &str,
) -> Result<Vec<ParsedAccountsStruct>, ParserError> {
    let file = syn::parse_file(source).map_err(|e| {
        Report::new(ParserError).attach_printable(format!("Failed to parse Rust source: {}", e))
    })?;

    // Step 1: Find structs with all fields being `&'a AccountView`
    let mut account_structs: Vec<(String, Vec<String>)> = Vec::new();
    for item in &file.items {
        if let syn::Item::Struct(item_struct) = item {
            if is_account_view_struct(item_struct) {
                let name = item_struct.ident.to_string();
                let field_names = extract_field_names(item_struct);
                account_structs.push((name, field_names));
            }
        }
    }

    if account_structs.is_empty() {
        return Ok(Vec::new());
    }

    // Step 2: Find TryFrom impls and extract checks generically
    let try_from_checks = extract_try_from_checks(&file);

    // Step 3: Build ParsedAccountsStruct for each struct
    let mut results = Vec::new();
    for (struct_name, field_names) in &account_structs {
        let checks = try_from_checks
            .iter()
            .find(|c| c.struct_name == *struct_name);
        let accounts = build_parsed_accounts(field_names, checks);
        results.push(ParsedAccountsStruct {
            name: struct_name.clone(),
            accounts,
        });
    }

    Ok(results)
}

/// Check if a struct has all named fields of type `&'a AccountView` or `&'a AccountInfo`.
/// Pinocchio uses `AccountView` (v0.10+) or `AccountInfo` depending on version.
fn is_account_view_struct(item_struct: &syn::ItemStruct) -> bool {
    if let syn::Fields::Named(fields) = &item_struct.fields {
        if fields.named.is_empty() {
            return false;
        }
        fields
            .named
            .iter()
            .all(|field| is_pinocchio_account_ref(&field.ty))
    } else {
        false
    }
}

/// Check if a type is `&'a AccountView` or `&'a AccountInfo` (Pinocchio account reference).
fn is_pinocchio_account_ref(ty: &syn::Type) -> bool {
    if let syn::Type::Reference(type_ref) = ty {
        if let syn::Type::Path(type_path) = &*type_ref.elem {
            if let Some(seg) = type_path.path.segments.last() {
                return seg.ident == "AccountView" || seg.ident == "AccountInfo";
            }
        }
    }
    false
}

fn extract_field_names(item_struct: &syn::ItemStruct) -> Vec<String> {
    if let syn::Fields::Named(fields) = &item_struct.fields {
        fields
            .named
            .iter()
            .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
            .collect()
    } else {
        Vec::new()
    }
}

// ─── Generic check extraction ─────────────────────────────────────────────────

/// A single check call found in a TryFrom body.
#[derive(Debug, Clone)]
struct RawCheck {
    /// The full check expression, e.g. "SignerAccount::check" or "WritableAccount::check"
    full_check_name: String,
    /// The field name this check applies to (first argument)
    field_name: String,
}

/// Inferred semantic from a check name.
#[derive(Debug, Clone, PartialEq)]
enum CheckSemantic {
    Signer,
    Writable,
    ProgramOwned,
    SystemProgram,
    TokenProgram,
    Mint,
    TokenAccount,
    /// A check we couldn't classify — stored as a generic validation
    Unknown(String),
}

struct TryFromChecks {
    struct_name: String,
    raw_checks: Vec<RawCheck>,
}

/// Extract TryFrom<&[AccountView]> implementations and their check calls.
fn extract_try_from_checks(file: &syn::File) -> Vec<TryFromChecks> {
    let mut results = Vec::new();

    for item in &file.items {
        if let syn::Item::Impl(item_impl) = item {
            if let Some((_, trait_path, _)) = &item_impl.trait_ {
                let trait_name = trait_path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::");

                if trait_name != "TryFrom" {
                    continue;
                }

                let struct_name = if let syn::Type::Path(tp) = &*item_impl.self_ty {
                    tp.path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default()
                } else {
                    continue;
                };

                let raw_checks = parse_generic_checks_from_impl(item_impl);

                results.push(TryFromChecks {
                    struct_name,
                    raw_checks,
                });
            }
        }
    }

    results
}

/// Generically extract all `Something::check(field)` and
/// `Something::check_method(field, ...)` patterns from a TryFrom impl body.
///
/// Also detects `field.is_signer()`, `field.is_writable()`, `field.owned_by()`
/// inline method calls.
fn parse_generic_checks_from_impl(item_impl: &syn::ItemImpl) -> Vec<RawCheck> {
    use quote::ToTokens;
    let impl_text = item_impl.to_token_stream().to_string();

    let mut raw_checks: Vec<RawCheck> = Vec::new();

    // to_token_stream produces a single line; split by `;` to get statements
    for statement in impl_text.split(';') {
        let trimmed = statement.trim();

        // Pattern 1: `TypeName :: method_name ( field_name )` or `TypeName :: method_name ( field_name , ...)`
        if let Some(check) = extract_generic_static_check(trimmed) {
            raw_checks.push(check);
        }

        // Pattern 2: `TypeName :: method_name ( tp , & [ f1 , f2 ] )`
        if let Some(checks) = extract_batch_check(trimmed) {
            raw_checks.extend(checks);
        }

        // Pattern 3: inline method calls `field . is_signer ( )`, `field . is_writable ( )`
        if let Some(check) = extract_inline_method_check(trimmed) {
            raw_checks.push(check);
        }
    }

    raw_checks
}

/// Extract `TypeName :: method ( field , ... )` → RawCheck
fn extract_generic_static_check(line: &str) -> Option<RawCheck> {
    // Look for pattern: IDENT :: IDENT ( IDENT
    // In proc_macro2 output, `::` has spaces around it
    let re_pattern = " :: ";
    let pos = line.find(re_pattern)?;

    // Get the type name before ::
    let before = line[..pos].trim();
    let type_name = before.split_whitespace().last()?;
    // Must start with uppercase (it's a type, not a variable)
    if !type_name.chars().next()?.is_uppercase() {
        return None;
    }

    let after_colons = &line[pos + re_pattern.len()..];
    // Get method name and opening paren
    let paren_pos = after_colons.find('(')?;
    let method_name = after_colons[..paren_pos].trim();
    // Method should contain "check" to be relevant (or similar validation method)
    // But let's be generous and capture any static call

    let args_start = paren_pos + 1;
    let after_paren = after_colons[args_start..].trim();

    // Get the first argument (field name)
    let field = after_paren
        .split(|c: char| c == ')' || c == ',')
        .next()?
        .trim();

    // Must be a simple identifier
    if field.is_empty() || !field.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    // Skip if field starts with uppercase (it's a type, not a field)
    if field.chars().next()?.is_uppercase() {
        return None;
    }

    let full_check_name = format!("{}::{}", type_name, method_name);

    Some(RawCheck {
        full_check_name,
        field_name: field.to_string(),
    })
}

/// Extract batch checks like `Type :: method ( tp , & [ f1 , f2 ] )`.
fn extract_batch_check(line: &str) -> Option<Vec<RawCheck>> {
    // Must contain :: and &[
    let colons_pos = line.find(" :: ")?;
    let bracket_start = line.find("& [")?;

    if bracket_start < colons_pos {
        return None;
    }

    let before = line[..colons_pos].trim();
    let type_name = before.split_whitespace().last()?;
    if !type_name.chars().next()?.is_uppercase() {
        return None;
    }

    let after_colons = &line[colons_pos + 4..];
    let paren_pos = after_colons.find('(')?;
    let method_name = after_colons[..paren_pos].trim();
    let full_check_name = format!("{}::{}", type_name, method_name);

    // Extract fields from &[ f1 , f2 ]
    let bracket_content_start = line.find('[')? + 1;
    let bracket_content_end = line.find(']')?;
    let inner = &line[bracket_content_start..bracket_content_end];

    let fields: Vec<String> = inner
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .collect();

    if fields.is_empty() {
        return None;
    }

    Some(
        fields
            .into_iter()
            .map(|field_name| RawCheck {
                full_check_name: full_check_name.clone(),
                field_name,
            })
            .collect(),
    )
}

/// Extract inline method check: `field . is_signer ( )` or `field . is_writable ( )`.
fn extract_inline_method_check(line: &str) -> Option<RawCheck> {
    // In token stream: `field . is_signer ( )`
    for (method, check_name) in &[
        (". is_signer (", "is_signer"),
        (". is_writable (", "is_writable"),
        (". owned_by (", "owned_by"),
    ] {
        if let Some(pos) = line.find(method) {
            let before = line[..pos].trim();
            // Also handle `! field . is_signer`
            let field = before
                .trim_start_matches('!')
                .trim()
                .split_whitespace()
                .last()?;
            if !field.is_empty() && field.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Some(RawCheck {
                    full_check_name: check_name.to_string(),
                    field_name: field.to_string(),
                });
            }
        }
    }
    None
}

// ─── Heuristic classification ─────────────────────────────────────────────────

/// Infer the semantic meaning of a check from its name using keyword heuristics.
fn classify_check(check_name: &str) -> CheckSemantic {
    let lower = check_name.to_lowercase();

    // Order matters — more specific patterns first
    if lower.contains("signer") || lower == "is_signer" {
        CheckSemantic::Signer
    } else if lower.contains("writable") || lower == "is_writable" {
        CheckSemantic::Writable
    } else if lower.contains("system") {
        CheckSemantic::SystemProgram
    } else if lower.contains("tokenprograminterface")
        || lower.contains("token_program_interface")
        || (lower.contains("tokenprogram") && !lower.contains("tokenaccount"))
    {
        CheckSemantic::TokenProgram
    } else if lower.contains("mintinterface")
        || lower.contains("mint_interface")
        || (lower.contains("mint") && !lower.contains("token"))
    {
        CheckSemantic::Mint
    } else if lower.contains("tokenaccountinterface")
        || lower.contains("token_account_interface")
        || lower.contains("tokenaccount")
    {
        CheckSemantic::TokenAccount
    } else if lower.contains("programaccount")
        || lower.contains("program_account")
        || lower == "owned_by"
    {
        CheckSemantic::ProgramOwned
    } else {
        CheckSemantic::Unknown(check_name.to_string())
    }
}

// ─── Build parsed accounts ────────────────────────────────────────────────────

fn build_parsed_accounts(
    field_names: &[String],
    checks: Option<&TryFromChecks>,
) -> Vec<ParsedAccount> {
    field_names
        .iter()
        .map(|field_name| {
            let mut attrs = ParsedAccountAttributes::default();
            let mut wrapper = "AccountView".to_string();
            let mut struct_name = "AccountView".to_string();
            let mut validations: Vec<String> = Vec::new();

            if let Some(checks) = checks {
                let field_raw_checks: Vec<_> = checks
                    .raw_checks
                    .iter()
                    .filter(|rc| rc.field_name == *field_name)
                    .collect();

                for rc in &field_raw_checks {
                    let semantic = classify_check(&rc.full_check_name);
                    match semantic {
                        CheckSemantic::Signer => {
                            wrapper = "Signer".to_string();
                            struct_name = "Signer".to_string();
                        }
                        CheckSemantic::Writable => {
                            attrs.is_mut = true;
                        }
                        CheckSemantic::ProgramOwned => {
                            if wrapper == "AccountView" {
                                wrapper = "Account".to_string();
                                struct_name = "ProgramAccount".to_string();
                            }
                        }
                        CheckSemantic::SystemProgram => {
                            wrapper = "SystemAccount".to_string();
                            struct_name = "SystemAccount".to_string();
                        }
                        CheckSemantic::TokenProgram => {
                            wrapper = "Program".to_string();
                            struct_name = "TokenProgram".to_string();
                        }
                        CheckSemantic::Mint => {
                            wrapper = "Mint".to_string();
                            struct_name = "Mint".to_string();
                        }
                        CheckSemantic::TokenAccount => {
                            wrapper = "TokenAccount".to_string();
                            struct_name = "TokenAccount".to_string();
                        }
                        CheckSemantic::Unknown(_) => {}
                    }
                }

                // Store all raw checks as validations
                for rc in &field_raw_checks {
                    validations.push(rc.full_check_name.clone());
                }
            }

            // Store validations in the constraints field for later use
            attrs.constraints = validations;

            ParsedAccount {
                field_name: field_name.clone(),
                account_wrapper_name: wrapper,
                account_struct_name: struct_name,
                lifetime_name: "'a".to_string(),
                is_boxed: false,
                attributes: attrs,
                raw_type: "&'a AccountView".to_string(),
            }
        })
        .collect()
}

// ─── Conversion helpers ───────────────────────────────────────────────────────

impl ParsedAccount {
    /// Convert a Pinocchio ParsedAccount into a CAAccountParser.
    pub fn to_pinocchio_ca_account_parser(
        &self,
        solana_account_type: SolanaAccountType,
    ) -> CAAccountParser {
        // Use the constraints collected during parsing as validations
        let validations = self.attributes.constraints.clone();

        CAAccountParser {
            content: String::new(),
            solana_account_type,
            account_struct_name: self.account_struct_name.clone(),
            account_wrapper_name: self.account_wrapper_name.clone(),
            lifetime_name: self.lifetime_name.clone(),
            account_name: self.field_name.clone(),
            is_pda: false,
            is_init: false,
            is_mut: self.attributes.is_mut,
            is_close: false,
            seeds: Vec::new(),
            rent_exemption_account: String::new(),
            validations,
            owner: None,
            token_mint: None,
            space: None,
            rent_exempt: false,
            realloc: None,
            bump: None,
        }
    }

    /// Determine SolanaAccountType for Pinocchio accounts based on inferred wrapper.
    pub fn determine_pinocchio_solana_account_type(&self) -> SolanaAccountType {
        match self.account_wrapper_name.as_str() {
            "Signer" => SolanaAccountType::Signer,
            "SystemAccount" => SolanaAccountType::SystemAccount,
            "Mint" => SolanaAccountType::Mint,
            "TokenAccount" => SolanaAccountType::TokenAccount,
            "Program" => SolanaAccountType::Other,
            "Account" => SolanaAccountType::ProgramStateAccount,
            _ => SolanaAccountType::UncheckedAccount,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_pinocchio_struct() {
        let source = r#"
            pub struct InitializeAccounts<'a> {
                pub user: &'a AccountView,
                pub state: &'a AccountView,
                pub system_program: &'a AccountView,
            }

            impl<'a> TryFrom<&'a [AccountView]> for InitializeAccounts<'a> {
                type Error = ProgramError;
                fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
                    let [user, state, system_program] = accounts else {
                        return Err(ProgramError::NotEnoughAccountKeys);
                    };
                    SignerAccount::check(user)?;
                    WritableAccount::check(user)?;
                    WritableAccount::check(state)?;
                    ProgramAccount::check(state)?;
                    SystemAccount::check(system_program)?;
                    Ok(Self { user, state, system_program })
                }
            }
        "#;
        let result = parse_pinocchio_context_accounts_from_source(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "InitializeAccounts");
        assert_eq!(result[0].accounts.len(), 3);

        let user = &result[0].accounts[0];
        assert_eq!(user.field_name, "user");
        assert_eq!(user.account_wrapper_name, "Signer");
        assert!(user.attributes.is_mut);

        let state = &result[0].accounts[1];
        assert_eq!(state.field_name, "state");
        assert!(state.attributes.is_mut);
        assert_eq!(state.account_struct_name, "ProgramAccount");

        let sys = &result[0].accounts[2];
        assert_eq!(sys.field_name, "system_program");
        assert_eq!(sys.account_wrapper_name, "SystemAccount");
    }

    #[test]
    fn test_token_checks() {
        let source = r#"
            pub struct TransferAccounts<'a> {
                pub token_mint: &'a AccountView,
                pub user_ata: &'a AccountView,
                pub token_program: &'a AccountView,
            }

            impl<'a> TryFrom<&'a [AccountView]> for TransferAccounts<'a> {
                type Error = ProgramError;
                fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
                    let [token_mint, user_ata, token_program] = accounts else {
                        return Err(ProgramError::NotEnoughAccountKeys);
                    };
                    MintInterface::check_with_program(token_mint, token_program)?;
                    TokenAccountInterface::check_with_program(user_ata, token_program)?;
                    TokenProgramInterface::check(token_program)?;
                    Ok(Self { token_mint, user_ata, token_program })
                }
            }
        "#;
        let result = parse_pinocchio_context_accounts_from_source(source).unwrap();
        assert_eq!(result[0].accounts.len(), 3);

        let mint = &result[0].accounts[0];
        assert_eq!(mint.account_wrapper_name, "Mint");

        let ata = &result[0].accounts[1];
        assert_eq!(ata.account_wrapper_name, "TokenAccount");

        let tp = &result[0].accounts[2];
        assert_eq!(tp.account_wrapper_name, "Program");
        assert_eq!(tp.account_struct_name, "TokenProgram");
    }

    #[test]
    fn test_no_account_view_struct() {
        let source = r#"
            pub struct RegularStruct {
                pub field: u64,
            }
        "#;
        let result = parse_pinocchio_context_accounts_from_source(source).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_struct_without_tryfrom() {
        let source = r#"
            pub struct SomeAccounts<'a> {
                pub user: &'a AccountView,
            }
        "#;
        let result = parse_pinocchio_context_accounts_from_source(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].accounts[0].account_wrapper_name, "AccountView");
    }

    #[test]
    fn test_custom_check_names() {
        // A hypothetical project with different naming conventions
        let source = r#"
            pub struct CustomAccounts<'a> {
                pub authority: &'a AccountView,
                pub vault: &'a AccountView,
            }

            impl<'a> TryFrom<&'a [AccountView]> for CustomAccounts<'a> {
                type Error = ProgramError;
                fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
                    let [authority, vault] = accounts else {
                        return Err(ProgramError::NotEnoughAccountKeys);
                    };
                    RequireSigner::check(authority)?;
                    CustomValidation::check(vault)?;
                    Ok(Self { authority, vault })
                }
            }
        "#;
        let result = parse_pinocchio_context_accounts_from_source(source).unwrap();
        let auth = &result[0].accounts[0];
        // "RequireSigner" contains "signer" → detected as Signer
        assert_eq!(auth.account_wrapper_name, "Signer");

        let vault = &result[0].accounts[1];
        // "CustomValidation" doesn't match any known keyword → Unknown, stays AccountView
        assert_eq!(vault.account_wrapper_name, "AccountView");
        // But the validation is still captured
        assert!(vault.attributes.constraints.iter().any(|v| v.contains("CustomValidation")));
    }

    #[test]
    fn test_inline_method_checks() {
        let source = r#"
            pub struct InlineAccounts<'a> {
                pub payer: &'a AccountView,
                pub state: &'a AccountView,
            }

            impl<'a> TryFrom<&'a [AccountView]> for InlineAccounts<'a> {
                type Error = ProgramError;
                fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
                    let [payer, state] = accounts else {
                        return Err(ProgramError::NotEnoughAccountKeys);
                    };
                    if !payer.is_signer() {
                        return Err(ProgramError::MissingRequiredSignature);
                    }
                    if !state.is_writable() {
                        return Err(ProgramError::InvalidAccountData);
                    }
                    Ok(Self { payer, state })
                }
            }
        "#;
        let result = parse_pinocchio_context_accounts_from_source(source).unwrap();
        let payer = &result[0].accounts[0];
        assert_eq!(payer.account_wrapper_name, "Signer");

        let state = &result[0].accounts[1];
        assert!(state.attributes.is_mut);
    }
}
