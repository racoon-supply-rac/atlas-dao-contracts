
use cw20::Cw20ReceiveMsg;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{to_json_binary, Binary, CosmosMsg, StdError, StdResult, WasmMsg, Decimal};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::ContractInfo;
use crate::state::RaffleInfo;
use crate::state::RaffleState;
use crate::state::{AssetInfo, RaffleOptionsMsg};

fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 3 || bytes.len() > 50 {
        return false;
    }
    true
}

pub fn into_binary<M: Serialize>(msg: M) -> StdResult<Binary> {
    to_json_binary(&msg)
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
    pub fee_addr: Option<String>,
    pub minimum_raffle_duration: Option<u64>,
    pub minimum_raffle_timeout: Option<u64>,
    pub max_participant_number: Option<u32>,
    pub raffle_fee: Option<Decimal>, 
    pub rand_fee: Option<Decimal>,   
    pub drand_url: Option<String>,
    pub random_pubkey: String,
    pub verify_signature_contract: String,
}

#[cw_serde]
pub struct MigrateMsg {}

impl InstantiateMsg {
    pub fn validate(&self) -> StdResult<()> {
        // Check name
        if !is_valid_name(&self.name) {
            return Err(StdError::generic_err(
                "Name is not in the expected format (3-50 UTF-8 bytes)",
            ));
        }
        Ok(())
    }
}

#[cw_serde]
pub struct DrandRandomness {
    pub round: u64,
    pub previous_signature: Binary,
    pub signature: Binary,
}

#[cw_serde]
pub enum ExecuteMsg {
    CreateRaffle {
        owner: Option<String>,
        assets: Vec<AssetInfo>,
        raffle_options: RaffleOptionsMsg,
        raffle_ticket_price: AssetInfo,
    },
    CancelRaffle {
        raffle_id: u64,
    },
    ModifyRaffle {
        raffle_id: u64,
        raffle_ticket_price: Option<AssetInfo>,
        raffle_options: RaffleOptionsMsg,
    },
    BuyTicket {
        raffle_id: u64,
        ticket_number: u32,
        sent_assets: AssetInfo,
    },
    Receive(Cw20ReceiveMsg),
    ClaimNft {
        raffle_id: u64,
    },
    UpdateRandomness {
        raffle_id: u64,
        randomness: DrandRandomness,
    },

    // Admin messages
    ToggleLock {
        lock: bool,
    },
    ChangeParameter {
        parameter: String,
        value: String,
    },
    ClaimOwnership { }
}

#[cw_serde]
pub struct QueryFilters {
    pub states: Option<Vec<String>>,
    pub owner: Option<String>,
    pub ticket_depositor: Option<String>,
    pub contains_token: Option<String>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ContractInfo)]
    ContractInfo {},
    #[returns(RaffleResponse)]
    RaffleInfo { raffle_id: u64 },
    #[returns(AllRafflesResponse)]
    AllRaffles {
        start_after: Option<u64>,
        limit: Option<u32>,
        filters: Option<QueryFilters>,
    },
    #[returns(Vec<String>)]
    AllTickets {
        raffle_id: u64,
        start_after: Option<u32>,
        limit: Option<u32>,
    },
    #[returns(u32)]
    TicketNumber { owner: String, raffle_id: u64 },
}

#[cw_serde]
pub enum VerifierExecuteMsg {
    Verify {
        randomness: DrandRandomness,
        pubkey: Binary,
        raffle_id: u64,
        owner: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum VerifierQueryMsg {}

#[cw_serde]
pub struct RaffleResponse {
    pub raffle_id: u64,
    pub raffle_state: RaffleState,
    pub raffle_info: Option<RaffleInfo>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct AllRafflesResponse {
    pub raffles: Vec<RaffleResponse>,
}
