use crate::state::{AssetInfo, CounterTradeInfo};
use cosmwasm_std::{to_binary, Binary, CosmosMsg, StdError, StdResult, WasmMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 3 || bytes.len() > 50 {
        return false;
    }
    true
}

pub fn into_binary<M: Serialize>(msg: M) -> StdResult<Binary> {
    to_binary(&msg)
}

pub fn into_cosmos_msg<M: Serialize, T: Into<String>>(
    message: M,
    contract_addr: T,
) -> StdResult<CosmosMsg> {
    let msg = into_binary(message)?;
    let execute = WasmMsg::Execute {
        contract_addr: contract_addr.into(),
        msg,
        funds: vec![],
    };
    Ok(execute.into())
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub name: String,
    pub owner: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct MigrateMsg {}

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
pub enum AddAssetAction{
    ToLastTrade{},
    ToLastCounterTrade{
        trade_id: u64
    },
    ToTrade{
        trade_id: u64,
    },
    ToCounterTrade{
        trade_id: u64,
        counter_id: u64
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    CreateTrade {
        whitelisted_users: Option<Vec<String>>,
        comment: Option<String>,
    },
    AddAsset {
        action: AddAssetAction,
        asset: AssetInfo,
    },
    RemoveAssets {
        trade_id: u64,
        counter_id: Option<u64>,
        assets: Vec<(u16, AssetInfo)>,
    },
    AddWhitelistedUsers {
        trade_id: u64,
        whitelisted_users: Vec<String>,
    },
    RemoveWhitelistedUsers {
        trade_id: u64,
        whitelisted_users: Vec<String>,
    },
    SetComment {
        trade_id: u64,
        counter_id: Option<u64>,
        comment: String,
    },
    AddNFTsWanted {
        trade_id: Option<u64>,
        nfts_wanted: Vec<String>,
    },
    RemoveNFTsWanted {
        trade_id: u64,
        nfts_wanted: Vec<String>,
    },
    /// Is used by the Trader to confirm they completed their end of the trade.
    ConfirmTrade {
        trade_id: Option<u64>,
    },
    /// Can be used to initiate Counter Trade, but also to add new tokens to it
    SuggestCounterTrade {
        trade_id: u64,
        comment: Option<String>,
    },
    /// Is used by the Client to confirm they completed their end of the trade.
    ConfirmCounterTrade {
        trade_id: u64,
        counter_id: Option<u64>,
    },
    /// Accept the Trade plain and simple, swap it up !
    AcceptTrade {
        trade_id: u64,
        counter_id: u64,
        comment: Option<String>,
    },
    /// Cancel the Trade :/ No luck there mate ?
    CancelTrade {
        trade_id: u64,
    },
    /// Cancel the Counter Trade :/ No luck there mate ?
    CancelCounterTrade {
        trade_id: u64,
        counter_id: u64,
    },
    /// Refuse the Trade plain and simple, no madam, I'm not interested in your tokens !
    RefuseCounterTrade {
        trade_id: u64,
        counter_id: u64,
    },
    /// Some parts of the traded tokens were interesting, but you can't accept the trade as is
    ReviewCounterTrade {
        trade_id: u64,
        counter_id: u64,
        comment: Option<String>,
    },
    /// The fee contract can Withdraw funds via this function only when the trade is accepted.
    WithdrawPendingAssets {
        trader: String,
        trade_id: u64,
    },
    /// You can Withdraw funds only at specific steps of the trade, but you're allowed to try anytime !
    WithdrawAllFromTrade {
        trade_id: u64,
    },
    /// You can Withdraw funds when your counter trade is aborted (refused or cancelled)
    /// Or when you are creating the trade and you just want to cancel it all
    WithdrawAllFromCounter {
        trade_id: u64,
        counter_id: u64,
    },
    SetNewOwner {
        owner: String,
    },
    SetNewFeeContract {
        fee_contract: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub struct QueryFilters {
    pub states: Option<Vec<String>>,
    pub owner: Option<String>,
    pub counterer: Option<String>,
    pub has_whitelist: Option<bool>,
    pub whitelisted_user: Option<String>,
    pub contains_token: Option<String>,
    pub wanted_nft: Option<String>,
    pub assets_withdrawn: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ContractInfo {},
    TradeInfo {
        trade_id: u64,
    },
    CounterTradeInfo {
        trade_id: u64,
        counter_id: u64,
    },
    GetAllTrades {
        start_after: Option<u64>,
        limit: Option<u32>,
        filters: Option<QueryFilters>,
    },
    GetCounterTrades {
        trade_id: u64,
        start_after: Option<u64>,
        limit: Option<u32>,
        filters: Option<QueryFilters>,
    },
    GetAllCounterTrades {
        start_after: Option<CounterTradeInfo>,
        limit: Option<u32>,
        filters: Option<QueryFilters>,
    },
}
