use cosmwasm_std::{Addr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ContractInfo {
    pub name: String,
    pub owner: Addr,
    pub treasury: Addr,
    pub projects_allocation_for_funds_fee: Uint128, // In 10th of percent
    pub projects_allocation_for_assets_fee: Uint128, // In 10th of percent
}
