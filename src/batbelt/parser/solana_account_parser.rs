use crate::batbelt::metadata::structs_source_code_metadata::StructMetadataType;
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser};
use crate::batbelt::parser::{ParserError, ParserResult};
use error_stack::{IntoReport, Report, Result, ResultExt};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    PartialEq,
    Clone,
    Copy,
    strum_macros::Display,
    strum_macros::EnumIter,
    Serialize,
    Deserialize,
)]
pub enum SolanaAccountType {
    TokenAccount,
    Mint,
    Signer,
    UncheckedAccount,
    SystemAccount,
    ProgramStateAccount,
    Other,
}

impl SolanaAccountType {
    pub fn from_context_account_content(
        context_account_content: &str,
    ) -> Result<Self, ParserError> {
        let last_line = context_account_content.lines().last().unwrap();

        // Try syn: parse last line as a struct field to extract the wrapper type name
        if let Some(wrapper_name) = Self::extract_wrapper_type_via_syn(last_line) {
            match wrapper_name.as_str() {
                "Signer" => return Ok(Self::Signer),
                "UncheckedAccount" => return Ok(Self::UncheckedAccount),
                "SystemAccount" => return Ok(Self::SystemAccount),
                "TokenAccount" => return Ok(Self::TokenAccount),
                "Mint" => return Ok(Self::Mint),
                _ => {
                    // Check if it matches a known Solana account struct
                    let solana_accounts_metadata = BatMetadata::read_metadata()
                        .change_context(ParserError)?
                        .source_code
                        .structs_source_code;
                    if solana_accounts_metadata.iter().any(|sa| {
                        sa.struct_type == StructMetadataType::SolanaAccount
                            && sa.name == wrapper_name
                    }) {
                        return Ok(Self::ProgramStateAccount);
                    }
                    // Also check inner type args for known account types
                    if let Some(inner) = Self::extract_inner_type_via_syn(last_line) {
                        if solana_accounts_metadata.iter().any(|sa| {
                            sa.struct_type == StructMetadataType::SolanaAccount && sa.name == inner
                        }) {
                            return Ok(Self::ProgramStateAccount);
                        }
                    }
                    return Ok(Self::Other);
                }
            }
        }

        // Fallback: string matching
        if last_line.contains("Signer<") {
            return Ok(Self::Signer);
        }
        if last_line.contains(&Self::UncheckedAccount.to_string()) {
            return Ok(Self::UncheckedAccount);
        }
        if last_line.contains(&Self::SystemAccount.to_string()) {
            return Ok(Self::SystemAccount);
        }
        if last_line.contains(&Self::TokenAccount.to_string()) {
            return Ok(Self::TokenAccount);
        }
        if last_line.contains(&Self::Mint.to_string()) {
            return Ok(Self::Mint);
        }

        let mut solana_accounts_metadata = BatMetadata::read_metadata()
            .change_context(ParserError)?
            .source_code
            .structs_source_code
            .into_iter()
            .filter(|s_metda| s_metda.struct_type == StructMetadataType::SolanaAccount);
        if solana_accounts_metadata.any(|solana_account| last_line.contains(&solana_account.name)) {
            return Ok(Self::ProgramStateAccount);
        }

        Ok(Self::Other)
    }

    /// Extracts the outermost type name from a struct field line via syn.
    /// E.g. "pub foo: Account<'info, MyStruct>" → "Account"
    fn extract_wrapper_type_via_syn(field_line: &str) -> Option<String> {
        let field_str = format!(
            "struct __Tmp {{ {} }}",
            field_line.trim().trim_end_matches(',')
        );
        let item_struct = syn::parse_str::<syn::ItemStruct>(&field_str).ok()?;
        let field = item_struct.fields.iter().next()?;
        Self::outermost_type_name(&field.ty)
    }

    /// Extracts the last generic type argument from a field line.
    /// E.g. "pub foo: Account<'info, MyStruct>" → "MyStruct"
    fn extract_inner_type_via_syn(field_line: &str) -> Option<String> {
        let field_str = format!(
            "struct __Tmp {{ {} }}",
            field_line.trim().trim_end_matches(',')
        );
        let item_struct = syn::parse_str::<syn::ItemStruct>(&field_str).ok()?;
        let field = item_struct.fields.iter().next()?;
        Self::last_type_arg(&field.ty)
    }

    fn outermost_type_name(ty: &syn::Type) -> Option<String> {
        if let syn::Type::Path(type_path) = ty {
            let segment = type_path.path.segments.last()?;
            return Some(segment.ident.to_string());
        }
        None
    }

    fn last_type_arg(ty: &syn::Type) -> Option<String> {
        use quote::ToTokens;
        if let syn::Type::Path(type_path) = ty {
            let segment = type_path.path.segments.last()?;
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                for arg in args.args.iter().rev() {
                    if let syn::GenericArgument::Type(inner_ty) = arg {
                        return Some(
                            crate::batbelt::parser::function_parser::normalize_generic_type(
                                &inner_ty.to_token_stream().to_string(),
                            ),
                        );
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct SolanaAccountParserAccount {
    pub account_name: String,
    pub account_type: String,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct SolanaAccountParser {
    pub solana_account_type: SolanaAccountType,
    pub account_struct_name: String,
    pub accounts: Vec<SolanaAccountParserAccount>,
}

impl SolanaAccountParser {
    pub fn new_from_struct_name_and_solana_account_type(
        account_struct_name: String,
        solana_account_type: SolanaAccountType,
    ) -> ParserResult<Self> {
        let mut new_solana_account_parser = Self {
            solana_account_type,
            account_struct_name,
            accounts: vec![],
        };
        match solana_account_type {
            SolanaAccountType::ProgramStateAccount => {
                new_solana_account_parser.parse_program_state_account()?;
            }
            _ => unimplemented!(),
        }
        Ok(new_solana_account_parser)
    }

    fn parse_program_state_account(&mut self) -> ParserResult<()> {
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        match bat_metadata
            .source_code
            .structs_source_code
            .into_iter()
            .find(|struct_sc| {
                struct_sc.struct_type == StructMetadataType::SolanaAccount
                    && struct_sc.name == self.account_struct_name
            }) {
            None => Err(Report::new(ParserError).attach_printable(format!(
                "No Solana Account was found with name {}",
                self.account_struct_name
            ))),
            Some(struct_metadata) => {
                let struct_metadata_content = struct_metadata
                    .to_source_code_parser(None)
                    .get_source_code_content();

                // Try syn first: parse as ItemStruct and extract fields
                if let Ok(item_struct) = syn::parse_str::<syn::ItemStruct>(&struct_metadata_content)
                {
                    use quote::ToTokens;
                    let account_vec: Vec<SolanaAccountParserAccount> = item_struct
                        .fields
                        .iter()
                        .filter_map(|field| {
                            let ident = field.ident.as_ref()?;
                            let ty_str =
                                crate::batbelt::parser::function_parser::normalize_generic_type(
                                    &field.ty.to_token_stream().to_string(),
                                );
                            Some(SolanaAccountParserAccount {
                                account_name: ident.to_string(),
                                account_type: ty_str,
                            })
                        })
                        .collect();
                    self.accounts = account_vec;
                    return Ok(());
                }

                // Fallback: regex matching
                let account_param_regex = Regex::new(r"pub [A-Za-z0-9_]+: [\w;\[\] ]+,")
                    .into_report()
                    .change_context(ParserError)?;
                let account_vec = struct_metadata_content
                    .lines()
                    .filter_map(|line| {
                        if account_param_regex.is_match(line) {
                            let mut line_split = line
                                .trim()
                                .trim_end_matches(',')
                                .trim_start_matches("pub ")
                                .split(": ");
                            Some(SolanaAccountParserAccount {
                                account_name: line_split.next().unwrap().to_string(),
                                account_type: line_split.next().unwrap().to_string(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                self.accounts = account_vec;
                Ok(())
            }
        }
    }
}
