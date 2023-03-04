use crate::batbelt::metadata::{BatMetadata, MetadataId, MetadataResult};
use crate::batbelt::miro::MiroColor;
use crate::batbelt::BatEnumerator;
use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    PartialEq,
    Clone,
    Copy,
    strum_macros::Display,
    strum_macros::EnumIter,
    Serialize,
    Deserialize,
)]
pub enum SignerType {
    Validated,
    NotValidated,
    Permissionless,
}

impl BatEnumerator for SignerType {}

impl SignerType {
    pub fn get_sticky_note_color(&self) -> MiroColor {
        match self {
            SignerType::Validated => MiroColor::Red,
            SignerType::NotValidated => MiroColor::DarkBlue,
            SignerType::Permissionless => MiroColor::Gray,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SignerInfo {
    pub signer_text: String,
    pub sticky_note_id: String,
    pub user_figure_id: String,
    pub signer_type: SignerType,
}

impl SignerInfo {
    pub fn new(
        signer_text: String,
        sticky_note_id: String,
        user_figure_id: String,
        signer_type: SignerType,
    ) -> Self {
        Self {
            signer_text,
            sticky_note_id,
            user_figure_id,
            signer_type,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MiroCodeOverhaulMetadata {
    pub metadata_id: String,
    pub entry_point_name: String,
    pub miro_frame_id: String,
    pub images_deployed: bool,
    pub entry_point_image_id: String,
    pub context_accounts_image_id: String,
    pub validations_image_id: String,
    pub handler_image_id: String,
    pub signers: Vec<SignerInfo>,
}

impl MiroCodeOverhaulMetadata {
    pub fn update_code_overhaul_metadata(&self) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        let position = bat_metadata
            .clone()
            .miro
            .code_overhaul
            .into_iter()
            .position(|ep| ep.entry_point_name == self.entry_point_name);
        match position {
            None => bat_metadata.miro.code_overhaul.push(self.clone()),
            Some(pos) => bat_metadata.miro.code_overhaul[pos] = self.clone(),
        };
        bat_metadata.entry_points.sort_by_key(|ep| ep.name.clone());
        bat_metadata.save_metadata()?;
        Ok(())
    }
}
