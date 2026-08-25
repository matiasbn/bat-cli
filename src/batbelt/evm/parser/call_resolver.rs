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

/// A single call site: the callee name plus where it sits in the source slice.
///
/// The column matters as much as the line. Two calls on one line — say
/// `MathLib.wadMul(amount, price(asset))` — need two connectors landing on two
/// different tokens, and the AST already knows exactly where each one starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    pub name: String,
    /// 1-based line, relative to the parsed slice.
    pub line: usize,
    /// 0-based column where the callee's own name begins. For `a.b()` this is
    /// the column of `b`, not of `a`.
    pub column: usize,
    /// The identifier the connector should point at: `b` for `a.b()`.
    pub symbol: String,
}

/// Extract every call site — name **and** line — from a Solidity source slice.
///
/// Unlike [`extract_calls_from_source`], which dedupes into a set of names, this
/// keeps one entry per occurrence, because the Miro deployment draws one
/// connector per call site anchored to its exact line.
///
/// The wrapper puts `source` on its own lines, so a reported column is the
/// column in `source` and a reported line is one more than the line in
/// `source`. Sharing a line with the wrapper would shift every column on the
/// first line by the length of the prefix.
pub fn extract_call_sites_from_source(source: &str) -> Vec<CallSite> {
    if source.trim().is_empty() {
        return Vec::new();
    }
    let wrapped = format!("contract _C {{ function _f() {{\n{}\n}} }}", source);

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

        let mut raw: Vec<(String, String, solar_parse::interface::Span)> = Vec::new();
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
            .map(|(name, symbol, span)| {
                let location = source_map.lookup_char_pos(span.lo());
                CallSite {
                    name,
                    // The wrapper occupies the first line.
                    line: location.line.saturating_sub(1).max(1),
                    column: location.col.0,
                    symbol,
                }
            })
            .collect();
        sites.sort_by(|a, b| {
            a.line
                .cmp(&b.line)
                .then_with(|| a.column.cmp(&b.column))
                .then_with(|| a.name.cmp(&b.name))
        });
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
            let symbol = name.rsplit('.').next().unwrap_or(&name).to_string();
            let column = cap
                .get(1)
                .map(|m| {
                    // Point at the last segment, matching the AST behaviour.
                    m.start() + name.len() - symbol.len()
                })
                .unwrap_or(0);
            sites.push(CallSite {
                name,
                line: idx + 1,
                column,
                symbol,
            });
        }
    }
    sites
}

type RawCallSite = (String, String, solar_parse::interface::Span);

fn collect_call_sites_from_stmt(kind: &ast::StmtKind<'_>, out: &mut Vec<RawCallSite>) {
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

fn collect_call_sites_from_expr(expr: &ast::Expr<'_>, out: &mut Vec<RawCallSite>) {
    match &expr.kind {
        ast::ExprKind::Call(callee, args) => {
            match &callee.kind {
                ast::ExprKind::Ident(ident) => {
                    let name = ident.as_str().to_string();
                    if !is_builtin(&name) {
                        // The identifier's own span, so the connector lands on
                        // the called name rather than on the whole expression.
                        out.push((name.clone(), name, ident.span));
                    }
                }
                ast::ExprKind::Member(obj_expr, method_ident) => {
                    if let ast::ExprKind::Ident(obj_ident) = &obj_expr.kind {
                        let obj_name = obj_ident.as_str().to_string();
                        let method_name = method_ident.as_str().to_string();
                        if !is_builtin(&obj_name) {
                            // Point at the method, not at the receiver: in
                            // `MathLib.wadMul(...)` the interesting token is
                            // `wadMul`.
                            out.push((
                                format!("{}.{}", obj_name, method_name),
                                method_name,
                                method_ident.span,
                            ));
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

/// Extract the storage locations WRITTEN by a function body, from its source text.
///
/// Fully generic — no library/framework assumptions. A "storage write" is any
/// assignment (`x = …`, `x += …`), `delete x`, `++`/`--`, or `.push()`/`.pop()`
/// whose lvalue's ROOT is either a declared state variable (`state_vars`) OR a
/// local declared with `storage` data location — a storage pointer, e.g. the
/// diamond-storage `MyStruct storage $ = _get();` then `$.field = …`. Resolving
/// the root through `Member`/`Index` is what makes `$.reserveStable += r0` and
/// `balances[user] = v` count, on any codebase.
///
/// Returns readable lvalue paths (e.g. `"reserveStable"`, `"$.reserveStable"`,
/// `"balances[]"`), sorted and deduped. Empty when the body doesn't parse.
pub fn extract_storage_writes_from_source(
    source: &str,
    state_vars: &[String],
    storage_params: &[String],
) -> Vec<String> {
    // `body_source` is inconsistent across sonar's parse: sometimes the inner
    // statements, sometimes the WHOLE function (`function f() { … }`, signature
    // and braces included). Wrapping a whole function inside a dummy `_f` would
    // make an illegal nested function and fail to parse. So try both shapes —
    // a contract holding the source as-is (whole-function case) and the source
    // as a dummy function body (bare-statements case) — and union whatever parses.
    let mut writes = Vec::new();
    for wrapped in [
        format!("contract _C {{ {} }}", source),
        format!("contract _C {{ function _f() {{ {} }} }}", source),
    ] {
        if let Some(w) = collect_writes_in_wrapped(&wrapped, state_vars, storage_params) {
            writes.extend(w);
        }
    }
    writes.sort();
    writes.dedup();
    writes
}

/// Parse one wrapped snippet and collect storage writes from every function it
/// contains. `None` when the snippet doesn't parse (so the caller can try the
/// other wrapping).
fn collect_writes_in_wrapped(
    wrapped: &str,
    state_vars: &[String],
    storage_params: &[String],
) -> Option<Vec<String>> {
    let sess = Session::builder().with_silent_emitter(None).build();
    sess.enter(|| -> Option<Vec<String>> {
        let arena = ast::Arena::new();
        let mut parser = Parser::from_source_code(
            &sess,
            &arena,
            FileName::Custom("storage_writes".into()),
            wrapped.to_string(),
        )
        .ok()?;
        let file = parser.parse_file().map_err(|e| e.emit()).ok()?;

        let mut writes = Vec::new();
        let mut found_fn = false;
        for item in file.items.iter() {
            if let ast::ItemKind::Contract(c) = &item.kind {
                for body_item in c.body.iter() {
                    if let ast::ItemKind::Function(f) = &body_item.kind {
                        if let Some(block) = &f.body {
                            found_fn = true;
                            // Pass 1: every storage pointer in scope — writes through
                            // it hit storage. Two sources:
                            // (a) `storage` PARAMETERS, e.g. a library taking the
                            //     storage struct: `execute(CvammStorage storage $, …)`
                            //     then `$.reserveStable += …`;
                            // (b) `storage` LOCALS, e.g. `CvammStorage storage $ = _s();`.
                            let mut storage_locals: Vec<String> = storage_params.to_vec();
                            for p in f.header.parameters.iter() {
                                if p.data_location == Some(ast::DataLocation::Storage) {
                                    if let Some(name) = p.name {
                                        storage_locals.push(name.as_str().to_string());
                                    }
                                }
                            }
                            for stmt in block.stmts.iter() {
                                collect_storage_locals(&stmt.kind, &mut storage_locals);
                            }
                            // Pass 2: the actual writes.
                            for stmt in block.stmts.iter() {
                                collect_writes_from_stmt(
                                    &stmt.kind,
                                    state_vars,
                                    &storage_locals,
                                    &mut writes,
                                );
                            }
                        }
                    }
                }
            }
        }
        // Treat "parsed but no function with a body" as a miss, so the caller
        // falls through to the other wrapping instead of accepting an empty set.
        if found_fn {
            Some(writes)
        } else {
            None
        }
    })
}

/// Collect the names of locals declared with `storage` data location (storage
/// pointers), recursing through nested blocks/control-flow.
fn collect_storage_locals(kind: &ast::StmtKind<'_>, out: &mut Vec<String>) {
    match kind {
        ast::StmtKind::DeclSingle(var) => {
            if var.data_location == Some(ast::DataLocation::Storage) {
                if let Some(name) = var.name {
                    out.push(name.as_str().to_string());
                }
            }
        }
        ast::StmtKind::Block(b) | ast::StmtKind::UncheckedBlock(b) => {
            for s in b.stmts.iter() {
                collect_storage_locals(&s.kind, out);
            }
        }
        ast::StmtKind::If(_, t, e) => {
            collect_storage_locals(&t.kind, out);
            if let Some(e) = e {
                collect_storage_locals(&e.kind, out);
            }
        }
        ast::StmtKind::For {
            init, body, ..
        } => {
            if let Some(i) = init {
                collect_storage_locals(&i.kind, out);
            }
            collect_storage_locals(&body.kind, out);
        }
        ast::StmtKind::While(_, body) | ast::StmtKind::DoWhile(body, _) => {
            collect_storage_locals(&body.kind, out);
        }
        ast::StmtKind::Try(t) => {
            for clause in t.clauses.iter() {
                for s in clause.block.stmts.iter() {
                    collect_storage_locals(&s.kind, out);
                }
            }
        }
        _ => {}
    }
}

/// Extract the EXTERNAL call targets of a function body as `(receiver, method)`
/// pairs — a call `x.foo(…)` on some receiver expression. The receiver is rendered
/// as a path (`positionManager`, `$.borrowerOps`, `arr[]`), so a caller can try to
/// resolve it to a concrete contract (a state var's interface type, a struct field's
/// type, …). Captures member/index chains that the deploy-time call-site extractor
/// drops, which is what lets `$.borrowerOps.adjustPosition` be surfaced at all.
/// Builtins (`require`, `.call`, …) are skipped. Dual-wrapped for the same
/// whole-function-vs-bare-statements `body_source` inconsistency as the write scan.
pub fn extract_external_call_targets(source: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for wrapped in [
        format!("contract _C {{ {} }}", source),
        format!("contract _C {{ function _f() {{ {} }} }}", source),
    ] {
        if let Some(v) = call_targets_in_wrapped(&wrapped) {
            out.extend(v);
        }
    }
    out.sort();
    out.dedup();
    out
}

fn call_targets_in_wrapped(wrapped: &str) -> Option<Vec<(String, String)>> {
    let sess = Session::builder().with_silent_emitter(None).build();
    sess.enter(|| -> Option<Vec<(String, String)>> {
        let arena = ast::Arena::new();
        let mut parser = Parser::from_source_code(
            &sess,
            &arena,
            FileName::Custom("call_targets".into()),
            wrapped.to_string(),
        )
        .ok()?;
        let file = parser.parse_file().map_err(|e| e.emit()).ok()?;
        let mut out = Vec::new();
        let mut found_fn = false;
        for item in file.items.iter() {
            if let ast::ItemKind::Contract(c) = &item.kind {
                for body_item in c.body.iter() {
                    if let ast::ItemKind::Function(f) = &body_item.kind {
                        if let Some(block) = &f.body {
                            found_fn = true;
                            for stmt in block.stmts.iter() {
                                collect_targets_from_stmt(&stmt.kind, &mut out);
                            }
                        }
                    }
                }
            }
        }
        if found_fn {
            Some(out)
        } else {
            None
        }
    })
}

fn collect_targets_from_stmt(kind: &ast::StmtKind<'_>, out: &mut Vec<(String, String)>) {
    match kind {
        ast::StmtKind::Expr(e) => collect_targets_from_expr(&e.kind, out),
        ast::StmtKind::Return(Some(e)) => collect_targets_from_expr(&e.kind, out),
        ast::StmtKind::Block(b) | ast::StmtKind::UncheckedBlock(b) => {
            for s in b.stmts.iter() {
                collect_targets_from_stmt(&s.kind, out);
            }
        }
        ast::StmtKind::If(cond, t, e) => {
            collect_targets_from_expr(&cond.kind, out);
            collect_targets_from_stmt(&t.kind, out);
            if let Some(e) = e {
                collect_targets_from_stmt(&e.kind, out);
            }
        }
        ast::StmtKind::For {
            init,
            cond,
            next,
            body,
        } => {
            if let Some(i) = init {
                collect_targets_from_stmt(&i.kind, out);
            }
            if let Some(c) = cond {
                collect_targets_from_expr(&c.kind, out);
            }
            if let Some(n) = next {
                collect_targets_from_expr(&n.kind, out);
            }
            collect_targets_from_stmt(&body.kind, out);
        }
        ast::StmtKind::While(c, b) => {
            collect_targets_from_expr(&c.kind, out);
            collect_targets_from_stmt(&b.kind, out);
        }
        ast::StmtKind::DoWhile(b, c) => {
            collect_targets_from_stmt(&b.kind, out);
            collect_targets_from_expr(&c.kind, out);
        }
        ast::StmtKind::DeclSingle(var) => {
            if let Some(init) = &var.initializer {
                collect_targets_from_expr(&init.kind, out);
            }
        }
        ast::StmtKind::DeclMulti(_, e) => collect_targets_from_expr(&e.kind, out),
        ast::StmtKind::Try(t) => {
            collect_targets_from_expr(&t.expr.kind, out);
            for clause in t.clauses.iter() {
                for s in clause.block.stmts.iter() {
                    collect_targets_from_stmt(&s.kind, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_targets_from_expr(kind: &ast::ExprKind<'_>, out: &mut Vec<(String, String)>) {
    match kind {
        ast::ExprKind::Call(callee, args) => {
            if let ast::ExprKind::Member(recv, method) = &callee.kind {
                let m = method.as_str().to_string();
                if !is_builtin(&m) {
                    if let Some((_, recv_path)) = lvalue_path(&recv.kind) {
                        out.push((recv_path, m));
                    }
                }
            }
            collect_targets_from_expr(&callee.kind, out);
            for arg in args.exprs() {
                collect_targets_from_expr(&arg.kind, out);
            }
        }
        ast::ExprKind::Assign(l, _, r) => {
            collect_targets_from_expr(&l.kind, out);
            collect_targets_from_expr(&r.kind, out);
        }
        ast::ExprKind::Binary(l, _, r) => {
            collect_targets_from_expr(&l.kind, out);
            collect_targets_from_expr(&r.kind, out);
        }
        ast::ExprKind::Unary(_, e) | ast::ExprKind::Delete(e) => {
            collect_targets_from_expr(&e.kind, out)
        }
        ast::ExprKind::Ternary(c, a, b) => {
            collect_targets_from_expr(&c.kind, out);
            collect_targets_from_expr(&a.kind, out);
            collect_targets_from_expr(&b.kind, out);
        }
        ast::ExprKind::Index(e, _) => collect_targets_from_expr(&e.kind, out),
        ast::ExprKind::Member(e, _) => collect_targets_from_expr(&e.kind, out),
        ast::ExprKind::Tuple(elems) => {
            for el in elems.iter() {
                if let solar_parse::interface::SpannedOption::Some(e) = el {
                    collect_targets_from_expr(&e.kind, out);
                }
            }
        }
        _ => {}
    }
}

/// The leftmost base of an lvalue: a plain identifier, or a call (a storage
/// accessor like `_s()` in the diamond-storage pattern — `_s().field = …`).
enum LvalueRoot {
    Ident(String),
    Call,
}

/// The `(root, rendered_path)` of an lvalue expression, walking through
/// member/index access to the leftmost base. `None` for a non-lvalue base
/// (a literal, an unsupported form, …).
fn lvalue_path(kind: &ast::ExprKind<'_>) -> Option<(LvalueRoot, String)> {
    match kind {
        ast::ExprKind::Ident(id) => {
            let n = id.as_str().to_string();
            Some((LvalueRoot::Ident(n.clone()), n))
        }
        ast::ExprKind::Member(obj, field) => {
            let (root, path) = lvalue_path(&obj.kind)?;
            Some((root, format!("{}.{}", path, field.as_str())))
        }
        ast::ExprKind::Index(obj, _) => {
            let (root, path) = lvalue_path(&obj.kind)?;
            Some((root, format!("{}[]", path)))
        }
        // A call at the base of an lvalue — you can only meaningfully assign
        // THROUGH a call result if it returns a storage reference, so a
        // `getter().field = …` is a storage write. Render the callee for context.
        ast::ExprKind::Call(callee, _) => {
            let name = match &callee.kind {
                ast::ExprKind::Ident(id) => id.as_str().to_string(),
                ast::ExprKind::Member(_, m) => m.as_str().to_string(),
                _ => "call".to_string(),
            };
            Some((LvalueRoot::Call, format!("{name}()")))
        }
        _ => None,
    }
}

/// Record `lval` as a storage write iff its root is a state variable, a
/// storage-pointer local, or a storage accessor call.
fn record_write(
    lval: &ast::ExprKind<'_>,
    state_vars: &[String],
    storage_locals: &[String],
    out: &mut Vec<String>,
) {
    if let Some((root, path)) = lvalue_path(lval) {
        let is_storage = match root {
            LvalueRoot::Call => true,
            LvalueRoot::Ident(name) => {
                state_vars.iter().any(|s| *s == name) || storage_locals.iter().any(|s| *s == name)
            }
        };
        if is_storage {
            out.push(path);
        }
    }
}

/// AST walk: record storage writes from a statement.
fn collect_writes_from_stmt(
    kind: &ast::StmtKind<'_>,
    sv: &[String],
    sl: &[String],
    out: &mut Vec<String>,
) {
    match kind {
        ast::StmtKind::Expr(expr) => collect_writes_from_expr(&expr.kind, sv, sl, out),
        ast::StmtKind::Block(b) | ast::StmtKind::UncheckedBlock(b) => {
            for s in b.stmts.iter() {
                collect_writes_from_stmt(&s.kind, sv, sl, out);
            }
        }
        ast::StmtKind::If(cond, t, e) => {
            collect_writes_from_expr(&cond.kind, sv, sl, out);
            collect_writes_from_stmt(&t.kind, sv, sl, out);
            if let Some(e) = e {
                collect_writes_from_stmt(&e.kind, sv, sl, out);
            }
        }
        ast::StmtKind::For {
            init,
            cond,
            next,
            body,
        } => {
            if let Some(i) = init {
                collect_writes_from_stmt(&i.kind, sv, sl, out);
            }
            if let Some(c) = cond {
                collect_writes_from_expr(&c.kind, sv, sl, out);
            }
            if let Some(n) = next {
                collect_writes_from_expr(&n.kind, sv, sl, out);
            }
            collect_writes_from_stmt(&body.kind, sv, sl, out);
        }
        ast::StmtKind::While(cond, body) => {
            collect_writes_from_expr(&cond.kind, sv, sl, out);
            collect_writes_from_stmt(&body.kind, sv, sl, out);
        }
        ast::StmtKind::DoWhile(body, cond) => {
            collect_writes_from_stmt(&body.kind, sv, sl, out);
            collect_writes_from_expr(&cond.kind, sv, sl, out);
        }
        ast::StmtKind::DeclSingle(var) => {
            if let Some(init) = &var.initializer {
                collect_writes_from_expr(&init.kind, sv, sl, out);
            }
        }
        ast::StmtKind::DeclMulti(_, expr) => collect_writes_from_expr(&expr.kind, sv, sl, out),
        ast::StmtKind::Try(t) => {
            collect_writes_from_expr(&t.expr.kind, sv, sl, out);
            for clause in t.clauses.iter() {
                for s in clause.block.stmts.iter() {
                    collect_writes_from_stmt(&s.kind, sv, sl, out);
                }
            }
        }
        _ => {}
    }
}

/// AST walk: record storage writes from an expression.
fn collect_writes_from_expr(
    kind: &ast::ExprKind<'_>,
    sv: &[String],
    sl: &[String],
    out: &mut Vec<String>,
) {
    match kind {
        ast::ExprKind::Assign(left, _op, right) => {
            record_write(&left.kind, sv, sl, out);
            collect_writes_from_expr(&left.kind, sv, sl, out);
            collect_writes_from_expr(&right.kind, sv, sl, out);
        }
        ast::ExprKind::Delete(inner) => {
            record_write(&inner.kind, sv, sl, out);
            collect_writes_from_expr(&inner.kind, sv, sl, out);
        }
        ast::ExprKind::Unary(op, inner) => {
            if matches!(
                op.kind,
                ast::UnOpKind::PreInc
                    | ast::UnOpKind::PreDec
                    | ast::UnOpKind::PostInc
                    | ast::UnOpKind::PostDec
            ) {
                record_write(&inner.kind, sv, sl, out);
            }
            collect_writes_from_expr(&inner.kind, sv, sl, out);
        }
        ast::ExprKind::Call(callee, args) => {
            // `.push(...)` / `.pop()` mutate the storage array they target.
            if let ast::ExprKind::Member(base, method) = &callee.kind {
                let m = method.as_str();
                if m == "push" || m == "pop" {
                    record_write(&base.kind, sv, sl, out);
                }
            }
            collect_writes_from_expr(&callee.kind, sv, sl, out);
            for arg in args.exprs() {
                collect_writes_from_expr(&arg.kind, sv, sl, out);
            }
        }
        ast::ExprKind::Binary(l, _, r) => {
            collect_writes_from_expr(&l.kind, sv, sl, out);
            collect_writes_from_expr(&r.kind, sv, sl, out);
        }
        ast::ExprKind::Ternary(c, a, b) => {
            collect_writes_from_expr(&c.kind, sv, sl, out);
            collect_writes_from_expr(&a.kind, sv, sl, out);
            collect_writes_from_expr(&b.kind, sv, sl, out);
        }
        ast::ExprKind::Index(e, _) => collect_writes_from_expr(&e.kind, sv, sl, out),
        ast::ExprKind::Member(e, _) => collect_writes_from_expr(&e.kind, sv, sl, out),
        ast::ExprKind::Tuple(elems) => {
            for el in elems.iter() {
                if let solar_parse::interface::SpannedOption::Some(e) = el {
                    collect_writes_from_expr(&e.kind, sv, sl, out);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod storage_write_test {


    use super::{extract_external_call_targets, extract_storage_writes_from_source};

    #[test]
    fn external_call_targets_capture_member_chains() {
        let src = "\
            $.borrowerOps.adjustPosition(a, b);\n\
            positionManager.setX(1);\n\
            uint256 x = oracle.price();\n\
            require(ok, \"e\");\n\
            token.safeTransfer(to, amt);\n";
        let t = extract_external_call_targets(src);
        assert!(t.contains(&("$.borrowerOps".to_string(), "adjustPosition".to_string())), "{t:?}");
        assert!(t.contains(&("positionManager".to_string(), "setX".to_string())), "{t:?}");
        assert!(t.contains(&("oracle".to_string(), "price".to_string())), "{t:?}");
        assert!(t.contains(&("token".to_string(), "safeTransfer".to_string())), "{t:?}");
        // `require` is a builtin, never a call target.
        assert!(!t.iter().any(|(_, m)| m == "require"), "{t:?}");
    }

    #[test]
    fn detects_direct_pointer_index_and_ignores_locals() {
        let src = r#"
            Layout storage $ = _layout();
            uint256 memTotal = 0;      // local, NOT storage
            $.reserveStable += r0;     // storage pointer write
            totalSupply = memTotal;    // direct state-var write
            balances[user] = 5;        // mapping/index write
            paused = true;             // direct state-var write
            memTotal = 7;              // local write — must be ignored
            counter++;                 // unary state-var write
            delete lastActor;          // delete state-var
            _s().kappa = 3;            // storage accessor call write
        "#;
        let state_vars = vec![
            "totalSupply".to_string(),
            "balances".to_string(),
            "paused".to_string(),
            "counter".to_string(),
            "lastActor".to_string(),
        ];
        let w = extract_storage_writes_from_source(src, &state_vars, &[]);
        assert!(w.contains(&"$.reserveStable".to_string()), "pointer: {w:?}");
        assert!(w.contains(&"totalSupply".to_string()), "direct: {w:?}");
        assert!(w.contains(&"balances[]".to_string()), "index: {w:?}");
        assert!(w.contains(&"paused".to_string()), "bool: {w:?}");
        assert!(w.contains(&"counter".to_string()), "unary: {w:?}");
        assert!(w.contains(&"lastActor".to_string()), "delete: {w:?}");
        assert!(w.contains(&"_s().kappa".to_string()), "accessor call: {w:?}");
        assert!(!w.contains(&"memTotal".to_string()), "local leaked: {w:?}");
    }

    #[test]
    fn detects_write_through_a_storage_parameter() {
        // A library that takes the storage struct as a `storage` PARAMETER and
        // writes through it — the CvammSwapLib.execute shape. Whole-function body.
        let src = r#"
            function execute(CvammStore.CvammStorage storage $, SwapVars memory v)
                internal
                returns (uint256)
            {
                uint256 k = $.kappa;
                $.reserveStable += v.amountInUsed;
                return k;
            }
        "#;
        let w = extract_storage_writes_from_source(src, &[], &[]);
        assert!(w.contains(&"$.reserveStable".to_string()), "got {w:?}");
    }

    #[test]
    fn detects_accessor_write_in_a_real_setter_shape() {
        // The exact shape of CvammALM.setPaused: guard branches, then an
        // accessor-call storage write. No state_vars needed (call-rooted).
        let src = r#"
            if (p) {
                if (msg.sender != owner() && msg.sender != guardian()) revert NotPauser();
            } else if (msg.sender != owner()) {
                revert NotOwner();
            }
            _s().paused = p;
        "#;
        let w = extract_storage_writes_from_source(src, &[], &[]);
        assert_eq!(w, vec!["_s().paused".to_string()], "got {w:?}");
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

    /// Two calls on one line must be told apart by column, which is what lets
    /// the deployment draw one connector per token instead of stacking both on
    /// the same spot.
    #[test]
    fn test_two_calls_on_the_same_line_have_distinct_columns() {
        let source = "return MathLib.wadMul(amount, price(asset));";
        let sites = extract_call_sites_from_source(source);

        let wad = sites
            .iter()
            .find(|s| s.name == "MathLib.wadMul")
            .expect("wadMul missing");
        let price = sites
            .iter()
            .find(|s| s.name == "price")
            .expect("price missing");

        assert_eq!(wad.line, price.line, "both are on the same line");
        assert_ne!(wad.column, price.column);
        assert!(wad.column < price.column, "wadMul comes first");

        // The column must point at the method, not at the receiver.
        assert_eq!(wad.symbol, "wadMul");
        assert_eq!(&source[wad.column..wad.column + wad.symbol.len()], "wadMul");
        assert_eq!(
            &source[price.column..price.column + price.symbol.len()],
            "price"
        );
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


/// Blank out everything that is not the function's body, keeping line and
/// column positions intact.
///
/// Call sites are extracted by wrapping the source in
/// `contract _C { function _f() { … } }`. A slice that still carries its own
/// signature puts a function declaration inside a function body, which is not
/// valid Solidity: the parse fails, the regex fallback runs, and it reads the
/// signature `foo(uint256 a)` as a call to `foo` — every function ended up
/// calling itself.
///
/// Blanking rather than trimming means a reported line is still the line in
/// `slice`, and a reported column is still the column in that line.
pub fn body_only(slice: &[String]) -> Vec<String> {
    let Some(open_index) = slice.iter().position(|line| line.contains('{')) else {
        return slice.to_vec();
    };

    let mut body: Vec<String> = slice.to_vec();
    for line in body.iter_mut().take(open_index) {
        *line = " ".repeat(line.len());
    }
    // Keep whatever follows the brace on its own line, blanking the rest.
    if let Some(brace) = body[open_index].find('{') {
        let tail = body[open_index][brace + 1..].to_string();
        body[open_index] = format!("{}{}", " ".repeat(brace + 1), tail);
    }

    // The closing brace would end the wrapper's function early.
    if let Some(close_index) = body.iter().rposition(|line| line.contains('}')) {
        if close_index > open_index {
            if let Some(brace) = body[close_index].rfind('}') {
                let head = body[close_index][..brace].to_string();
                body[close_index] = format!("{}{}", head, " ".repeat(body[close_index].len() - brace));
            }
        }
    }
    body
}

#[cfg(test)]
mod body_only_test {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(|line| line.to_string()).collect()
    }

    /// The signature must not be read as a call to the function itself.
    #[test]
    fn test_signature_is_not_a_call_site() {
        let slice = lines(
            "function depositWithReferral(uint256 assets, address receiver)\n             external\n             returns (uint256 mintedShares)\n             {\n             mintedShares = _deposit(msg.sender, receiver, assets);\n             }",
        );
        let sites = extract_call_sites_from_source(&body_only(&slice).join("\n"));
        let names: Vec<&str> = sites.iter().map(|site| site.name.as_str()).collect();

        assert!(!names.contains(&"depositWithReferral"), "got {names:?}");
        assert!(names.contains(&"_deposit"), "got {names:?}");
    }

    /// Blanking keeps every position, so anchors still land on the right token.
    #[test]
    fn test_positions_survive_blanking() {
        let slice = lines(
            "function quote(address asset) external view returns (uint256) {\n             return MathLib.wadMul(amount, price(asset));\n             }",
        );
        let sites = extract_call_sites_from_source(&body_only(&slice).join("\n"));

        let wad = sites.iter().find(|s| s.name == "MathLib.wadMul").unwrap();
        assert_eq!(wad.line, 2, "line numbers must still refer to the slice");
        assert_eq!(&slice[1][wad.column..wad.column + wad.symbol.len()], "wadMul");
    }

    /// A one-line body keeps its call.
    #[test]
    fn test_body_on_the_signature_line() {
        let slice = lines("function f() internal { return g(); }");
        let sites = extract_call_sites_from_source(&body_only(&slice).join("\n"));
        let names: Vec<&str> = sites.iter().map(|site| site.name.as_str()).collect();
        assert!(names.contains(&"g"), "got {names:?}");
        assert!(!names.contains(&"f"), "got {names:?}");
    }
}

#[cfg(test)]
mod ast_path_test {
    use super::*;

    /// Source where the two extractors must disagree.
    ///
    /// The regex fallback scans text, so it reads `ghost()` inside a block
    /// comment and `phantom()` inside a string literal as calls, and it takes
    /// `returns (` for a call to `returns`. The AST sees none of them. Asserting
    /// their absence is therefore an assertion that the AST path ran.
    const TRICKY: &str = r#"{
        /* ghost(); */
        string memory note = "phantom()";
        uint256 value = real(1);
    }"#;

    #[test]
    fn test_dependency_scan_uses_the_ast() {
        let names = extract_calls_from_source(TRICKY);
        assert!(names.contains(&"real".to_string()), "got {names:?}");
        assert!(!names.contains(&"ghost".to_string()), "comment read as a call: {names:?}");
        assert!(!names.contains(&"phantom".to_string()), "string read as a call: {names:?}");
    }

    #[test]
    fn test_call_sites_use_the_ast() {
        let sites = extract_call_sites_from_source(TRICKY);
        let names: Vec<&str> = sites.iter().map(|site| site.name.as_str()).collect();
        assert!(names.contains(&"real"), "got {names:?}");
        assert!(!names.contains(&"ghost"), "comment read as a call: {names:?}");
        assert!(!names.contains(&"phantom"), "string read as a call: {names:?}");
    }

    /// What the fallback would produce, kept as the contrast: if this ever
    /// matches the AST output, the probe above has stopped proving anything.
    #[test]
    fn test_the_fallback_really_is_fooled() {
        let names = extract_calls_regex(TRICKY);
        assert!(
            names.contains(&"ghost".to_string()) || names.contains(&"phantom".to_string()),
            "the regex fallback no longer differs from the AST: {names:?}"
        );
    }
}
