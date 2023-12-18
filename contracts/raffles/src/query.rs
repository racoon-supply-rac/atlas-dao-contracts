
use cosmwasm_std::StdError;
#[cfg(not(feature = "library"))]
use cosmwasm_std::{Api, Deps, Env, Order, StdResult, WasmQuery};
use cosmwasm_std::QueryRequest;
use cosmwasm_std::to_json_binary;
use cosmwasm_std::Addr;

use raffles_export::msg::AllRafflesResponse;
use raffles_export::msg::RaffleResponse;

use cw_storage_plus::Bound;

use cw721::{Cw721QueryMsg, OwnerOfResponse};
use crate::error::ContractError;
use crate::state::{get_raffle_state, load_raffle, CONTRACT_INFO, RAFFLE_INFO, USER_TICKETS, RAFFLE_TICKETS};
use raffles_export::msg::QueryFilters;
use raffles_export::state::{AssetInfo, ContractInfo, RaffleInfo, RaffleState};

// settings for pagination
const MAX_LIMIT: u32 = 100;
const DEFAULT_LIMIT: u32 = 10;
const BASE_LIMIT: usize = 100;

pub fn query_contract_info(deps: Deps) -> StdResult<ContractInfo> {
    CONTRACT_INFO.load(deps.storage)
}

// parse raffles to human readable format
fn parse_raffles(
    _: &dyn Api,
    env: Env,
    item: StdResult<(u64, RaffleInfo)>,
) -> StdResult<RaffleResponse> {
    item.map(|(raffle_id, raffle)| RaffleResponse {
        raffle_id,
        raffle_state: get_raffle_state(env, raffle.clone()),
        raffle_info: Some(raffle),
    })
}
/// Filters the raffles results according to any of the indicated filters
/// State : only select raffles that are in a special state (multiple states possible)
/// Owner: only select raffles that are owned by a specific address
/// Contains_token : only selevty raffles that have a specific asset in them
pub fn raffle_filter(
    _api: &dyn Api,
    env: Env,
    raffle_info: &StdResult<RaffleResponse>,
    filters: &Option<QueryFilters>,
) -> bool {
    if let Some(filters) = filters {
        let raffle = raffle_info.as_ref().unwrap();

        (match &filters.states {
            Some(state) => state
                .contains(&get_raffle_state(env, raffle.raffle_info.clone().unwrap()).to_string()),
            None => true,
        } && match &filters.owner {
            Some(owner) => raffle.raffle_info.as_ref().unwrap().owner == owner.clone(),
            None => true,
        } && match &filters.contains_token {
            Some(token) => {
                raffle
                    .raffle_info
                    .clone()
                    .unwrap()
                    .assets
                    .iter()
                    .any(|asset| match asset {
                        AssetInfo::Coin(x) => x.denom == token.as_ref(),
                        AssetInfo::Cw721Coin(x) => x.address == token.as_ref(),
                        // AssetInfo::Cw20Coin(x) => x.address == token.as_ref(),
                        // AssetInfo::Cw1155Coin(x) => x.address == token.as_ref(),
                    })
            }
            None => true,
        })
    } else {
        true
    }
}

/// Query the number of tickets a ticket_depositor bought in a specific raffle, designated by a raffle_id
pub fn query_ticket_number(
    deps: Deps,
    _env: Env,
    raffle_id: u64,
    ticket_depositor: String,
) -> StdResult<u32> {
    Ok(USER_TICKETS.load(
        deps.storage,
        (&deps.api.addr_validate(&ticket_depositor)?, raffle_id),
    )?)
}

/// Query all raffles using ALL filters
pub fn query_all_raffles(
    deps: Deps,
    env: Env,
    start_after: Option<u64>,
    limit: Option<u32>,
    filters: Option<QueryFilters>,
) -> StdResult<AllRafflesResponse> {
    if filters.is_some() && filters.clone().unwrap().ticket_depositor.is_some() {
        query_all_raffles_by_depositor(deps, env, start_after, limit, filters)
    } else {
        query_all_raffles_raw(deps, env, start_after, limit, filters)
        .map_err(|err| StdError::from(err))
    }
}

/// Query all raffles in which a depositor bought a ticket
/// Returns an empty raffle_info if none where found in the BASE_LIMIT first results
pub fn query_all_raffles_by_depositor(
    deps: Deps,
    env: Env,
    start_after: Option<u64>,
    limit: Option<u32>,
    filters: Option<QueryFilters>,
) -> StdResult<AllRafflesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let start = start_after.map(Bound::exclusive);

    let ticket_depositor = deps.api.addr_validate(
        &filters
    .clone()
    .ok_or_else(|| StdError::generic_err("unauthorized"))?
    .ticket_depositor
    .ok_or_else(|| StdError::generic_err("unauthorized"))?
    )?;

    let mut raffles = USER_TICKETS
        .prefix(&ticket_depositor)
        .keys(deps.storage, None, start.clone(), Order::Descending)
        .take(BASE_LIMIT)
        .filter_map(|response| response.ok())
        .map(|raffle_id| Ok((raffle_id, load_raffle(deps.storage, raffle_id).unwrap()))) // This unwrap is safe if the data structure was respected
        .map(|kv_item| parse_raffles(deps.api, env.clone(), kv_item))
        .filter(|response| raffle_filter(deps.api, env.clone(), response, &filters))
        .take(limit)
        .collect::<StdResult<Vec<RaffleResponse>>>()?;

    if raffles.is_empty() {
        let last_raffle_id = USER_TICKETS
            .prefix(&ticket_depositor)
            .keys(deps.storage, None, start.clone(), Order::Descending)
            .take(BASE_LIMIT)
            .filter_map(|response| response.ok())
            .last();
        if let Some(raffle_id) = last_raffle_id {
            if raffle_id != 0 {
                raffles = vec![RaffleResponse {
                    raffle_id,
                    raffle_state: RaffleState::Claimed,
                    raffle_info: None,
                }]
            }
        }
    }

    Ok(AllRafflesResponse { raffles })
}

/// Query all raffles without ticket_depositor filters
/// Returns an empty raffle_info if none where found in the BASE_LIMIT first results
pub fn query_all_raffles_raw(
    deps: Deps,
    env: Env,
    start_after: Option<u64>,
    limit: Option<u32>,
    filters: Option<QueryFilters>,
) -> StdResult<AllRafflesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let mut raffles: Vec<RaffleResponse> = RAFFLE_INFO
        .range(deps.storage, None, start.clone(), Order::Descending)
        .take(BASE_LIMIT)
        .map(|kv_item| parse_raffles(deps.api, env.clone(), kv_item))
        .filter(|response| raffle_filter(deps.api, env.clone(), response, &filters))
        .take(limit)
        .collect::<StdResult<Vec<RaffleResponse>>>()?;

    if raffles.is_empty() {
        let raffle_id = RAFFLE_INFO
            .keys(deps.storage, None, start, Order::Descending)
            .take(BASE_LIMIT)
            .last();

        if let Some(Ok(raffle_id)) = raffle_id {
            if raffle_id != 0 {
                raffles = vec![RaffleResponse {
                    raffle_id,
                    raffle_state: RaffleState::Claimed,
                    raffle_info: None,
                }]
            }
        }
    }
    Ok(AllRafflesResponse { raffles })
}

/// Query all ticket onwers within a raffle
///
pub fn query_all_tickets(
    deps: Deps,
    _env: Env,
    raffle_id: u64,
    start_after: Option<u32>,
    limit: Option<u32>,
) -> StdResult<Vec<String>> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    RAFFLE_TICKETS
        .prefix(raffle_id)
        .range(deps.storage, start.clone(), None, Order::Ascending)
        .map(|kv_item| Ok(kv_item?.1.to_string()))
        .take(limit)
        .collect()
}

pub fn is_nft_owner(deps: Deps, sender: Addr, nft_address: String, token_id: String) -> Result<(), ContractError>{

    let owner_response: OwnerOfResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: nft_address,
            msg: to_json_binary(&Cw721QueryMsg::OwnerOf { token_id, include_expired: None })?,
        }))?;

    if owner_response.owner != sender{
        return Err(ContractError::SenderNotOwner{})
    }
    Ok(())
}
