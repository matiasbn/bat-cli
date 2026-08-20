use std::collections::HashMap;

use solar_parse::{
    ast,
    interface::{source_map::FileName, Session},
    Parser,
};

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
    pub fn resolve_calls(&self, contract_name: &str, function: &EvmFunction) -> Vec<ResolvedCall> {
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

    let sess = Session::builder().with_silent_emitter(None).build();

    let result = sess.enter(|| -> Option<Vec<String>> {
        let arena = ast::Arena::new();
        let mut parser = Parser::from_source_code(
            &sess,
            &arena,
            FileName::Custom("call_resolver".into()),
            wrapped.clone(),
        )
        .ok()?;

        let file = parser.parse_file().map_err(|e| e.emit()).ok()?;

        let mut calls = Vec::new();
        // Navigate: file > contract > function > body > stmts
        for item in file.items.iter() {
            if let ast::ItemKind::Contract(c) = &item.kind {
                for body_item in c.body.iter() {
                    if let ast::ItemKind::Function(f) = &body_item.kind {
                        if let Some(block) = &f.body {
                            for stmt in block.stmts.iter() {
                                extract_calls_from_stmt(&stmt.kind, &mut calls);
                            }
                        }
                    }
                }
            }
        }
        calls.sort();
        calls.dedup();
        Some(calls)
    });

    result.unwrap_or_else(|| extract_calls_regex(source))
}

/// AST walk: extract call names from a statement kind.
fn extract_calls_from_stmt(kind: &ast::StmtKind<'_>, calls: &mut Vec<String>) {
    match kind {
        ast::StmtKind::Expr(expr) => {
            extract_calls_from_expr(&expr.kind, calls);
        }
        ast::StmtKind::Return(opt_expr) => {
            if let Some(expr) = opt_expr {
                extract_calls_from_expr(&expr.kind, calls);
            }
        }
        ast::StmtKind::Block(block) => {
            for s in block.stmts.iter() {
                extract_calls_from_stmt(&s.kind, calls);
            }
        }
        ast::StmtKind::UncheckedBlock(block) => {
            for s in block.stmts.iter() {
                extract_calls_from_stmt(&s.kind, calls);
            }
        }
        ast::StmtKind::If(cond, then_branch, else_branch) => {
            extract_calls_from_expr(&cond.kind, calls);
            extract_calls_from_stmt(&then_branch.kind, calls);
            if let Some(else_stmt) = else_branch {
                extract_calls_from_stmt(&else_stmt.kind, calls);
            }
        }
        ast::StmtKind::For {
            init,
            cond,
            next,
            body,
        } => {
            if let Some(init_stmt) = init {
                extract_calls_from_stmt(&init_stmt.kind, calls);
            }
            if let Some(cond_expr) = cond {
                extract_calls_from_expr(&cond_expr.kind, calls);
            }
            if let Some(next_expr) = next {
                extract_calls_from_expr(&next_expr.kind, calls);
            }
            extract_calls_from_stmt(&body.kind, calls);
        }
        ast::StmtKind::While(cond, body) => {
            extract_calls_from_expr(&cond.kind, calls);
            extract_calls_from_stmt(&body.kind, calls);
        }
        ast::StmtKind::DoWhile(body, cond) => {
            extract_calls_from_stmt(&body.kind, calls);
            extract_calls_from_expr(&cond.kind, calls);
        }
        ast::StmtKind::DeclSingle(var) => {
            if let Some(init) = &var.initializer {
                extract_calls_from_expr(&init.kind, calls);
            }
        }
        ast::StmtKind::DeclMulti(_, expr) => {
            extract_calls_from_expr(&expr.kind, calls);
        }
        ast::StmtKind::Try(try_stmt) => {
            extract_calls_from_expr(&try_stmt.expr.kind, calls);
            for clause in try_stmt.clauses.iter() {
                for s in clause.block.stmts.iter() {
                    extract_calls_from_stmt(&s.kind, calls);
                }
            }
        }
        ast::StmtKind::Emit(_, _) => {
            // emit EventName(...) — skip, not a function call
        }
        ast::StmtKind::Revert(_, _) => {
            // revert ErrorName(...) — skip builtin
        }
        _ => {}
    }
}

/// AST walk: extract call names from an expression kind.
fn extract_calls_from_expr(kind: &ast::ExprKind<'_>, calls: &mut Vec<String>) {
    match kind {
        ast::ExprKind::Call(callee, args) => {
            // Extract the callee name
            match &callee.kind {
                ast::ExprKind::Ident(ident) => {
                    let name = ident.as_str().to_string();
                    if !is_builtin(&name) {
                        calls.push(name);
                    }
                }
                ast::ExprKind::Member(obj_expr, method_ident) => {
                    // obj.method() — extract as "obj.method"
                    if let ast::ExprKind::Ident(obj_ident) = &obj_expr.kind {
                        let obj_name = obj_ident.as_str().to_string();
                        let method_name = method_ident.as_str().to_string();
                        if !is_builtin(&obj_name) {
                            calls.push(format!("{}.{}", obj_name, method_name));
                        }
                    }
                }
                _ => {
                    // Complex callee (e.g. chained calls) — recurse into it
                    extract_calls_from_expr(&callee.kind, calls);
                }
            }
            // Also walk arguments for nested calls like foo(bar(x))
            for arg in args.exprs() {
                extract_calls_from_expr(&arg.kind, calls);
            }
        }
        ast::ExprKind::Binary(left, _op, right) => {
            extract_calls_from_expr(&left.kind, calls);
            extract_calls_from_expr(&right.kind, calls);
        }
        ast::ExprKind::Unary(_op, expr) => {
            extract_calls_from_expr(&expr.kind, calls);
        }
        ast::ExprKind::Ternary(cond, if_true, if_false) => {
            extract_calls_from_expr(&cond.kind, calls);
            extract_calls_from_expr(&if_true.kind, calls);
            extract_calls_from_expr(&if_false.kind, calls);
        }
        ast::ExprKind::Assign(left, _op, right) => {
            extract_calls_from_expr(&left.kind, calls);
            extract_calls_from_expr(&right.kind, calls);
        }
        ast::ExprKind::Index(expr, _index_kind) => {
            extract_calls_from_expr(&expr.kind, calls);
        }
        ast::ExprKind::Tuple(elems) => {
            for elem in elems.iter() {
                if let solar_parse::interface::SpannedOption::Some(e) = elem {
                    extract_calls_from_expr(&e.kind, calls);
                }
            }
        }
        ast::ExprKind::Member(expr, _ident) => {
            extract_calls_from_expr(&expr.kind, calls);
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

/// A single call site: the callee name plus the 1-based line, relative to the
/// source slice that was parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    pub name: String,
    pub line: usize,
}

/// Extract every call site — name **and** line — from a Solidity source slice.
///
/// Unlike [`extract_calls_from_source`], which dedupes into a set of names, this
/// keeps one entry per occurrence, because the Miro deployment draws one
/// connector per call site anchored to its exact line.
///
/// The source is wrapped in `contract _C { function _f() { … } }` on a single
/// leading line, so line `N` of the wrapped input is line `N` of `source`.
pub fn extract_call_sites_from_source(source: &str) -> Vec<CallSite> {
    if source.trim().is_empty() {
        return Vec::new();
    }
    let wrapped = format!("contract _C {{ function _f() {{ {} }} }}", source);

    let sess = Session::builder().with_silent_emitter(None).build();

    let result = sess.enter(|| -> Option<Vec<CallSite>> {
        let arena = ast::Arena::new();
        let mut parser = Parser::from_source_code(
            &sess,
            &arena,
            FileName::Custom("call_sites".into()),
            wrapped.clone(),
        )
        .ok()?;

        let file = parser.parse_file().map_err(|e| e.emit()).ok()?;

        let mut raw: Vec<(String, solar_parse::interface::Span)> = Vec::new();
        for item in file.items.iter() {
            if let ast::ItemKind::Contract(c) = &item.kind {
                for body_item in c.body.iter() {
                    if let ast::ItemKind::Function(f) = &body_item.kind {
                        if let Some(block) = &f.body {
                            for stmt in block.stmts.iter() {
                                collect_call_sites_from_stmt(&stmt.kind, &mut raw);
                            }
                        }
                    }
                }
            }
        }

        let source_map = sess.source_map();
        let mut sites: Vec<CallSite> = raw
            .into_iter()
            .map(|(name, span)| CallSite {
                name,
                line: source_map.lookup_char_pos(span.lo()).line,
            })
            .collect();
        sites.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.name.cmp(&b.name)));
        sites.dedup();
        Some(sites)
    });

    result.unwrap_or_else(|| extract_call_sites_regex(source))
}

/// Line-aware regex fallback, used when the AST parse fails.
fn extract_call_sites_regex(source: &str) -> Vec<CallSite> {
    let identifier_pattern = regex::Regex::new(r"(\w+(?:\.\w+)?)\s*\(").unwrap();
    let mut sites = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        // Drop line comments so a call mentioned in prose is not picked up.
        let code = line.split("//").next().unwrap_or("");
        for cap in identifier_pattern.captures_iter(code) {
            let name = cap[1].to_string();
            let head = name.split('.').next().unwrap_or(&name);
            if is_builtin(&name) || is_builtin(head) {
                continue;
            }
            sites.push(CallSite {
                name,
                line: idx + 1,
            });
        }
    }
    sites
}

fn collect_call_sites_from_stmt(
    kind: &ast::StmtKind<'_>,
    out: &mut Vec<(String, solar_parse::interface::Span)>,
) {
    match kind {
        ast::StmtKind::Expr(expr) => collect_call_sites_from_expr(expr, out),
        ast::StmtKind::Return(opt_expr) => {
            if let Some(expr) = opt_expr {
                collect_call_sites_from_expr(expr, out);
            }
        }
        ast::StmtKind::Block(block) | ast::StmtKind::UncheckedBlock(block) => {
            for s in block.stmts.iter() {
                collect_call_sites_from_stmt(&s.kind, out);
            }
        }
        ast::StmtKind::If(cond, then_branch, else_branch) => {
            collect_call_sites_from_expr(cond, out);
            collect_call_sites_from_stmt(&then_branch.kind, out);
            if let Some(else_stmt) = else_branch {
                collect_call_sites_from_stmt(&else_stmt.kind, out);
            }
        }
        ast::StmtKind::For {
            init,
            cond,
            next,
            body,
        } => {
            if let Some(init_stmt) = init {
                collect_call_sites_from_stmt(&init_stmt.kind, out);
            }
            if let Some(cond_expr) = cond {
                collect_call_sites_from_expr(cond_expr, out);
            }
            if let Some(next_expr) = next {
                collect_call_sites_from_expr(next_expr, out);
            }
            collect_call_sites_from_stmt(&body.kind, out);
        }
        ast::StmtKind::While(cond, body) => {
            collect_call_sites_from_expr(cond, out);
            collect_call_sites_from_stmt(&body.kind, out);
        }
        ast::StmtKind::DoWhile(body, cond) => {
            collect_call_sites_from_stmt(&body.kind, out);
            collect_call_sites_from_expr(cond, out);
        }
        ast::StmtKind::DeclSingle(var) => {
            if let Some(init) = &var.initializer {
                collect_call_sites_from_expr(init, out);
            }
        }
        ast::StmtKind::DeclMulti(_, expr) => collect_call_sites_from_expr(expr, out),
        ast::StmtKind::Try(try_stmt) => {
            collect_call_sites_from_expr(&try_stmt.expr, out);
            for clause in try_stmt.clauses.iter() {
                for s in clause.block.stmts.iter() {
                    collect_call_sites_from_stmt(&s.kind, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_call_sites_from_expr(
    expr: &ast::Expr<'_>,
    out: &mut Vec<(String, solar_parse::interface::Span)>,
) {
    match &expr.kind {
        ast::ExprKind::Call(callee, args) => {
            match &callee.kind {
                ast::ExprKind::Ident(ident) => {
                    let name = ident.as_str().to_string();
                    if !is_builtin(&name) {
                        // Anchor on the whole call expression, so the line is the
                        // one the reader sees the call on.
                        out.push((name, expr.span));
                    }
                }
                ast::ExprKind::Member(obj_expr, method_ident) => {
                    if let ast::ExprKind::Ident(obj_ident) = &obj_expr.kind {
                        let obj_name = obj_ident.as_str().to_string();
                        let method_name = method_ident.as_str().to_string();
                        if !is_builtin(&obj_name) {
                            out.push((format!("{}.{}", obj_name, method_name), expr.span));
                        }
                    }
                }
                _ => collect_call_sites_from_expr(callee, out),
            }
            for arg in args.exprs() {
                collect_call_sites_from_expr(arg, out);
            }
        }
        ast::ExprKind::Binary(left, _op, right) => {
            collect_call_sites_from_expr(left, out);
            collect_call_sites_from_expr(right, out);
        }
        ast::ExprKind::Unary(_op, inner) => collect_call_sites_from_expr(inner, out),
        ast::ExprKind::Ternary(cond, if_true, if_false) => {
            collect_call_sites_from_expr(cond, out);
            collect_call_sites_from_expr(if_true, out);
            collect_call_sites_from_expr(if_false, out);
        }
        ast::ExprKind::Assign(left, _op, right) => {
            collect_call_sites_from_expr(left, out);
            collect_call_sites_from_expr(right, out);
        }
        ast::ExprKind::Index(inner, _index_kind) => collect_call_sites_from_expr(inner, out),
        ast::ExprKind::Tuple(elems) => {
            for elem in elems.iter() {
                if let solar_parse::interface::SpannedOption::Some(e) = elem {
                    collect_call_sites_from_expr(e, out);
                }
            }
        }
        ast::ExprKind::Member(inner, _ident) => collect_call_sites_from_expr(inner, out),
        _ => {}
    }
}

#[cfg(test)]
mod call_site_test {
    use super::*;

    #[test]
    fn test_extract_call_sites_keeps_line_and_repeats() {
        let source = r#"
uint256 fee = FeeLib.feeOf(assets, entryFeeBps);
uint256 net = assets - fee;
mintedShares = ShareLib.toShares(net, totalAssets(), totalShares);
_mint(receiver, mintedShares);
"#;
        let sites = extract_call_sites_from_source(source);

        let find = |name: &str| -> Vec<usize> {
            sites
                .iter()
                .filter(|s| s.name == name)
                .map(|s| s.line)
                .collect()
        };

        assert_eq!(find("FeeLib.feeOf"), vec![2]);
        assert_eq!(find("ShareLib.toShares"), vec![4]);
        assert_eq!(find("totalAssets"), vec![4]);
        assert_eq!(find("_mint"), vec![5]);
    }

    #[test]
    fn test_same_callee_on_several_lines() {
        let source = r#"
uint256 a = mulDiv(x, y, z);
uint256 b = other(1);
uint256 c = mulDiv(p, q, r);
"#;
        let lines: Vec<usize> = extract_call_sites_from_source(source)
            .into_iter()
            .filter(|s| s.name == "mulDiv")
            .map(|s| s.line)
            .collect();
        assert_eq!(lines, vec![2, 4], "one call site per occurrence");
    }
}
