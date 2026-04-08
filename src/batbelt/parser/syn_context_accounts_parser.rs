use crate::batbelt::parser::context_accounts_parser::CAAccountParser;
use crate::batbelt::parser::solana_account_parser::SolanaAccountType;
use crate::batbelt::parser::ParserError;
use error_stack::{Report, Result};
use std::fs;

// ─── Output structs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ParsedAccountsStruct {
    pub name: String,
    pub accounts: Vec<ParsedAccount>,
}

#[derive(Debug, Clone)]
pub struct ParsedAccount {
    pub field_name: String,
    pub account_wrapper_name: String,
    pub account_struct_name: String,
    pub lifetime_name: String,
    pub is_boxed: bool,
    pub attributes: ParsedAccountAttributes,
    pub raw_type: String,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedAccountAttributes {
    pub is_mut: bool,
    pub is_init: bool,
    pub is_close: bool,
    pub is_pda: bool,
    pub close_target: Option<String>,
    pub payer: Option<String>,
    pub space: Option<String>,
    pub owner: Option<String>,
    pub seeds: Vec<String>,
    pub bump: Option<String>,
    pub has_one: Vec<HasOneConstraint>,
    pub address: Option<AddressConstraint>,
    pub constraints: Vec<String>,
    pub token_mint: Option<String>,
    pub token_authority: Option<String>,
    pub associated_token_mint: Option<String>,
    pub associated_token_authority: Option<String>,
    pub associated_token_token_program: Option<String>,
    pub realloc: Option<String>,
    pub rent_exempt: bool,
    pub zero: bool,
}

#[derive(Debug, Clone)]
pub struct HasOneConstraint {
    pub field: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AddressConstraint {
    pub expression: String,
    pub error: Option<String>,
}

// ─── Main entry point ─────────────────────────────────────────────────────────

pub fn parse_context_accounts_from_file(
    file_path: &str,
) -> Result<Vec<ParsedAccountsStruct>, ParserError> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| Report::new(ParserError).attach_printable(format!("Failed to read file {}: {}", file_path, e)))?;
    parse_context_accounts_from_source(&content)
}

pub fn parse_context_accounts_from_source(
    source: &str,
) -> Result<Vec<ParsedAccountsStruct>, ParserError> {
    let file = syn::parse_file(source)
        .map_err(|e| Report::new(ParserError).attach_printable(format!("Failed to parse Rust source: {}", e)))?;

    let mut results = Vec::new();

    for item in &file.items {
        if let syn::Item::Struct(item_struct) = item {
            if has_derive_accounts(&item_struct.attrs) {
                let parsed = parse_accounts_struct(item_struct)?;
                results.push(parsed);
            }
        }
    }

    Ok(results)
}

// ─── Struct-level parsing ─────────────────────────────────────────────────────

fn has_derive_accounts(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("derive") {
            if let Ok(nested) = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            ) {
                for path in &nested {
                    if path.is_ident("Accounts") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn parse_accounts_struct(
    item_struct: &syn::ItemStruct,
) -> Result<ParsedAccountsStruct, ParserError> {
    let name = item_struct.ident.to_string();
    let mut accounts = Vec::new();

    if let syn::Fields::Named(fields) = &item_struct.fields {
        for field in &fields.named {
            let field_name = field
                .ident
                .as_ref()
                .map(|i| i.to_string())
                .unwrap_or_default();

            let attributes = parse_account_attributes(&field.attrs)?;
            let type_info = parse_field_type(&field.ty);

            accounts.push(ParsedAccount {
                field_name,
                account_wrapper_name: type_info.wrapper_name,
                account_struct_name: type_info.struct_name,
                lifetime_name: type_info.lifetime_name,
                is_boxed: type_info.is_boxed,
                attributes,
                raw_type: quote_type(&field.ty),
            });
        }
    }

    Ok(ParsedAccountsStruct { name, accounts })
}

// ─── Attribute parsing ────────────────────────────────────────────────────────

fn parse_account_attributes(attrs: &[syn::Attribute]) -> Result<ParsedAccountAttributes, ParserError> {
    let mut result = ParsedAccountAttributes::default();

    for attr in attrs {
        if !attr.path().is_ident("account") {
            continue;
        }

        // Get the raw token stream inside the parentheses
        let tokens = match &attr.meta {
            syn::Meta::List(list) => list.tokens.clone(),
            _ => continue,
        };

        let token_str = normalize_token_str(&tokens.to_string());
        parse_account_attr_tokens(&token_str, &mut result)?;
    }

    result.is_pda = !result.seeds.is_empty();

    Ok(result)
}

fn parse_account_attr_tokens(
    token_str: &str,
    result: &mut ParsedAccountAttributes,
) -> Result<(), ParserError> {
    // We parse the comma-separated items manually, respecting brackets and parens.
    let items = split_top_level_commas(token_str);

    for item in &items {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }

        // Simple flags
        if item == "mut" {
            result.is_mut = true;
            continue;
        }
        if item == "init" || item == "init_if_necessary" {
            result.is_init = true;
            continue;
        }
        if item == "zero" {
            result.zero = true;
            continue;
        }

        // Key-value pairs
        if let Some((key, value)) = split_key_value(item) {
            match key {
                "seeds" => {
                    result.seeds = parse_seeds_value(value);
                }
                "bump" => {
                    result.bump = Some(value.to_string());
                }
                "close" => {
                    result.is_close = true;
                    result.close_target = Some(value.to_string());
                }
                "payer" => {
                    result.payer = Some(value.to_string());
                }
                "space" => {
                    result.space = Some(value.to_string());
                }
                "owner" => {
                    result.owner = Some(value.to_string());
                }
                "constraint" => {
                    result.constraints.push(value.to_string());
                }
                "realloc" => {
                    result.realloc = Some(value.to_string());
                }
                "has_one" => {
                    let (field, error) = split_at_sign(value);
                    result.has_one.push(HasOneConstraint { field, error });
                }
                "address" => {
                    let (expression, error) = split_at_sign(value);
                    result.address = Some(AddressConstraint { expression, error });
                }
                _ => {
                    // Handle namespaced keys like "token :: mint", "realloc :: payer", "realloc :: zero"
                    let normalized = key.replace(' ', "");
                    if normalized == "token::mint" {
                        result.token_mint = Some(value.to_string());
                    } else if normalized == "token::authority" {
                        result.token_authority = Some(value.to_string());
                    } else if normalized == "associated_token::mint" {
                        result.associated_token_mint = Some(value.to_string());
                    } else if normalized == "associated_token::authority" {
                        result.associated_token_authority = Some(value.to_string());
                    } else if normalized == "associated_token::token_program" {
                        result.associated_token_token_program = Some(value.to_string());
                    } else if normalized == "realloc::payer" {
                        result.payer = Some(value.to_string());
                    } else if normalized == "realloc::zero" {
                        result.zero = value.trim() == "true";
                    } else if normalized == "rent_exempt" {
                        result.rent_exempt = value.trim() == "enforce";
                    } else if normalized == "mint::decimals" || normalized == "mint::authority" {
                        result.constraints.push(format!("{} = {}", normalized, value));
                    } else {
                        // Unknown key-value, store as generic constraint
                        result.constraints.push(format!("{} = {}", key, value));
                    }
                }
            }
            continue;
        }

        // Handle bare "rent_exempt = enforce" - already handled above via key-value
        // Handle "bump" without value
        if item == "bump" {
            result.bump = Some(String::new());
            continue;
        }

        // If it's a standalone identifier we don't recognize, store as constraint
        if item == "rent_exempt" {
            result.rent_exempt = true;
            continue;
        }
    }

    Ok(())
}

/// Normalize token stream string by removing extra spaces around `.`, `::`, `(`, `)`, `!`, `&`, `<`, `>`.
/// proc_macro2 TokenStream::to_string() inserts spaces like `state . key ()` → `state.key()`.
fn normalize_token_str(s: &str) -> String {
    let s = s.trim().to_string();
    // Remove spaces around `.`
    let s = regex::Regex::new(r"\s*\.\s*").unwrap().replace_all(&s, ".").to_string();
    // Remove spaces around `::`
    let s = regex::Regex::new(r"\s*::\s*").unwrap().replace_all(&s, "::").to_string();
    // Remove space before `(`
    let s = regex::Regex::new(r"\s+\(").unwrap().replace_all(&s, "(").to_string();
    // Remove space after `(`
    let s = regex::Regex::new(r"\(\s+").unwrap().replace_all(&s, "(").to_string();
    // Remove space before `)`
    let s = regex::Regex::new(r"\s+\)").unwrap().replace_all(&s, ")").to_string();
    // Remove space after `&`
    let s = regex::Regex::new(r"&\s+").unwrap().replace_all(&s, "&").to_string();
    // Remove space before `[`
    let s = regex::Regex::new(r"\s+\[").unwrap().replace_all(&s, "[").to_string();
    // Remove space after `[`
    let s = regex::Regex::new(r"\[\s+").unwrap().replace_all(&s, "[").to_string();
    // Remove space before `]`
    let s = regex::Regex::new(r"\s+\]").unwrap().replace_all(&s, "]").to_string();
    // Remove space before `!`
    let s = regex::Regex::new(r"\s+!").unwrap().replace_all(&s, "!").to_string();
    s
}

/// Split a string at top-level commas, respecting brackets `[]`, parens `()`, and braces `{}`.
fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for ch in s.chars() {
        match ch {
            '(' | '[' | '{' | '<' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' | '}' | '>' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                items.push(current.clone());
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.trim().is_empty() {
        items.push(current);
    }
    items
}

/// Split "key = value" at the first `=` sign, but NOT at `==`, `!=`, `<=`, `>=`.
fn split_key_value(s: &str) -> Option<(&str, &str)> {
    // Find first '=' that is not part of ==, !=, <=, >=
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' {
            // Check it's not preceded by !, <, >, = or followed by =
            let prev = if i > 0 { Some(bytes[i - 1]) } else { None };
            let next = if i + 1 < bytes.len() {
                Some(bytes[i + 1])
            } else {
                None
            };
            if prev != Some(b'!') && prev != Some(b'<') && prev != Some(b'>') && prev != Some(b'=')
                && next != Some(b'=')
            {
                let key = s[..i].trim();
                let value = s[i + 1..].trim();
                if !key.is_empty() && !value.is_empty() {
                    return Some((key, value));
                }
            }
        }
        i += 1;
    }
    None
}

/// Split "expr @ ErrorType::Variant" at the `@` sign.
fn split_at_sign(s: &str) -> (String, Option<String>) {
    if let Some(pos) = s.find('@') {
        let expr = s[..pos].trim().to_string();
        let error = s[pos + 1..].trim().to_string();
        (expr, Some(error))
    } else {
        (s.trim().to_string(), None)
    }
}

/// Parse seeds from the value part of "seeds = [expr1, expr2, ...]"
fn parse_seeds_value(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    // Remove surrounding brackets
    let inner = if trimmed.starts_with('[') && trimmed.ends_with(']') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    split_top_level_commas(inner)
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ─── Type parsing ─────────────────────────────────────────────────────────────

struct TypeInfo {
    wrapper_name: String,
    struct_name: String,
    lifetime_name: String,
    is_boxed: bool,
}

fn parse_field_type(ty: &syn::Type) -> TypeInfo {
    let mut is_boxed = false;
    let inner_ty = unwrap_box(ty, &mut is_boxed);

    match inner_ty {
        syn::Type::Path(type_path) => {
            let seg = type_path.path.segments.last().unwrap();
            let wrapper_name = seg.ident.to_string();

            match &seg.arguments {
                syn::PathArguments::AngleBracketed(args) => {
                    let mut lifetime_name = String::new();
                    let mut struct_name = String::new();

                    for arg in &args.args {
                        match arg {
                            syn::GenericArgument::Lifetime(lt) => {
                                if lifetime_name.is_empty() {
                                    lifetime_name = format!("'{}", lt.ident);
                                }
                            }
                            syn::GenericArgument::Type(inner) => {
                                if struct_name.is_empty() {
                                    struct_name = quote_type(inner);
                                }
                            }
                            _ => {}
                        }
                    }

                    // If no separate struct name (e.g., Signer<'info>), struct = wrapper
                    if struct_name.is_empty() {
                        struct_name = wrapper_name.clone();
                    }

                    TypeInfo {
                        wrapper_name,
                        struct_name,
                        lifetime_name,
                        is_boxed,
                    }
                }
                syn::PathArguments::None => TypeInfo {
                    wrapper_name: wrapper_name.clone(),
                    struct_name: wrapper_name,
                    lifetime_name: String::new(),
                    is_boxed,
                },
                syn::PathArguments::Parenthesized(_) => TypeInfo {
                    wrapper_name: wrapper_name.clone(),
                    struct_name: wrapper_name,
                    lifetime_name: String::new(),
                    is_boxed,
                },
            }
        }
        _ => TypeInfo {
            wrapper_name: quote_type(inner_ty),
            struct_name: quote_type(inner_ty),
            lifetime_name: String::new(),
            is_boxed,
        },
    }
}

fn unwrap_box<'a>(ty: &'a syn::Type, is_boxed: &mut bool) -> &'a syn::Type {
    if let syn::Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if seg.ident == "Box" {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        *is_boxed = true;
                        return inner;
                    }
                }
            }
        }
    }
    ty
}

fn quote_type(ty: &syn::Type) -> String {
    use proc_macro2::TokenStream;
    use quote::ToTokens;
    let mut tokens = TokenStream::new();
    ty.to_tokens(&mut tokens);
    tokens.to_string()
}

// ─── SolanaAccountType detection ──────────────────────────────────────────────

impl ParsedAccount {
    /// Determine the SolanaAccountType from parsed type info.
    /// `solana_account_names` should be a set of known program state account struct names
    /// (from BatMetadata structs_source_code with StructMetadataType::SolanaAccount).
    pub fn determine_solana_account_type(
        &self,
        solana_account_names: &[String],
    ) -> SolanaAccountType {
        match self.account_wrapper_name.as_str() {
            "Signer" => SolanaAccountType::Signer,
            "UncheckedAccount" | "AccountInfo" => SolanaAccountType::UncheckedAccount,
            "SystemAccount" => SolanaAccountType::SystemAccount,
            _ => {
                // Check struct name for known types
                if self.account_struct_name == "TokenAccount" {
                    return SolanaAccountType::TokenAccount;
                }
                if self.account_struct_name == "Mint" {
                    return SolanaAccountType::Mint;
                }
                // Check if the struct is a known solana account from metadata
                if solana_account_names.iter().any(|name| *name == self.account_struct_name) {
                    return SolanaAccountType::ProgramStateAccount;
                }
                SolanaAccountType::Other
            }
        }
    }
}

// ─── Conversion to CAAccountParser ────────────────────────────────────────────

impl ParsedAccount {
    pub fn to_ca_account_parser(
        &self,
        solana_account_type: SolanaAccountType,
        content: &str,
    ) -> CAAccountParser {
        let rent_exemption_account = self
            .attributes
            .payer
            .clone()
            .or_else(|| self.attributes.close_target.clone())
            .unwrap_or_default();

        let mut validations = Vec::new();

        for ho in &self.attributes.has_one {
            let val = if let Some(err) = &ho.error {
                format!("has_one = {} @ {}", ho.field, err)
            } else {
                format!("has_one = {}", ho.field)
            };
            validations.push(val);
        }

        if let Some(addr) = &self.attributes.address {
            let val = if let Some(err) = &addr.error {
                format!("address = {} @ {}", addr.expression, err)
            } else {
                format!("address = {}", addr.expression)
            };
            validations.push(val);
        }

        // token::authority as validation
        if let Some(ref auth) = self.attributes.token_authority {
            validations.push(format!("token::authority = {}", auth));
        }

        // associated_token constraints as validations
        if let Some(ref mint) = self.attributes.associated_token_mint {
            validations.push(format!("associated_token::mint = {}", mint));
        }
        if let Some(ref auth) = self.attributes.associated_token_authority {
            validations.push(format!("associated_token::authority = {}", auth));
        }
        if let Some(ref tp) = self.attributes.associated_token_token_program {
            validations.push(format!("associated_token::token_program = {}", tp));
        }

        for constraint in &self.attributes.constraints {
            validations.push(format!("constraint = {}", constraint));
        }

        // Use associated_token::mint as fallback for token_mint
        let token_mint = self.attributes.token_mint.clone()
            .or_else(|| self.attributes.associated_token_mint.clone());

        CAAccountParser {
            content: content.to_string(),
            solana_account_type,
            account_struct_name: self.account_struct_name.clone(),
            account_wrapper_name: self.account_wrapper_name.clone(),
            lifetime_name: self.lifetime_name.clone(),
            account_name: self.field_name.clone(),
            is_pda: self.attributes.is_pda,
            is_init: self.attributes.is_init,
            is_mut: self.attributes.is_mut,
            is_close: self.attributes.is_close,
            seeds: self.attributes.seeds.clone(),
            rent_exemption_account,
            validations,
            owner: self.attributes.owner.clone(),
            token_mint,
            space: self.attributes.space.clone(),
            rent_exempt: self.attributes.rent_exempt,
            realloc: self.attributes.realloc.clone(),
            bump: self.attributes.bump.clone(),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_signer() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Initialize<'info> {
                #[account(mut)]
                pub authority: Signer<'info>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Initialize");
        assert_eq!(result[0].accounts.len(), 1);
        let acc = &result[0].accounts[0];
        assert_eq!(acc.field_name, "authority");
        assert_eq!(acc.account_wrapper_name, "Signer");
        assert!(acc.attributes.is_mut);
        assert!(!acc.attributes.is_init);
    }

    #[test]
    fn test_pda_with_seeds() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Deposit<'info> {
                #[account(
                    mut,
                    seeds = [
                        state.key().to_bytes().as_ref(),
                        b"vault",
                    ],
                    bump = state.vault_bump,
                )]
                pub vault: Account<'info, TokenAccount>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert!(acc.attributes.is_pda);
        assert_eq!(acc.attributes.seeds.len(), 2);
        assert!(acc.attributes.seeds[0].contains("state.key()"));
        assert!(acc.attributes.seeds[1].contains("b\"vault\"") || acc.attributes.seeds[1].contains("b \"vault\""));
        assert!(acc.attributes.bump.is_some());
    }

    #[test]
    fn test_boxed_account() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Transfer<'info> {
                #[account(mut)]
                pub state: Box<Account<'info, State>>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert!(acc.is_boxed);
        assert_eq!(acc.account_wrapper_name, "Account");
        assert_eq!(acc.account_struct_name, "State");
        assert_eq!(acc.lifetime_name, "'info");
    }

    #[test]
    fn test_has_one_with_error() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Withdraw<'info> {
                #[account(
                    mut,
                    has_one = authority @ ErrorCode::Unauthorized,
                )]
                pub vault: Account<'info, Vault>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert_eq!(acc.attributes.has_one.len(), 1);
        assert_eq!(acc.attributes.has_one[0].field, "authority");
        assert!(acc.attributes.has_one[0].error.is_some());
    }

    #[test]
    fn test_init_account() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Create<'info> {
                #[account(
                    init,
                    payer = authority,
                    space = 8 + 32 + 8,
                    seeds = [b"state", authority.key().as_ref()],
                    bump,
                )]
                pub state: Account<'info, State>,
                #[account(mut)]
                pub authority: Signer<'info>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let state = &result[0].accounts[0];
        assert!(state.attributes.is_init);
        assert!(state.attributes.is_pda);
        assert_eq!(state.attributes.payer.as_deref(), Some("authority"));
        assert!(state.attributes.space.is_some());
        assert_eq!(state.attributes.seeds.len(), 2);
        assert!(state.attributes.bump.is_some());
    }

    #[test]
    fn test_token_mint_constraint() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Stake<'info> {
                #[account(
                    mut,
                    token::mint = mint,
                    token::authority = authority,
                )]
                pub token_account: Account<'info, TokenAccount>,
                pub mint: Account<'info, Mint>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert_eq!(acc.attributes.token_mint.as_deref(), Some("mint"));
    }

    #[test]
    fn test_owner_constraint() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Check<'info> {
                #[account(
                    owner = token_program.key(),
                )]
                pub token_account: AccountInfo<'info>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert_eq!(acc.attributes.owner.as_deref(), Some("token_program.key()"));
    }

    #[test]
    fn test_close_account() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Close<'info> {
                #[account(
                    mut,
                    close = recipient,
                )]
                pub state: Account<'info, State>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert!(acc.attributes.is_close);
        assert_eq!(acc.attributes.close_target.as_deref(), Some("recipient"));
    }

    #[test]
    fn test_system_account() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Pay<'info> {
                pub recipient: SystemAccount<'info>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert_eq!(acc.account_wrapper_name, "SystemAccount");
        assert_eq!(acc.account_struct_name, "SystemAccount");
        assert_eq!(acc.lifetime_name, "'info");
    }

    #[test]
    fn test_realloc_constraint() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Resize<'info> {
                #[account(
                    mut,
                    realloc = 8 + 32 + new_size,
                    realloc::payer = authority,
                    realloc::zero = true,
                )]
                pub state: Account<'info, State>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert!(acc.attributes.realloc.is_some());
        assert_eq!(acc.attributes.payer.as_deref(), Some("authority"));
    }

    #[test]
    fn test_rent_exempt() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Init<'info> {
                #[account(
                    mut,
                    rent_exempt = enforce,
                )]
                pub state: Account<'info, State>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert!(acc.attributes.rent_exempt);
    }

    #[test]
    fn test_multiple_has_one() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Complex<'info> {
                #[account(
                    mut,
                    has_one = authority,
                    has_one = fleet_ships @ GameError::FleetNotFound,
                    constraint = state.is_active == true @ GameError::Inactive,
                )]
                pub state: Account<'info, GameState>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert_eq!(acc.attributes.has_one.len(), 2);
        assert_eq!(acc.attributes.constraints.len(), 1);
    }

    #[test]
    fn test_address_constraint() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Verify<'info> {
                #[account(
                    address = PROGRAM_ID @ ErrorType::InvalidProgram,
                )]
                pub program: AccountInfo<'info>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert!(acc.attributes.address.is_some());
        let addr = acc.attributes.address.as_ref().unwrap();
        assert_eq!(addr.expression, "PROGRAM_ID");
        assert!(addr.error.is_some());
    }

    #[test]
    fn test_no_account_attr() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Simple<'info> {
                pub system_program: Program<'info, System>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        assert!(!acc.attributes.is_mut);
        assert!(!acc.attributes.is_init);
        assert!(!acc.attributes.is_pda);
        assert_eq!(acc.account_wrapper_name, "Program");
    }

    #[test]
    fn test_conversion_to_ca_parser() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Test<'info> {
                #[account(
                    mut,
                    seeds = [b"test"],
                    bump,
                    has_one = authority,
                    owner = crate::ID,
                )]
                pub state: Account<'info, State>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let acc = &result[0].accounts[0];
        let ca = acc.to_ca_account_parser(SolanaAccountType::ProgramStateAccount, "test content");
        assert!(ca.is_mut);
        assert!(ca.is_pda);
        assert_eq!(ca.seeds.len(), 1);
        assert_eq!(ca.account_name, "state");
        assert!(ca.owner.is_some());
        assert!(!ca.validations.is_empty());
    }

    #[test]
    fn test_marinade_real_patterns() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct Claim<'info> {
                #[account(
                    mut,
                    has_one = msol_mint,
                    has_one = operational_sol_account,
                )]
                pub state: Box<Account<'info, State>>,

                #[account(
                    mut,
                    address = state.validator_system.validator_list.account,
                )]
                pub validator_list: UncheckedAccount<'info>,

                #[account(
                    address = ticket_account.beneficiary @ MarinadeError::WrongBeneficiary,
                )]
                pub transfer_sol_to: SystemAccount<'info>,

                #[account(
                    mut,
                    address = state.liq_pool.lp_mint,
                    owner = token_program.key(),
                )]
                pub lp_mint: Account<'info, Mint>,

                #[account(
                    mut,
                    token::mint = state.msol_mint,
                    token::authority = msol_mint_authority,
                )]
                pub mint_to: Account<'info, TokenAccount>,

                #[account(
                    mut,
                    seeds = [
                        state.key().as_ref(),
                        b"vault",
                        validator_index.to_le_bytes().as_ref(),
                    ],
                    bump,
                )]
                pub stake_account: Account<'info, TokenAccount>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        assert_eq!(result[0].name, "Claim");
        assert_eq!(result[0].accounts.len(), 6);

        // state: Box<Account<'info, State>> with has_one
        let state = &result[0].accounts[0];
        assert!(state.is_boxed);
        assert_eq!(state.account_wrapper_name, "Account");
        assert_eq!(state.account_struct_name, "State");
        assert!(state.attributes.is_mut);
        assert_eq!(state.attributes.has_one.len(), 2);
        assert_eq!(state.attributes.has_one[0].field, "msol_mint");
        assert_eq!(state.attributes.has_one[1].field, "operational_sol_account");

        // validator_list: address with method chain
        let vl = &result[0].accounts[1];
        assert_eq!(vl.account_wrapper_name, "UncheckedAccount");
        let addr = vl.attributes.address.as_ref().unwrap();
        assert_eq!(addr.expression, "state.validator_system.validator_list.account");
        assert!(addr.error.is_none());

        // transfer_sol_to: address with @ error
        let ts = &result[0].accounts[2];
        assert_eq!(ts.account_wrapper_name, "SystemAccount");
        let addr = ts.attributes.address.as_ref().unwrap();
        assert_eq!(addr.expression, "ticket_account.beneficiary");
        assert_eq!(addr.error.as_deref(), Some("MarinadeError::WrongBeneficiary"));

        // lp_mint: owner constraint
        let lp = &result[0].accounts[3];
        assert!(lp.attributes.owner.is_some());
        assert_eq!(lp.attributes.owner.as_deref(), Some("token_program.key()"));

        // mint_to: token::mint and token::authority
        let mt = &result[0].accounts[4];
        assert_eq!(mt.attributes.token_mint.as_deref(), Some("state.msol_mint"));
        assert_eq!(mt.attributes.token_authority.as_deref(), Some("msol_mint_authority"));

        // stake_account: complex seeds with method chains
        let sa = &result[0].accounts[5];
        assert!(sa.attributes.is_pda);
        assert_eq!(sa.attributes.seeds.len(), 3);
        assert!(sa.attributes.seeds[0].contains("state.key()"));
        assert!(sa.attributes.seeds[1].contains("b\"vault\"") || sa.attributes.seeds[1].contains("b \"vault\""));
        assert!(sa.attributes.seeds[2].contains("validator_index.to_le_bytes()"));
        assert!(sa.attributes.bump.is_some());
    }

    #[test]
    fn test_associated_token_account() {
        let source = r#"
            use anchor_lang::prelude::*;

            #[derive(Accounts)]
            pub struct CreateAta<'info> {
                #[account(
                    init_if_necessary,
                    payer = authority,
                    associated_token::mint = mint,
                    associated_token::authority = authority,
                    associated_token::token_program = token_program,
                )]
                pub ata: Account<'info, TokenAccount>,

                pub mint: Account<'info, Mint>,
                #[account(mut)]
                pub authority: Signer<'info>,
                pub token_program: Program<'info, Token>,
            }
        "#;
        let result = parse_context_accounts_from_source(source).unwrap();
        let ata = &result[0].accounts[0];

        assert!(ata.attributes.is_init);
        assert_eq!(ata.attributes.payer.as_deref(), Some("authority"));
        assert_eq!(ata.attributes.associated_token_mint.as_deref(), Some("mint"));
        assert_eq!(ata.attributes.associated_token_authority.as_deref(), Some("authority"));
        assert_eq!(ata.attributes.associated_token_token_program.as_deref(), Some("token_program"));

        // Conversion: associated_token::mint should fill token_mint as fallback
        let ca = ata.to_ca_account_parser(SolanaAccountType::TokenAccount, "");
        assert_eq!(ca.token_mint.as_deref(), Some("mint"));
        assert!(ca.validations.iter().any(|v| v.contains("associated_token::mint")));
        assert!(ca.validations.iter().any(|v| v.contains("associated_token::authority")));
        assert!(ca.validations.iter().any(|v| v.contains("associated_token::token_program")));
    }
}
