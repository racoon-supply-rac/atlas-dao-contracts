use std::convert::TryInto;
use strum_macros;
use cosmwasm_std::{coin, Addr, Binary, Coin, Env, Timestamp, Uint128, Decimal, StdError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utils::state::OwnerStruct;

/*
pub const MINIMUM_RAFFLE_DURATION: u64 = 3600; // A raffle last at least 1 hour
pub const MINIMUM_RAFFLE_TIMEOUT: u64 = 120; // The raffle duration is a least 2 minutes
pub const MINIMUM_RAND_FEE: u128 = 1; // The randomness provider gets at least 1/10_000 of the total raffle price
pub const MAXIMUM_PARTICIPANT_NUMBER: u64 = 1000;
*/

pub const MINIMUM_RAFFLE_DURATION: u64 = 1;
pub const MINIMUM_RAFFLE_TIMEOUT: u64 = 120; // The raffle timeout is a least 2 minutes
pub const DECIMAL_FRACTIONAL: u128 = 1_000_000_000_000_000_000u128; // 1*10**18
pub const MINIMUM_RAND_FEE: Decimal = Decimal::raw(DECIMAL_FRACTIONAL/10_000u128); // The randomness provider gets at least 1/10_000 of the total raffle price

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Cw1155Coin {
    pub address: String,
    pub token_id: String,
    pub value: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Cw721Coin {
    pub address: String,
    pub token_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Cw20Coin {
    pub address: String,
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Sg721Token {
    pub address: String,
    pub amount: Uint128,
    // TODO: from -  https://docs.rs/sg721-base/latest/sg721_base/struct.Sg721Contract.html
    // pub parent: Cw721Contract<'a, T, StargazeMsgWrapper, Empty, Empty>,
    // pub collection_info: Item<'a, CollectionInfo<RoyaltyInfo>>,
    // pub frozen_collection_info: Item<'a, bool>,
    // pub royalty_updated_at: Item<'a, Timestamp>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo<> {
    Cw20Coin(Cw20Coin),
    Cw721Coin(Cw721Coin),
    Cw1155Coin(Cw1155Coin),
    Sg721Token(Sg721Token),
    Coin(Coin),
}

impl AssetInfo {
    pub fn coin(amount: u128, denom: &str) -> Self {
        AssetInfo::Coin(coin(amount, denom))
    }

    pub fn coin_raw(amount: Uint128, denom: &str) -> Self {
        AssetInfo::Coin(Coin {
            denom: denom.to_string(),
            amount,
        })
    }

    pub fn cw20(amount: u128, address: &str) -> Self {
        AssetInfo::cw20_raw(Uint128::from(amount), address)
    }

    pub fn cw20_raw(amount: Uint128, address: &str) -> Self {
        AssetInfo::Cw20Coin(Cw20Coin {
            address: address.to_string(),
            amount,
        })
    }

    pub fn cw721(address: &str, token_id: &str) -> Self {
        AssetInfo::Cw721Coin(Cw721Coin {
            address: address.to_string(),
            token_id: token_id.to_string(),
        })
    }

    pub fn cw1155(address: &str, token_id: &str, value: u128) -> Self {
        AssetInfo::cw1155_raw(address, token_id, Uint128::from(value))
    }

    pub fn cw1155_raw(address: &str, token_id: &str, value: Uint128) -> Self {
        AssetInfo::Cw1155Coin(Cw1155Coin {
            address: address.to_string(),
            token_id: token_id.to_string(),
            value,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, strum_macros::Display)]
#[serde(rename_all = "snake_case")]
pub enum RaffleState {
    Created,
    Started,
    Closed,
    Finished,
    Claimed,
    Cancelled,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ContractInfo {
    pub name: String,
    pub owner: OwnerStruct,
    pub fee_addr: Addr,
    pub last_raffle_id: Option<u64>,
    pub minimum_raffle_duration: u64, // The minimum interval in which users can buy raffle tickets
    pub minimum_raffle_timeout: u64, // The minimum interval during which users can provide entropy to the contract
    pub raffle_fee: Decimal, // The percentage of the resulting ticket-tokens that will go to the treasury
    pub rand_fee: Decimal, // The percentage of the resulting ticket-tokens that will go to the entropy provider
    pub lock: bool,        // Wether the contract can accept new raffles
    pub drand_url: String, // The drand provider url (to find the right entropy provider)
    pub verify_signature_contract: Addr, // The contract that can verify the entropy signature
    pub random_pubkey: Binary, // The public key of the randomness provider, to verify entropy origin
}


impl ContractInfo{
    pub fn validate_fee(&self) -> Result<(), StdError>{
        // Check the fee distribution
        if self.raffle_fee + self.rand_fee >= Decimal::one(){
            return Err(StdError::generic_err(
                "The Total Fee rate should be lower than 1"
            ))
        }
        Ok(())
    }
}


#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct RaffleOptions {
    pub raffle_start_timestamp: Timestamp, // If not specified, starts immediately
    pub raffle_duration: u64,
    pub raffle_timeout: u64,
    pub comment: Option<String>,
    pub max_participant_number: Option<u32>,
    pub max_ticket_per_address: Option<u32>,
    pub raffle_preview: u32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct RaffleOptionsMsg {
    pub raffle_start_timestamp: Option<Timestamp>, // If not specified, starts immediately
    pub raffle_duration: Option<u64>,
    pub raffle_timeout: Option<u64>,
    pub comment: Option<String>,
    pub max_participant_number: Option<u32>,
    pub max_ticket_per_address: Option<u32>,
    pub raffle_preview: Option<u32>,
}

impl RaffleOptions {
    pub fn new(
        env: Env,
        assets_len: usize,
        raffle_options: RaffleOptionsMsg,
        contract_info: ContractInfo,
    ) -> Self {
        Self {
            raffle_start_timestamp: raffle_options
                .raffle_start_timestamp
                .unwrap_or(env.block.time)
                .max(env.block.time),
            raffle_duration: raffle_options
                .raffle_duration
                .unwrap_or(contract_info.minimum_raffle_duration)
                .max(contract_info.minimum_raffle_duration),
            raffle_timeout: raffle_options
                .raffle_timeout
                .unwrap_or(contract_info.minimum_raffle_timeout)
                .max(contract_info.minimum_raffle_timeout),
            comment: raffle_options.comment,
            max_participant_number: raffle_options.max_participant_number,
            max_ticket_per_address: raffle_options.max_ticket_per_address,
            raffle_preview: raffle_options
                .raffle_preview
                .map(|preview| {
                    if preview >= assets_len.try_into().unwrap() {
                        0u32
                    } else {
                        preview
                    }
                })
                .unwrap_or(0u32),
        }
    }

    pub fn new_from(
        current_options: RaffleOptions,
        assets_len: usize,
        raffle_options: RaffleOptionsMsg,
        contract_info: ContractInfo,
    ) -> Self {
        Self {
            raffle_start_timestamp: raffle_options
                .raffle_start_timestamp
                .unwrap_or(current_options.raffle_start_timestamp)
                .max(current_options.raffle_start_timestamp),
            raffle_duration: raffle_options
                .raffle_duration
                .unwrap_or(current_options.raffle_duration)
                .max(contract_info.minimum_raffle_duration),
            raffle_timeout: raffle_options
                .raffle_timeout
                .unwrap_or(current_options.raffle_timeout)
                .max(contract_info.minimum_raffle_timeout),
            comment: raffle_options.comment.or(current_options.comment),
            max_participant_number: raffle_options
                .max_participant_number
                .or(current_options.max_participant_number),
            max_ticket_per_address: raffle_options
                .max_ticket_per_address
                .or(current_options.max_ticket_per_address),
            raffle_preview: raffle_options
                .raffle_preview
                .map(|preview| {
                    if preview >= assets_len.try_into().unwrap() {
                        0u32
                    } else {
                        preview
                    }
                })
                .unwrap_or(current_options.raffle_preview),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Randomness {
    pub randomness: [u8; 32],
    pub randomness_round: u64,
    pub randomness_owner: Addr,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct RaffleInfo {
    pub owner: Addr,
    pub assets: Vec<AssetInfo>,
    pub raffle_ticket_price: AssetInfo,
    pub number_of_tickets: u32,
    pub randomness: Option<Randomness>,
    pub winner: Option<Addr>,
    pub is_cancelled: bool,
    pub raffle_options: RaffleOptions,
}

