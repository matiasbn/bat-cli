use crate::batbelt::parser::ParserError;
use crate::batbelt::sonar::SonarResult;
use error_stack::Result;

pub enum SolanaAccountTypeParser {
    TokenAccount,
    TokenMint,
    Signer,
    UncheckedAccount,
    PDA,
    Other,
}

pub struct SolanaAccountParser {
    pub name: String,
    pub is_pda: bool,
    pub seeds: Option<String>,
    pub account_type: SolanaAccountTypeParser,
}

impl SolanaAccountParser {
    pub fn new(
        name: String,
        is_pda: bool,
        seeds: Option<String>,
        account_type: SolanaAccountTypeParser,
    ) -> Self {
        Self {
            name,
            is_pda,
            seeds,
            account_type,
        }
    }

    // pub fn new_from_ca_sonar_result(sonar_result: SonarResult) -> Result<Self, ParserError> {
    //     let last_line =
    // }
}

#[test]
fn test_get_solana_account_type() {
    let test_text_1 = "pub key: Signer<'info>,";
    let test_text_2 = "pub profile: AccountLoader<'info, Profile>,";
    let test_text_2 = "pub mint: Account<'info, Mint>,";
    let test_text_2 = "pub token_from: Account<'info, TokenAccount>,";
}
