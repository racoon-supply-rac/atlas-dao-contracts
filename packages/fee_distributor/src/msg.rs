use cosmwasm_std::{StdError, StdResult, Uint128};
use fee_contract_export::state::FeeType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utils::msg::is_valid_name;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub name: String,
    pub owner: Option<String>,
    pub treasury: String,
}

impl InstantiateMsg {
    pub fn validate(&self) -> StdResult<()> {
        // Check name, symbol, decimals
        if !is_valid_name(&self.name) {
            return Err(StdError::generic_err(
                "Name is not in the expected format (3-50 UTF-8 bytes)",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    ModifyContractInfo {
        owner: Option<String>,
        treasury: Option<String>,
        projects_allocation_for_assets_fee: Option<Uint128>,
        projects_allocation_for_funds_fee: Option<Uint128>,
    },
    DepositFees {
        addresses: Vec<String>,
        fee_type: FeeType,
    },
    WithdrawFees {
        addresses: Vec<String>,
    },
    AddAssociatedAddress {
        address: String,
        fee_address: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ContractInfo {},
    Amount {
        address: String,
    },
    Addresses {
        start_after: Option<String>,
        limit: Option<u32>,
    },
}
