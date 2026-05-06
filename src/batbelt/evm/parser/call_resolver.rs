use std::collections::HashMap;

use crate::batbelt::evm::types::{EvmContract, EvmFunction};

/// Represents a resolved function call.
#[derive(Debug, Clone)]
pub struct ResolvedCall {
    pub caller_contract: String,
    pub caller_function: String,
    pub callee_contract: String,
    pub callee_function: String,
    pub is_external: bool,
    pub is_super: bool,
}

/// Resolves function calls within and across contracts.
pub struct CallResolver<'a> {
    contracts_by_name: HashMap<String, &'a EvmContract>,
}

impl<'a> CallResolver<'a> {
    pub fn new(contracts: &'a [EvmContract]) -> Self {
        let contracts_by_name = contracts.iter().map(|c| (c.name.clone(), c)).collect();
        Self { contracts_by_name }
    }

    /// Resolve all calls in a function's body using AST-based extraction.
    /// Falls back to regex if AST parsing fails.
    pub fn resolve_calls(
        &self,
        contract_name: &str,
        function: &EvmFunction,
    ) -> Vec<ResolvedCall> {
        let body = &function.body_source;
        if body.is_empty() {
            return Vec::new();
        }

        // Try AST-based extraction first
        let call_names = extract_calls_from_source(body);

        let mut calls = Vec::new();

        for callee_name in &call_names {
            if is_builtin(callee_name) {
                continue;
            }

            // Check for external calls: "Contract.function"
            if let Some(dot_pos) = callee_name.find('.') {
                let target = &callee_name[..dot_pos];
                let method = &callee_name[dot_pos + 1..];

                if target == "super" {
                    calls.push(ResolvedCall {
                        caller_contract: contract_name.to_string(),
                        caller_function: function.name.clone(),
                        callee_contract: contract_name.to_string(),
                        callee_function: method.to_string(),
                        is_external: false,
                        is_super: true,
                    });
                } else if self.contracts_by_name.contains_key(target) {
                    calls.push(ResolvedCall {
                        caller_contract: contract_name.to_string(),
                        caller_function: function.name.clone(),
                        callee_contract: target.to_string(),
                        callee_function: method.to_string(),
                        is_external: true,
                        is_super: false,
                    });
                }
                continue;
            }

            // Internal call
            if let Some(resolved) =
                self.resolve_single_call(contract_name, &function.name, callee_name)
            {
                calls.push(resolved);
            }
        }

        calls
    }

    fn resolve_single_call(
        &self,
        contract_name: &str,
        caller_function: &str,
        callee_name: &str,
    ) -> Option<ResolvedCall> {
        // Check internal functions in same contract
        if let Some(contract) = self.contracts_by_name.get(contract_name) {
            if contract.functions.iter().any(|f| f.name == callee_name) {
                return Some(ResolvedCall {
                    caller_contract: contract_name.to_string(),
                    caller_function: caller_function.to_string(),
                    callee_contract: contract_name.to_string(),
                    callee_function: callee_name.to_string(),
                    is_external: false,
                    is_super: false,
                });
            }
        }

        // Check inherited functions
        if let Some(contract) = self.contracts_by_name.get(contract_name) {
            for base_name in &contract.base_contracts {
                if let Some(base) = self.contracts_by_name.get(base_name.as_str()) {
                    if base.functions.iter().any(|f| f.name == callee_name) {
                        return Some(ResolvedCall {
                            caller_contract: contract_name.to_string(),
                            caller_function: caller_function.to_string(),
                            callee_contract: base_name.clone(),
                            callee_function: callee_name.to_string(),
                            is_external: false,
                            is_super: false,
                        });
                    }
                }
            }
        }

        None
    }
}

/// Extract function call names from Solidity source code using AST.
/// Falls back to regex if AST parsing fails.
pub fn extract_calls_from_source(source: &str) -> Vec<String> {
    // Wrap in a dummy function so it parses as a valid Solidity file
    let wrapped = format!("contract _C {{ function _f() {{ {} }} }}", source);

    if let Ok(tokens) = wrapped.parse::<proc_macro2::TokenStream>() {
        if let Ok(file) = syn_solidity::parse2(tokens) {
            let mut calls = Vec::new();
            // Navigate: file > contract > function > body > stmts
            for item in &file.items {
                if let syn_solidity::Item::Contract(c) = item {
                    for body_item in &c.body {
                        if let syn_solidity::Item::Function(f) = body_item {
                            if let syn_solidity::FunctionBody::Block(block) = &f.body {
                                for stmt in &block.stmts {
                                    extract_calls_from_stmt(stmt, &mut calls);
                                }
                            }
                        }
                    }
                }
            }
            calls.sort();
            calls.dedup();
            return calls;
        }
    }

    // Fallback: regex-based extraction
    extract_calls_regex(source)
}

/// AST walk: extract call names from a statement.
fn extract_calls_from_stmt(stmt: &syn_solidity::Stmt, calls: &mut Vec<String>) {
    match stmt {
        syn_solidity::Stmt::Expr(expr_stmt) => {
            extract_calls_from_expr(&expr_stmt.expr, calls);
        }
        syn_solidity::Stmt::Return(ret) => {
            if let Some(expr) = &ret.expr {
                extract_calls_from_expr(expr, calls);
            }
        }
        syn_solidity::Stmt::Block(block) => {
            for s in &block.stmts {
                extract_calls_from_stmt(s, calls);
            }
        }
        syn_solidity::Stmt::UncheckedBlock(block) => {
            for s in &block.block.stmts {
                extract_calls_from_stmt(s, calls);
            }
        }
        syn_solidity::Stmt::If(if_stmt) => {
            extract_calls_from_expr(&if_stmt.cond, calls);
            extract_calls_from_stmt(&if_stmt.then_branch, calls);
            if let Some((_, else_branch)) = &if_stmt.else_branch {
                extract_calls_from_stmt(else_branch, calls);
            }
        }
        syn_solidity::Stmt::For(for_stmt) => {
            // init is ForInitStmt (not Option)
            extract_calls_from_for_init(&for_stmt.init, calls);
            if let Some(cond) = &for_stmt.cond {
                extract_calls_from_expr(cond, calls);
            }
            if let Some(post) = &for_stmt.post {
                extract_calls_from_expr(post, calls);
            }
            extract_calls_from_stmt(&for_stmt.body, calls);
        }
        syn_solidity::Stmt::While(while_stmt) => {
            extract_calls_from_expr(&while_stmt.cond, calls);
            extract_calls_from_stmt(&while_stmt.body, calls);
        }
        syn_solidity::Stmt::DoWhile(do_while) => {
            extract_calls_from_stmt(&do_while.body, calls);
            extract_calls_from_expr(&do_while.cond, calls);
        }
        syn_solidity::Stmt::VarDecl(var_decl) => {
            if let Some((_, expr)) = &var_decl.assignment {
                extract_calls_from_expr(expr, calls);
            }
        }
        syn_solidity::Stmt::Emit(emit) => {
            // emit EventName(...) — skip, not a function call
        }
        syn_solidity::Stmt::Revert(revert) => {
            // revert ErrorName(...) — skip builtin
        }
        syn_solidity::Stmt::Try(try_stmt) => {
            extract_calls_from_expr(&try_stmt.expr, calls);
            for s in &try_stmt.block.stmts {
                extract_calls_from_stmt(s, calls);
            }
            for catch in &try_stmt.catch {
                for s in &catch.block.stmts {
                    extract_calls_from_stmt(s, calls);
                }
            }
        }
        _ => {}
    }
}

/// Handle ForInitStmt (variable decl or expression).
fn extract_calls_from_for_init(init: &syn_solidity::ForInitStmt, calls: &mut Vec<String>) {
    match init {
        syn_solidity::ForInitStmt::VarDecl(var_decl) => {
            if let Some((_, expr)) = &var_decl.assignment {
                extract_calls_from_expr(expr, calls);
            }
        }
        syn_solidity::ForInitStmt::Expr(expr_stmt) => {
            extract_calls_from_expr(&expr_stmt.expr, calls);
        }
        _ => {}
    }
}

/// AST walk: extract call names from an expression.
fn extract_calls_from_expr(expr: &syn_solidity::Expr, calls: &mut Vec<String>) {
    match expr {
        syn_solidity::Expr::Call(call) => {
            // Extract the callee name
            match &*call.expr {
                syn_solidity::Expr::Ident(ident) => {
                    let name = ident.to_string();
                    if !is_builtin(&name) {
                        calls.push(name);
                    }
                }
                syn_solidity::Expr::Member(member) => {
                    // obj.method() — extract as "obj.method"
                    if let (syn_solidity::Expr::Ident(obj), syn_solidity::Expr::Ident(method)) =
                        (&*member.expr, &*member.member)
                    {
                        let obj_name = obj.to_string();
                        let method_name = method.to_string();
                        if !is_builtin(&obj_name) {
                            calls.push(format!("{}.{}", obj_name, method_name));
                        }
                    }
                }
                _ => {
                    // Complex callee (e.g. chained calls) — recurse into it
                    extract_calls_from_expr(&call.expr, calls);
                }
            }
            // Also walk arguments for nested calls like foo(bar(x))
            if let syn_solidity::ArgListImpl::Unnamed(args) = &call.args.list {
                for arg in args.iter() {
                    extract_calls_from_expr(arg, calls);
                }
            }
        }
        syn_solidity::Expr::Binary(bin) => {
            extract_calls_from_expr(&bin.left, calls);
            extract_calls_from_expr(&bin.right, calls);
        }
        syn_solidity::Expr::Unary(un) => {
            extract_calls_from_expr(&un.expr, calls);
        }
        syn_solidity::Expr::Postfix(post) => {
            extract_calls_from_expr(&post.expr, calls);
        }
        syn_solidity::Expr::Ternary(tern) => {
            extract_calls_from_expr(&tern.cond, calls);
            extract_calls_from_expr(&tern.if_true, calls);
            extract_calls_from_expr(&tern.if_false, calls);
        }
        syn_solidity::Expr::Index(idx) => {
            extract_calls_from_expr(&idx.expr, calls);
            if let Some(start) = &idx.start {
                extract_calls_from_expr(start, calls);
            }
            if let Some(end) = &idx.end {
                extract_calls_from_expr(end, calls);
            }
        }
        syn_solidity::Expr::Tuple(tuple) => {
            for elem in &tuple.elems {
                extract_calls_from_expr(elem, calls);
            }
        }
        syn_solidity::Expr::Member(member) => {
            extract_calls_from_expr(&member.expr, calls);
        }
        _ => {}
    }
}

/// Regex fallback for extracting function calls when AST parsing fails.
fn extract_calls_regex(source: &str) -> Vec<String> {
    let mut calls = Vec::new();

    let identifier_pattern = regex::Regex::new(r"(\w+)\s*\(").unwrap();
    for cap in identifier_pattern.captures_iter(source) {
        let name = cap[1].to_string();
        if !is_builtin(&name) {
            calls.push(name);
        }
    }

    let external_pattern = regex::Regex::new(r"(\w+)\.(\w+)\s*\(").unwrap();
    for cap in external_pattern.captures_iter(source) {
        let target = &cap[1];
        let method = &cap[2];
        if !is_builtin(target) && !is_builtin(method) {
            calls.push(format!("{}.{}", target, method));
        }
    }

    calls.sort();
    calls.dedup();
    calls
}

fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "require"
            | "assert"
            | "revert"
            | "emit"
            | "keccak256"
            | "sha256"
            | "ripemd160"
            | "ecrecover"
            | "addmod"
            | "mulmod"
            | "selfdestruct"
            | "type"
            | "abi"
            | "block"
            | "msg"
            | "tx"
            | "gasleft"
            | "blockhash"
            | "address"
            | "uint256"
            | "uint"
            | "int"
            | "bool"
            | "bytes"
            | "string"
            | "if"
            | "else"
            | "for"
            | "while"
            | "do"
            | "return"
            | "delete"
            | "new"
            | "this"
            | "super"
            | "push"
            | "pop"
            | "length"
    )
}
