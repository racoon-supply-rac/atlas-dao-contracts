use cosmwasm_std::{StdError, StdResult, Uint128};
use p2p_trading_export::state::AssetInfo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utils::msg::is_valid_name;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub name: String,
    pub owner: Option<String>,
    pub p2p_contract: String,
    pub fee_distributor: String,
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
    PayFeeAndWithdraw {
        trade_id: u64,
    },
    UpdateFeeRates {
        asset_fee_rate: Option<Uint128>, // In thousandths (fee rate for liquid assets (terra native funds))
        fee_max: Option<Uint128>, // In uusd (max asset fee paid (outside of terra native funds))
        first_teer_limit: Option<Uint128>, // Max number of NFT to fall into the first tax teer
        first_teer_rate: Option<Uint128>, // Fee per asset in the first teer
        second_teer_limit: Option<Uint128>, // Max number of NFT to fall into the second tax teer
        second_teer_rate: Option<Uint128>, // Fee per asset in the second teer
        third_teer_rate: Option<Uint128>, // Fee per asset in the third teer
        acceptable_fee_deviation: Option<Uint128>, // To account for fluctuations in terra native prices, we allow the provided fee the deviate from the quoted fee (non simultaeous operations)
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Fee {
        trade_id: u64,
        counter_id: Option<u64>,
    },
    SimulateFee {
        trade_id: u64,
        counter_assets: Vec<AssetInfo>,
    },
    ContractInfo {},
    FeeRates {},
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct FeeResponse {
    pub amount: Uint128,
    pub denom: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct FeeRawResponse {
    pub assets_fee: Uint128,
    pub funds_fee: Uint128,
}
