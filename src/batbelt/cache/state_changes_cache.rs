use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct StateChangesCache {
    pub last_priority_parsed: usize,
    pub init_accounts_by_priority: Vec<InitializedAccountsByPriority>,
    pub program_account_state_changes: Vec<ProgramAccountStateChanges>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct InitializedAccountsByPriority {
    pub priority: usize,
    pub initialized_program_accounts: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProgramAccountStateChanges {
    pub account_name: String,
    pub init_entry_points: Vec<ValueState>,
    pub mut_entry_points: Vec<ValueState>,
    pub close_entry_points: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ValueState {
    pub entry_point_name: String,
    pub values: Vec<AccountValue>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AccountValue {
    pub name: String,
    pub value: AccountValueType,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub enum AccountValueType {
    #[default]
    Number,
    String,
    Pubkey,
    Struct,
}
