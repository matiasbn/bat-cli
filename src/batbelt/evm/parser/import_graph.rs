//! Which source files a given file can see through its imports.
//!
//! Solidity does not have a global namespace. Two contracts named `BeaconProxy` in two
//! vendored copies of OpenZeppelin are different contracts, and which one a name refers to
//! is decided by the `import` statements of the file that uses it, resolved through
//! `remappings.txt` — exactly the way `solc` decides it.
//!
//! The metadata stores contracts by name, and name lookups used to take the first match in
//! the whole project. With three copies of a library vendored under `lib/`, that silently
//! drew a copy the audited code never touches. This module answers the question the
//! compiler already answers, so the lookup can too.
//!
//! Everything here is a pure function of the files on disk, memoised per process: a deploy
//! runs from the project root and the tree does not change underneath it.

use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use super::import_resolver::ImportResolver;

static RESOLVER: Lazy<Option<ImportResolver>> = Lazy::new(|| ImportResolver::new(".").ok());

static CLOSURES: Lazy<Mutex<HashMap<String, Arc<Vec<String>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

static IMPORT: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r#"\bimport\b[^;"']*["']([^"']+)["']"#).unwrap());

static BLOCK_COMMENT: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"(?s)/\*.*?\*/").unwrap());

static LINE_COMMENT: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"//[^\n]*").unwrap());

/// A path in the one canonical spelling used for comparisons: no leading `./`, no `.` or
/// `..` segments. Purely lexical, so it agrees with the relative paths stored in the
/// metadata (`./lib/...`) whether or not a symlink sits in between.
pub fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// The files `file` imports directly, resolved and normalised.
fn direct_imports(file: &str) -> Vec<String> {
    let Some(resolver) = RESOLVER.as_ref() else {
        return Vec::new();
    };
    let Ok(source) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    // Strip comments first, so a commented-out import does not count.
    let source = BLOCK_COMMENT.replace_all(&source, "");
    let source = LINE_COMMENT.replace_all(&source, "");
    IMPORT
        .captures_iter(&source)
        .filter_map(|capture| resolver.resolve(&capture[1], file))
        .map(|path| normalize(&path.to_string_lossy()))
        .collect()
}

/// Every file reachable from `file` through imports, NEAREST FIRST, starting with `file`
/// itself.
///
/// Breadth-first order is what lets a lookup prefer the copy a file imports directly over
/// one it only reaches three hops away, when a project happens to pull in both.
pub fn import_closure(file: &str) -> Arc<Vec<String>> {
    let start = normalize(file);
    if let Some(cached) = CLOSURES.lock().unwrap().get(&start) {
        return cached.clone();
    }

    let mut order: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::from([start.clone()]);
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current.clone()) {
            continue;
        }
        for next in direct_imports(&current) {
            if !seen.contains(&next) {
                queue.push_back(next);
            }
        }
        order.push(current);
    }

    let closure = Arc::new(order);
    CLOSURES.lock().unwrap().insert(start, closure.clone());
    closure
}

/// Every file reachable through imports from ANY of `roots`, the roots included.
///
/// One breadth-first walk with a shared visited set, rather than one closure per root:
/// seeded with every in-scope file of a project, per-root closures would re-walk the same
/// library trees hundreds of times.
pub fn reachable_from<'a>(roots: impl Iterator<Item = &'a str>) -> HashSet<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = roots.map(normalize).collect();
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current.clone()) {
            continue;
        }
        for next in direct_imports(&current) {
            if !seen.contains(&next) {
                queue.push_back(next);
            }
        }
    }
    seen
}

#[cfg(test)]
mod import_graph_test {
    use super::normalize;

    #[test]
    fn normalize_agrees_with_the_paths_the_metadata_stores() {
        assert_eq!(normalize("./lib/oz/contracts/Proxy.sol"), "lib/oz/contracts/Proxy.sol");
        assert_eq!(normalize("lib/oz/contracts/Proxy.sol"), "lib/oz/contracts/Proxy.sol");
        assert_eq!(
            normalize("./lib/oz/contracts/proxy/beacon/../ERC1967/ERC1967Upgrade.sol"),
            "lib/oz/contracts/proxy/ERC1967/ERC1967Upgrade.sol"
        );
        assert_eq!(normalize("a/./b//c"), "a/b/c");
    }
}
