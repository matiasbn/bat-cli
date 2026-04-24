use std::collections::HashSet;
use std::fs;

pub struct FileClassification {
    pub context_accounts_names: HashSet<String>,
    pub solana_account_names: HashSet<String>,
    pub entrypoint_function_names: HashSet<String>,
    pub pinocchio_context_accounts_names: HashSet<String>,
}

pub fn classify_file_from_path(path: &str) -> FileClassification {
    let content = fs::read_to_string(path).unwrap_or_default();
    classify_file(&content)
}

pub fn classify_file(file_content: &str) -> FileClassification {
    let mut classification = FileClassification {
        context_accounts_names: HashSet::new(),
        solana_account_names: HashSet::new(),
        entrypoint_function_names: HashSet::new(),
        pinocchio_context_accounts_names: HashSet::new(),
    };

    let Ok(file) = syn::parse_file(file_content) else {
        return classification;
    };

    for item in &file.items {
        match item {
            syn::Item::Struct(item_struct) => {
                if has_derive_accounts(item_struct) {
                    classification
                        .context_accounts_names
                        .insert(item_struct.ident.to_string());
                } else if has_derive_codama_account(item_struct) {
                    classification
                        .solana_account_names
                        .insert(item_struct.ident.to_string());
                } else if has_account_attribute(item_struct) {
                    classification
                        .solana_account_names
                        .insert(item_struct.ident.to_string());
                }
            }
            syn::Item::Mod(item_mod) => {
                if has_program_attribute(item_mod) {
                    extract_entrypoint_functions(item_mod, &mut classification);
                }
            }
            syn::Item::Impl(item_impl) => {
                // Detect `impl TryFrom<&[AccountView]> for SomeStruct`
                if let Some(name) = extract_pinocchio_context_accounts(item_impl) {
                    classification
                        .pinocchio_context_accounts_names
                        .insert(name);
                }
            }
            syn::Item::Fn(item_fn) => {
                // Detect `pub fn process(accounts: &[AccountView], ...)` as Pinocchio entry point
                if is_pinocchio_entrypoint(item_fn) {
                    classification
                        .entrypoint_function_names
                        .insert(item_fn.sig.ident.to_string());
                }
            }
            _ => {}
        }
    }

    classification
}

/// Finds the `Context<T>` type parameter for a given entrypoint function name
/// by parsing the `#[program]` module with syn. Returns `Some(T)` if found.
pub fn get_context_type_for_entrypoint(
    file_content: &str,
    entrypoint_name: &str,
) -> Option<String> {
    let file = syn::parse_file(file_content).ok()?;
    for item in &file.items {
        if let syn::Item::Mod(item_mod) = item {
            if !has_program_attribute(item_mod) {
                continue;
            }
            let Some((_, items)) = &item_mod.content else {
                continue;
            };
            for inner in items {
                if let syn::Item::Fn(item_fn) = inner {
                    if item_fn.sig.ident != entrypoint_name {
                        continue;
                    }
                    // Look for the Context<T> parameter
                    for arg in &item_fn.sig.inputs {
                        if let syn::FnArg::Typed(pat_type) = arg {
                            if let Some(ctx_type) = extract_context_type(&pat_type.ty) {
                                return Some(ctx_type);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Extracts `T` from `Context<'_, T>` or `Context<T>` type.
fn extract_context_type(ty: &syn::Type) -> Option<String> {
    use quote::ToTokens;
    if let syn::Type::Path(type_path) = ty {
        let segment = type_path.path.segments.last()?;
        if segment.ident != "Context" {
            return None;
        }
        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
            // The last generic argument that is a Type (not a lifetime) is T
            for arg in args.args.iter().rev() {
                if let syn::GenericArgument::Type(inner_ty) = arg {
                    let ty_str = inner_ty.to_token_stream().to_string();
                    return Some(
                        crate::batbelt::parser::function_parser::normalize_generic_type(&ty_str),
                    );
                }
            }
        }
    }
    None
}

fn has_derive_codama_account(item: &syn::ItemStruct) -> bool {
    item.attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let Ok(nested) = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
        ) else {
            return false;
        };
        nested.iter().any(|path| path.is_ident("CodamaAccount"))
    })
}

/// Extracts the self type name from `impl TryFrom<&[AccountView]> for T`.
fn extract_pinocchio_context_accounts(item_impl: &syn::ItemImpl) -> Option<String> {
    use quote::ToTokens;
    // Must be a trait impl
    let (_, trait_path, _) = item_impl.trait_.as_ref()?;
    let last_seg = trait_path.segments.last()?;
    if last_seg.ident != "TryFrom" {
        return None;
    }
    // Check that the generic argument contains AccountView slice
    if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
        let has_account_view = args.args.iter().any(|arg| {
            let s = arg.to_token_stream().to_string();
            s.contains("AccountView")
        });
        if !has_account_view {
            return None;
        }
    } else {
        return None;
    }
    // Extract self type name
    if let syn::Type::Path(type_path) = &*item_impl.self_ty {
        let name = type_path.path.segments.last()?.ident.to_string();
        return Some(name);
    }
    None
}

/// Detects `pub fn process(accounts: &[AccountView], ...)` as a Pinocchio entry point.
fn is_pinocchio_entrypoint(item_fn: &syn::ItemFn) -> bool {
    use quote::ToTokens;
    if item_fn.sig.ident != "process" {
        return false;
    }
    // Must be pub
    if !matches!(item_fn.vis, syn::Visibility::Public(_)) {
        return false;
    }
    // Check if first non-self param contains &[AccountView]
    for arg in &item_fn.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty_str = pat_type.ty.to_token_stream().to_string();
            if ty_str.contains("AccountView") {
                return true;
            }
        }
    }
    false
}

fn has_derive_accounts(item: &syn::ItemStruct) -> bool {
    item.attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let Ok(nested) = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
        ) else {
            return false;
        };
        nested.iter().any(|path| path.is_ident("Accounts"))
    })
}

fn has_account_attribute(item: &syn::ItemStruct) -> bool {
    item.attrs
        .iter()
        .any(|attr| attr.path().is_ident("account"))
}

fn has_program_attribute(item: &syn::ItemMod) -> bool {
    item.attrs
        .iter()
        .any(|attr| attr.path().is_ident("program"))
}

fn extract_entrypoint_functions(item_mod: &syn::ItemMod, classification: &mut FileClassification) {
    let Some((_, items)) = &item_mod.content else {
        return;
    };
    for item in items {
        if let syn::Item::Fn(item_fn) = item {
            classification
                .entrypoint_function_names
                .insert(item_fn.sig.ident.to_string());
        }
    }
}
