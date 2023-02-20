use std::vec;

pub const MIRO_ACCOUNTS_SUBSECTION_FRAME_URL_HEADER: &str = "### Accounts frame url";
pub const MIRO_SUBSECTIONS_HEADERS: &[&str] = &["## Entrypoints", "## Accounts"];
pub const METADATA_CONTENT_STICKY_NOTE_COLOR_SECTION: &str = "- sticky_note_color:";
pub const METADATA_CONTENT_MIRO_ITEM_ID_SECTION: &str = "- miro_item_id:";

#[derive(Debug, Clone)]
pub struct MiroAccountMetadata {
    pub sticky_note_color: String,
    pub account_name: String,
    pub miro_item_id: String,
}

pub fn get_format_miro_accounts_to_result_string(
    miro_accounts_vec: Vec<MiroAccountMetadata>,
    subsection_header: &str,
) -> String {
    let mut sorted_vec = miro_accounts_vec.clone();
    sorted_vec.sort_by(|miro_a, miro_b| miro_a.account_name.cmp(&miro_b.account_name));
    let mut initial_vec = vec![format!("{}\n\n", subsection_header.to_string())];
    let mut result_vec = sorted_vec
        .iter()
        .enumerate()
        .map(|(miro_result_index, miro_result)| {
            format!(
                "{}{}{}",
                format!("### {}\n\n", miro_result.account_name),
                format!(
                    "{} {}\n",
                    METADATA_CONTENT_STICKY_NOTE_COLOR_SECTION,
                    miro_result.sticky_note_color.to_string()
                ),
                if miro_result_index == sorted_vec.len() - 1 {
                    // last
                    format!(
                        "{} {}",
                        METADATA_CONTENT_MIRO_ITEM_ID_SECTION, miro_result.miro_item_id
                    )
                } else {
                    format!(
                        "{} {}\n\n",
                        METADATA_CONTENT_MIRO_ITEM_ID_SECTION, miro_result.miro_item_id
                    )
                }
            )
        })
        .collect::<Vec<_>>();
    initial_vec.append(&mut result_vec);
    initial_vec.join("")
}
