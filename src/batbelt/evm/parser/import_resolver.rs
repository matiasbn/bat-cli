use error_stack::{Report, ResultExt};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{EvmParserError, EvmParserResult};

/// Resolves Solidity import paths using Foundry remappings.
pub struct ImportResolver {
    /// Map from remapping prefix to replacement path.
    remappings: HashMap<String, String>,
    /// Base source directory (from foundry.toml `src`).
    src_dir: PathBuf,
    /// Project root directory.
    root_dir: PathBuf,
}

impl ImportResolver {
    /// Create a new resolver by reading foundry.toml and remappings.txt.
    pub fn new(project_root: &str) -> EvmParserResult<Self> {
        let root_dir = PathBuf::from(project_root);
        let src_dir = Self::detect_src_dir(&root_dir)?;
        let remappings = Self::load_remappings(&root_dir)?;

        Ok(Self {
            remappings,
            src_dir,
            root_dir,
        })
    }

    /// Resolve an import path to an absolute file path.
    pub fn resolve(&self, import_path: &str, from_file: &str) -> Option<PathBuf> {
        // Try remappings first
        for (prefix, replacement) in &self.remappings {
            if import_path.starts_with(prefix.as_str()) {
                let resolved = import_path.replacen(prefix.as_str(), replacement.as_str(), 1);
                let full_path = self.root_dir.join(&resolved);
                if full_path.is_file() {
                    return Some(full_path);
                }
            }
        }

        // Try relative path from current file
        let from_dir = Path::new(from_file).parent()?;
        let relative = from_dir.join(import_path);
        if relative.is_file() {
            return Some(relative);
        }

        // Try from project root
        let from_root = self.root_dir.join(import_path);
        if from_root.is_file() {
            return Some(from_root);
        }

        // Try from src dir
        let from_src = self.src_dir.join(import_path);
        if from_src.is_file() {
            return Some(from_src);
        }

        // Try lib/ (common for Foundry dependencies)
        let from_lib = self.root_dir.join("lib").join(import_path);
        if from_lib.is_file() {
            return Some(from_lib);
        }

        // Try node_modules (Hardhat-style)
        let from_node_modules = self.root_dir.join("node_modules").join(import_path);
        if from_node_modules.is_file() {
            return Some(from_node_modules);
        }

        None
    }

    fn detect_src_dir(root: &Path) -> EvmParserResult<PathBuf> {
        let foundry_toml_path = root.join("foundry.toml");
        if foundry_toml_path.is_file() {
            let content = fs::read_to_string(&foundry_toml_path).map_err(|e| {
                Report::new(EvmParserError)
                    .attach_printable(format!("Cannot read foundry.toml: {}", e))
            })?;

            // Simple TOML parsing for src field
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("src") {
                    if let Some(value) = trimmed.split('=').nth(1) {
                        let src = value.trim().trim_matches('"').trim_matches('\'');
                        return Ok(root.join(src));
                    }
                }
            }
        }

        // Default Foundry src directory
        Ok(root.join("src"))
    }

    fn load_remappings(root: &Path) -> EvmParserResult<HashMap<String, String>> {
        let mut remappings = HashMap::new();

        // Load from remappings.txt
        let remappings_file = root.join("remappings.txt");
        if remappings_file.is_file() {
            let content = fs::read_to_string(&remappings_file).unwrap_or_default();
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((prefix, replacement)) = line.split_once('=') {
                    remappings.insert(prefix.to_string(), replacement.to_string());
                }
            }
        }

        // Also check foundry.toml for inline remappings
        let foundry_toml = root.join("foundry.toml");
        if foundry_toml.is_file() {
            let content = fs::read_to_string(&foundry_toml).unwrap_or_default();
            let mut in_remappings = false;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("remappings") && trimmed.contains('[') {
                    in_remappings = true;
                    continue;
                }
                if in_remappings {
                    if trimmed.contains(']') {
                        in_remappings = false;
                        continue;
                    }
                    let cleaned = trimmed.trim_matches(|c| c == '"' || c == '\'' || c == ',');
                    if let Some((prefix, replacement)) = cleaned.split_once('=') {
                        remappings.insert(prefix.to_string(), replacement.to_string());
                    }
                }
            }
        }

        Ok(remappings)
    }

    pub fn get_src_dir(&self) -> &Path {
        &self.src_dir
    }

    pub fn get_root_dir(&self) -> &Path {
        &self.root_dir
    }
}
