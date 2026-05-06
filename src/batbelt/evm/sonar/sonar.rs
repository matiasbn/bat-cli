use colored::Colorize;
use dialoguer::console::Emoji;
use error_stack::{Report, ResultExt};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::Duration;
use walkdir::WalkDir;

use crate::batbelt::evm::metadata::bat_metadata::{EvmBatMetadata, FunctionDependency, EvmMetadataError, EvmMetadataResult};
use crate::batbelt::evm::parser::call_resolver::{CallResolver, extract_calls_from_source};
use crate::batbelt::evm::parser::import_resolver::ImportResolver;
use crate::batbelt::evm::parser::inheritance_resolver::InheritanceResolver;
use crate::batbelt::evm::parser::evm_file_parser::parse_sol_file;
use crate::batbelt::evm::types::EvmContract;

static BAT: Emoji<'_, '_> = Emoji("🦇", "BatSonar");
static FOLDER: Emoji<'_, '_> = Emoji("📂", "Folder");
static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", ":-)");
static WAVE: Emoji<'_, '_> = Emoji("〰", "-");

/// Orchestrator for the 5-phase EVM scanning process.
pub struct EvmSonar {
    project_root: String,
    contracts: Vec<EvmContract>,
    error_count: usize,
}

impl EvmSonar {
    pub fn new(project_root: &str) -> Self {
        Self {
            project_root: project_root.to_string(),
            contracts: Vec::new(),
            error_count: 0,
        }
    }

    fn create_spinner() -> ProgressBar {
        let spinner_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(100));
        pb.set_style(spinner_style);
        pb
    }

    /// Append an error line to Batlog.log immediately.
    fn log_error(&mut self, msg: &str) {
        self.error_count += 1;
        let log_path = "Batlog.log";
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
            let _ = writeln!(file, "[EvmSonar] {}", msg);
        }
    }

    fn sonar_start_animation(&self) {
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(100));
        pb.set_style(
            ProgressStyle::with_template("{spinner:.blue} {msg}")
                .unwrap()
                .tick_strings(&[
                    &format!("{}                  {}{}", FOLDER, WAVE, BAT),
                    &format!("{}                {}  {}", FOLDER, WAVE, BAT),
                    &format!("{}              {}    {}", FOLDER, WAVE, BAT),
                    &format!("{}            {}      {}", FOLDER, WAVE, BAT),
                    &format!("{}          {}        {}", FOLDER, WAVE, BAT),
                    &format!("{}        {}          {}", FOLDER, WAVE, BAT),
                    &format!("{}      {}            {}", FOLDER, WAVE, BAT),
                    &format!("{}    {}              {}", FOLDER, WAVE, BAT),
                    &format!("{}  {}                {}", FOLDER, WAVE, BAT),
                    &format!("{} {}", FOLDER, BAT),
                ]),
        );
        pb.set_message(format!("Initializing {}...", "EvmSonar".red()));
        std::thread::sleep(Duration::from_secs(1));
        pb.finish_and_clear();
    }

    /// Run all 5 phases of the EVM sonar scan.
    pub fn run(&mut self) -> EvmMetadataResult<EvmBatMetadata> {
        // Clear previous log
        let _ = std::fs::write("Batlog.log", "");
        self.sonar_start_animation();

        self.phase_1_source_scan()?;
        self.phase_2_imports_and_inheritance()?;
        self.phase_3_access_control()?;
        let deps = self.phase_4_function_dependencies()?;
        let metadata = self.phase_5_entry_points(deps)?;

        if self.error_count > 0 {
            println!(
                "  {} {} parse errors written to {}",
                "⚠".bright_yellow(),
                self.error_count,
                "Batlog.log".bright_cyan()
            );
        }

        Ok(metadata)
    }

    /// Phase 1: Parse all .sol files in src/ and lib/ directories.
    fn phase_1_source_scan(&mut self) -> EvmMetadataResult<()> {
        let import_resolver = ImportResolver::new(&self.project_root)
            .change_context(EvmMetadataError)?;
        let src_dir = import_resolver.get_src_dir().to_path_buf();
        let lib_dir = import_resolver.get_root_dir().join("lib");

        if !src_dir.is_dir() {
            return Err(Report::new(EvmMetadataError).attach_printable(format!(
                "Source directory not found: {}",
                src_dir.display()
            )));
        }

        let src_files = Self::collect_sol_files(&src_dir);
        let lib_files = if lib_dir.is_dir() {
            Self::collect_sol_files(&lib_dir)
        } else {
            vec![]
        };

        let total = src_files.len() + lib_files.len();
        let pb = Self::create_spinner();
        pb.set_message(format!("Source scan [0/{}]", total));

        let mut count = 0usize;

        // Parse src/ contracts
        for file_path in &src_files {
            count += 1;
            let short = file_path.split("/src/").last().unwrap_or(file_path);
            pb.set_message(format!("Source scan [{}/{}]: {}", count, total, short));
            match parse_sol_file(file_path) {
                Ok(sol_file) => {
                    for contract in sol_file.contracts {
                        self.contracts.push(contract);
                    }
                }
                Err(e) => {
                    self.log_error(&format!("Failed to parse [SRC] {}: {:?}", file_path, e));
                }
            }
        }

        // Parse lib/ contracts
        for file_path in &lib_files {
            count += 1;
            let short = file_path.split("/lib/").last().unwrap_or(file_path);
            pb.set_message(format!("Source scan [{}/{}]: [EXT] {}", count, total, short));
            match parse_sol_file(file_path) {
                Ok(sol_file) => {
                    for contract in sol_file.contracts {
                        self.contracts.push(contract);
                    }
                }
                Err(e) => {
                    self.log_error(&format!("Failed to parse [EXT] {}: {:?}", file_path, e));
                }
            }
        }

        let src_contracts = self.contracts.iter().filter(|c| !c.external).count();
        let ext_contracts = self.contracts.iter().filter(|c| c.external).count();
        let error_msg = if self.error_count > 0 {
            format!(", {} errors", self.error_count)
        } else {
            String::new()
        };

        pb.finish_with_message(format!(
            "{} Source scan: {} contracts ({} src, {} lib){}",
            SPARKLE, self.contracts.len(), src_contracts, ext_contracts, error_msg
        ));

        Ok(())
    }

    fn collect_sol_files(dir: &std::path::Path) -> Vec<String> {
        WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && e.path().extension().map(|ext| ext == "sol").unwrap_or(false)
                    && !e.path().to_str().unwrap_or("").contains("/test/")
                    && !e.path().to_str().unwrap_or("").contains("/tests/")
                    && !e.path().to_str().unwrap_or("").contains("/script/")
                    && !e.path().to_str().unwrap_or("").contains("/scripts/")
                    && !e.path().to_str().unwrap_or("").contains("/mock/")
                    && !e.path().to_str().unwrap_or("").contains("/mocks/")
            })
            .map(|e| e.path().to_str().unwrap().to_string())
            .collect()
    }

    /// Phase 2: Resolve imports and build inheritance graph.
    fn phase_2_imports_and_inheritance(&mut self) -> EvmMetadataResult<()> {
        let resolver = InheritanceResolver::new(&self.contracts);
        let contracts_with_bases: Vec<_> = self.contracts
            .iter()
            .filter(|c| !c.base_contracts.is_empty())
            .collect();

        let total = contracts_with_bases.len();
        let pb = Self::create_spinner();
        pb.set_message(format!("Inheritance [0/{}]", total));

        for (idx, contract) in contracts_with_bases.iter().enumerate() {
            let linearization = resolver.linearize(&contract.name);
            pb.set_message(format!(
                "Inheritance [{}/{}]: {} ({} bases)",
                idx + 1, total, contract.name, linearization.len() - 1
            ));
        }

        pb.finish_with_message(format!(
            "{} Inheritance: {} contracts with bases resolved",
            SPARKLE, total
        ));

        Ok(())
    }

    /// Phase 3: Detect access control patterns.
    fn phase_3_access_control(&self) -> EvmMetadataResult<()> {
        let total_modifiers: usize = self.contracts.iter().map(|c| c.modifiers.len()).sum();
        let pb = Self::create_spinner();
        pb.set_message(format!("Access control [0/{}]", total_modifiers));

        let mut count = 0usize;
        for contract in &self.contracts {
            for modifier in &contract.modifiers {
                count += 1;
                pb.set_message(format!(
                    "Access control [{}/{}]: {}.{}",
                    count, total_modifiers, contract.name, modifier.name
                ));
            }
        }

        pb.finish_with_message(format!(
            "{} Access control: {} modifiers detected",
            SPARKLE, total_modifiers
        ));
        Ok(())
    }

    /// Phase 4: Resolve function call dependencies using AST.
    /// Returns a Vec of FunctionDependency to be persisted in metadata.
    fn phase_4_function_dependencies(&self) -> EvmMetadataResult<Vec<FunctionDependency>> {
        let total_functions: usize = self.contracts.iter().map(|c| c.functions.len()).sum();
        let pb = Self::create_spinner();
        pb.set_message(format!("Function dependencies [0/{}]", total_functions));

        let mut all_deps: Vec<FunctionDependency> = Vec::new();
        let mut count = 0usize;
        let mut total_calls = 0usize;

        for contract in &self.contracts {
            for function in &contract.functions {
                count += 1;
                pb.set_message(format!(
                    "Function dependencies [{}/{}]: {}.{}",
                    count, total_functions, contract.name, function.name
                ));

                // Use AST-based call extraction on the function body
                let callees = extract_calls_from_source(&function.body_source);
                total_calls += callees.len();

                let func_id = format!("{}_{}_{}", contract.file_path, contract.name, function.name);
                all_deps.push(FunctionDependency {
                    function_metadata_id: func_id,
                    callees,
                });
            }
        }

        pb.finish_with_message(format!(
            "{} Function dependencies: {} functions, {} calls resolved",
            SPARKLE, total_functions, total_calls
        ));

        Ok(all_deps)
    }

    /// Phase 5: Build entry points and generate final metadata.
    fn phase_5_entry_points(&self, deps: Vec<FunctionDependency>) -> EvmMetadataResult<EvmBatMetadata> {
        let pb = Self::create_spinner();
        pb.set_message("Building entry points...");

        let mut metadata = EvmBatMetadata::from_contracts(self.contracts.clone());
        metadata.function_dependencies = deps;
        metadata.save_metadata()?;

        pb.finish_with_message(format!(
            "{} Entry points: {} detected across {} contracts",
            SPARKLE,
            metadata.entry_points.len(),
            metadata.contracts.iter().filter(|c| !c.external).count()
        ));

        Ok(metadata)
    }
}
