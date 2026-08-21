use crate::batbelt::metadata::BatMetadata;
use crate::batbelt::sonar::SonarResultType;

use crate::batbelt::BatEnumerator;
use clap::Subcommand;

use crate::batbelt::path::BatFile;
use error_stack::{Result, ResultExt};

use crate::batbelt::sonar::sonar_interactive::BatSonarInteractive;
use crate::batbelt::templates::TemplateGenerator;
use crate::commands::{BatCommandEnumerator, CommandResult};
use crate::config::{BatConfig, ProjectType};

use super::CommandError;

#[derive(Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter)]
pub enum SonarCommand {
    /// Rescan the source code and rebuild the metadata
    Run,
}
impl BatEnumerator for SonarCommand {}

impl BatCommandEnumerator for SonarCommand {
    fn execute_command(&self) -> CommandResult<()> {
        match self {
            SonarCommand::Run => self.execute_run(),
        }
    }

    fn check_metadata_is_initialized(&self) -> bool {
        false
    }
}

impl SonarCommand {
    fn execute_run(&self) -> CommandResult<()> {
        let bat_config = BatConfig::get_config().change_context(CommandError)?;

        if bat_config.project_type == ProjectType::Foundry {
            self.execute_run_foundry()?;
        } else {
            self.execute_run_svm()?;
        }

        // A rescan is the canonical "this project caught up with the binary" moment: stamp
        // the version into Bat.toml and regenerate the guide an assistant reads. Doing it
        // here also covers `init`, which finishes by scanning.
        crate::guide::refresh_project_ai_surface();
        Ok(())
    }

    fn execute_run_foundry(&self) -> CommandResult<()> {
        use crate::batbelt::evm::sonar::sonar::EvmSonar;

        let mut evm_sonar = EvmSonar::new(".");
        evm_sonar.run().change_context(CommandError)?;

        Ok(())
    }

    fn execute_run_svm(&self) -> CommandResult<()> {
        let metadata_bat_file = BatFile::BatMetadataFile;
        // in case the file does not exist, so the BatMetadata can be read
        if !metadata_bat_file
            .file_exists()
            .change_context(CommandError)?
        {
            TemplateGenerator
                .create_metadata_json()
                .change_context(CommandError)?;
        }
        let metadata_bkp_bat_file = BatFile::Generic {
            file_path: "BatMetadata_backup.json".to_string(),
        };

        // if the backup exists, then the previous process didn't finished successfully, so we use it to reload sensitive data
        if metadata_bkp_bat_file
            .file_exists()
            .change_context(CommandError)?
        {
            let metadata_bkp_content = metadata_bkp_bat_file
                .read_content(false)
                .change_context(CommandError)?;
            metadata_bat_file
                .write_content(false, &metadata_bkp_content)
                .change_context(CommandError)?;
        }

        // backup co metadata
        let metadata_content = metadata_bat_file
            .read_content(false)
            .change_context(CommandError)?;
        metadata_bkp_bat_file
            .write_content(false, &metadata_content)
            .change_context(CommandError)?;

        // create new file
        TemplateGenerator
            .create_metadata_json()
            .change_context(CommandError)?;
        BatSonarInteractive::SonarStart {
            sonar_result_type: SonarResultType::Struct,
        }
        .print_interactive()
        .change_context(CommandError)?;
        self.execute_source_code()?;
        BatSonarInteractive::run_post_scan_parallel().change_context(CommandError)?;

        let mut bat_metadata = BatMetadata::read_metadata().change_context(CommandError)?;
        bat_metadata.initialized = true;
        bat_metadata.save_metadata().change_context(CommandError)?;

        // delete metadata backup
        metadata_bkp_bat_file
            .remove_file()
            .change_context(CommandError)?;

        Ok(())
    }

    fn execute_source_code(&self) -> Result<(), CommandError> {
        BatSonarInteractive::GetSourceCodeMetadata
            .print_interactive()
            .change_context(CommandError)?;
        Ok(())
    }
}
