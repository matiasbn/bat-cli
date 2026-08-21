//! EVM deployment to Miro.
//!
//! Only [`auto_deploy`] lives here now. The earlier functions in this module —
//! `deploy_co_frames`, `deploy_co_screenshots`, `deploy_source_code_screenshots`
//! — belonged to the code-overhaul workflow, where a human placed screenshots
//! and drew the arrows by hand. `auto_deploy` computes the whole layout, so
//! there is nothing left for them to do.

pub mod auto_deploy;

use std::{error::Error, fmt};

#[derive(Debug)]
pub struct EvmMiroError;

impl fmt::Display for EvmMiroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EvmMiro error")
    }
}

impl Error for EvmMiroError {}

pub type EvmMiroResult<T> = error_stack::Result<T, EvmMiroError>;
