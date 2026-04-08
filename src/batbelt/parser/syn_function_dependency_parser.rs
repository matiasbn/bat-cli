use syn::visit::Visit;

#[derive(Clone, Debug, PartialEq)]
pub struct DetectedCall {
    pub function_name: String,
    pub call_type: CallType,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CallType {
    FreeFunction,
    StaticMethod { type_name: String },
    MethodCall,
}

const FILTERED_NAMES: &[&str] = &[
    "Ok", "Some", "Err", "None", "vec", "format", "println", "eprintln", "print", "eprint",
    "panic", "todo", "unimplemented", "unreachable", "assert", "assert_eq", "assert_ne",
    "debug_assert", "debug_assert_eq", "debug_assert_ne", "write", "writeln", "log",
    "cfg", "include", "include_str", "include_bytes", "env", "option_env",
    "concat", "stringify", "file", "line", "column", "module_path",
    "Box", "Vec", "String", "Arc", "Rc", "Mutex", "RefCell",
];

const FILTERED_METHOD_NAMES: &[&str] = &[
    "unwrap", "expect", "clone", "to_string", "to_owned",
    "iter", "into_iter", "map", "filter", "collect", "fold", "for_each", "find", "any", "all",
    "push", "pop", "len", "is_empty", "contains", "get", "insert", "remove", "extend",
    "ok_or", "ok_or_else", "map_err", "and_then", "or_else", "unwrap_or", "unwrap_or_else",
    "as_ref", "as_mut", "borrow", "borrow_mut",
    "into", "from", "try_into", "try_from",
    "default", "to_vec", "as_slice", "as_str",
    "change_context", "attach_printable",
    "is_some", "is_none", "is_ok", "is_err",
    "trim", "trim_start", "trim_end", "split", "join", "replace", "starts_with", "ends_with",
    "lines", "chars", "bytes",
    "next", "enumerate", "skip", "take", "zip", "chain", "flat_map", "flatten",
    "filter_map", "position", "count",
    "sort", "sort_by", "sort_by_key", "dedup",
];

pub fn detect_function_calls(function_source: &str) -> Result<Vec<DetectedCall>, String> {
    let item_fn = syn::parse_str::<syn::ItemFn>(function_source).or_else(|_| {
        let wrapped = format!("fn __wrapper() {{ {} }}", function_source);
        syn::parse_str::<syn::ItemFn>(&wrapped)
    });

    let item_fn = match item_fn {
        Ok(f) => f,
        Err(e) => return Err(format!("syn parse error: {}", e)),
    };

    let mut visitor = CallVisitor {
        calls: Vec::new(),
    };
    visitor.visit_item_fn(&item_fn);

    // Deduplicate by function_name
    let mut seen = std::collections::HashSet::new();
    visitor.calls.retain(|call| {
        if FILTERED_NAMES.contains(&call.function_name.as_str()) {
            return false;
        }
        seen.insert(call.function_name.clone())
    });

    Ok(visitor.calls)
}

struct CallVisitor {
    calls: Vec<DetectedCall>,
}

impl<'ast> Visit<'ast> for CallVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(expr_path) = &*node.func {
            let segments = &expr_path.path.segments;
            let len = segments.len();
            if len == 1 {
                let name = segments[0].ident.to_string();
                self.calls.push(DetectedCall {
                    function_name: name,
                    call_type: CallType::FreeFunction,
                });
            } else if len >= 2 {
                let type_name = segments[len - 2].ident.to_string();
                let func_name = segments[len - 1].ident.to_string();
                self.calls.push(DetectedCall {
                    function_name: func_name,
                    call_type: CallType::StaticMethod { type_name },
                });
            }
        }
        // Visit arguments recursively to find nested calls
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let name = node.method.to_string();
        if !FILTERED_METHOD_NAMES.contains(&name.as_str()) {
            self.calls.push(DetectedCall {
                function_name: name,
                call_type: CallType::MethodCall,
            });
        }
        // Visit receiver and arguments recursively
        syn::visit::visit_expr_method_call(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_free_function() {
        let source = r#"fn test() { foo(1, 2); }"#;
        let calls = detect_function_calls(source).unwrap();
        assert!(calls.iter().any(|c| c.function_name == "foo"
            && c.call_type == CallType::FreeFunction));
    }

    #[test]
    fn test_detect_static_method() {
        let source = r#"fn test() { MyStruct::do_something(x); }"#;
        let calls = detect_function_calls(source).unwrap();
        assert!(calls.iter().any(|c| c.function_name == "do_something"
            && c.call_type == CallType::StaticMethod {
                type_name: "MyStruct".to_string()
            }));
    }

    #[test]
    fn test_detect_method_call() {
        let source = r#"fn test() { obj.method(arg); }"#;
        let calls = detect_function_calls(source).unwrap();
        assert!(calls.iter().any(|c| c.function_name == "method"
            && c.call_type == CallType::MethodCall));
    }

    #[test]
    fn test_filters_common_names() {
        let source = r#"fn test() { Ok(value); Some(x); vec![1,2]; foo(1); }"#;
        let calls = detect_function_calls(source).unwrap();
        assert!(!calls.iter().any(|c| c.function_name == "Ok"));
        assert!(!calls.iter().any(|c| c.function_name == "Some"));
        assert!(calls.iter().any(|c| c.function_name == "foo"));
    }

    #[test]
    fn test_deduplicates() {
        let source = r#"fn test() { foo(1); foo(2); foo(3); }"#;
        let calls = detect_function_calls(source).unwrap();
        let foo_count = calls.iter().filter(|c| c.function_name == "foo").count();
        assert_eq!(foo_count, 1);
    }

    #[test]
    fn test_nested_calls() {
        let source = r#"fn test() { outer(inner(x)); }"#;
        let calls = detect_function_calls(source).unwrap();
        assert!(calls.iter().any(|c| c.function_name == "outer"));
        assert!(calls.iter().any(|c| c.function_name == "inner"));
    }

    #[test]
    fn test_chained_methods() {
        let source = r#"fn test() { x.foo().bar().baz(); }"#;
        let calls = detect_function_calls(source).unwrap();
        assert!(calls.iter().any(|c| c.function_name == "foo"));
        assert!(calls.iter().any(|c| c.function_name == "bar"));
        assert!(calls.iter().any(|c| c.function_name == "baz"));
    }

    #[test]
    fn test_closure_calls() {
        let source = r#"fn test() { let f = |x| compute(x); items.iter().map(|i| transform(i)); }"#;
        let calls = detect_function_calls(source).unwrap();
        assert!(calls.iter().any(|c| c.function_name == "compute"));
        assert!(calls.iter().any(|c| c.function_name == "transform"));
    }

    #[test]
    fn test_body_only_fallback() {
        let source = r#"let x = foo(1); bar(x);"#;
        let calls = detect_function_calls(source).unwrap();
        assert!(calls.iter().any(|c| c.function_name == "foo"));
        assert!(calls.iter().any(|c| c.function_name == "bar"));
    }

    #[test]
    fn test_long_path_static() {
        let source = r#"fn test() { module::SubModule::create(arg); }"#;
        let calls = detect_function_calls(source).unwrap();
        assert!(calls.iter().any(|c| c.function_name == "create"
            && c.call_type == CallType::StaticMethod {
                type_name: "SubModule".to_string()
            }));
    }
}
