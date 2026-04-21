use std::collections::HashSet;

pub struct FileClassification {
    pub context_accounts_names: HashSet<String>,
    pub solana_account_names: HashSet<String>,
    pub entrypoint_function_names: HashSet<String>,
}

pub fn classify_file(file_content: &str) -> FileClassification {
    let mut classification = FileClassification {
        context_accounts_names: HashSet::new(),
        solana_account_names: HashSet::new(),
        entrypoint_function_names: HashSet::new(),
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
            _ => {}
        }
    }

    classification
}

fn has_derive_accounts(item: &syn::ItemStruct) -> bool {
    item.attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let Ok(nested) =
            attr.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
        else {
            return false;
        };
        nested.iter().any(|path| path.is_ident("Accounts"))
    })
}

fn has_account_attribute(item: &syn::ItemStruct) -> bool {
    item.attrs.iter().any(|attr| attr.path().is_ident("account"))
}

fn has_program_attribute(item: &syn::ItemMod) -> bool {
    item.attrs.iter().any(|attr| attr.path().is_ident("program"))
}

fn extract_entrypoint_functions(
    item_mod: &syn::ItemMod,
    classification: &mut FileClassification,
) {
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
