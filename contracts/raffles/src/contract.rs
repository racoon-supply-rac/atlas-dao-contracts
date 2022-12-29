use anyhow::{anyhow, bail, Result};
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, Event, MessageInfo, Reply, Response, StdError,
    SubMsgResult, Uint128,
};
#[cfg(not(feature = "library"))]
use std::convert::TryInto;

use cw2::set_contract_version;

use crate::error::ContractError;

use crate::state::{get_raffle_state, is_owner, load_raffle, CONTRACT_INFO, RAFFLE_INFO};
use raffles_export::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, RaffleResponse};
use raffles_export::state::{
    ContractInfo, Randomness, MINIMUM_RAFFLE_DURATION, MINIMUM_RAFFLE_TIMEOUT, MINIMUM_RAND_FEE,
};

use crate::execute::{
    execute_buy_tickets, execute_cancel_raffle, execute_claim, execute_create_raffle,
    execute_modify_raffle, execute_receive, execute_update_randomness,
};
use crate::query::{
    query_all_raffles, query_all_tickets, query_contract_info, query_ticket_number,
};

const CONTRACT_NAME: &str = "illiquidlabs.io:raffles";
const CONTRACT_VERSION: &str = "0.1.0";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Verify the contract name
    msg.validate()?;
    // store token info
    let data = ContractInfo {
        name: msg.name,
        owner: deps
            .api
            .addr_validate(&msg.owner.unwrap_or_else(|| info.sender.to_string()))?,
        fee_addr: deps
            .api
            .addr_validate(&msg.fee_addr.unwrap_or_else(|| info.sender.to_string()))?,
        last_raffle_id: None,
        minimum_raffle_duration: msg
            .minimum_raffle_duration
            .unwrap_or(MINIMUM_RAFFLE_DURATION)
            .max(MINIMUM_RAFFLE_DURATION),
        minimum_raffle_timeout: msg
            .minimum_raffle_timeout
            .unwrap_or(MINIMUM_RAFFLE_TIMEOUT)
            .max(MINIMUM_RAFFLE_TIMEOUT),
        raffle_fee: msg.raffle_fee.unwrap_or(Uint128::zero()),
        rand_fee: msg
            .rand_fee
            .unwrap_or_else(|| Uint128::from(MINIMUM_RAND_FEE)),
        lock: false,
        drand_url: msg
            .drand_url
            .unwrap_or_else(|| "https://api.drand.sh/".to_string()),
        random_pubkey: msg.random_pubkey,
        verify_signature_contract: deps.api.addr_validate(&msg.verify_signature_contract)?,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default()
        .add_attribute("action", "init")
        .add_attribute("contract", "raffle")
        .add_attribute("owner", data.owner))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        ExecuteMsg::CreateRaffle {
            owner,
            assets,
            raffle_ticket_price,
            raffle_options,
        } => execute_create_raffle(
            deps,
            env,
            info,
            owner,
            assets,
            raffle_ticket_price,
            raffle_options,
        ),
        ExecuteMsg::CancelRaffle { raffle_id } => execute_cancel_raffle(deps, env, info, raffle_id),
        ExecuteMsg::ModifyRaffle {
            raffle_id,
            raffle_ticket_price,
            raffle_options,
        } => execute_modify_raffle(
            deps,
            env,
            info,
            raffle_id,
            raffle_ticket_price,
            raffle_options,
        ),
        ExecuteMsg::BuyTicket {
            raffle_id,
            ticket_number,
            sent_assets,
        } => execute_buy_tickets(deps, env, info, raffle_id, ticket_number, sent_assets),
        ExecuteMsg::Receive {
            sender,
            amount,
            msg,
        } => execute_receive(deps, env, info, sender, amount, msg),
        ExecuteMsg::ClaimNft { raffle_id } => execute_claim(deps, env, info, raffle_id),
        ExecuteMsg::UpdateRandomness {
            raffle_id,
            randomness,
        } => execute_update_randomness(deps, env, info, raffle_id, randomness),

        // Admin messages
        ExecuteMsg::ToggleLock { lock } => execute_toggle_lock(deps, env, info, lock),
        ExecuteMsg::Renounce {} => execute_renounce(deps, env, info),
        ExecuteMsg::ChangeParameter { parameter, value } => {
            execute_change_parameter(deps, env, info, parameter, value)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // No state migrations performed, just returned a Response
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary> {
    match msg {
        QueryMsg::ContractInfo {} => to_binary(&query_contract_info(deps)?).map_err(|x| anyhow!(x)),
        QueryMsg::RaffleInfo { raffle_id } => {
            let raffle_info = load_raffle(deps.storage, raffle_id)?;
            to_binary(&RaffleResponse {
                raffle_id,
                raffle_state: get_raffle_state(env, raffle_info.clone()),
                raffle_info: Some(raffle_info),
            })
            .map_err(|x| anyhow!(x))
        }

        QueryMsg::AllRaffles {
            start_after,
            limit,
            filters,
        } => to_binary(&query_all_raffles(deps, env, start_after, limit, filters)?)
            .map_err(|x| anyhow!(x)),

        QueryMsg::AllTickets {
            raffle_id,
            start_after,
            limit,
        } => to_binary(&query_all_tickets(
            deps,
            env,
            raffle_id,
            start_after,
            limit,
        )?)
        .map_err(|x| anyhow!(x)),

        QueryMsg::TicketNumber { owner, raffle_id } => {
            to_binary(&query_ticket_number(deps, env, raffle_id, owner)?).map_err(|x| anyhow!(x))
        }
    }
}

/// Replace the current contract owner with the provided owner address
/// * `owner` must be a valid Terra address
/// The owner has limited power on this contract :
/// 1. Change the contract owner
/// 2. Change the fee contract
/// 3. Change the default raffle parameters
pub fn execute_renounce(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.owner = env.contract.address;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "modify_parameter")
        .add_attribute("parameter", "owner")
        .add_attribute("value", contract_info.owner))
}

/// Locking the contract (lock=true) means preventing the creation of new raffles
/// Tickets can still be bought and NFTs retrieved when a contract is locked
pub fn execute_toggle_lock(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    lock: bool,
) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.lock = lock;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "modify_parameter")
        .add_attribute("parameter", "contract_lock")
        .add_attribute("value", lock.to_string()))
}

/// Change the different contract parameters
/// Admin only action
pub fn execute_change_parameter(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    parameter: String,
    value: String,
) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    match parameter.as_str() {
        "owner" => {
            let owner = deps.api.addr_validate(&value)?;
            contract_info.owner = owner;
        }
        "fee_addr" => {
            let addr = deps.api.addr_validate(&value)?;
            contract_info.fee_addr = addr;
        }
        "minimum_raffle_duration" => {
            let time = value.parse::<u64>()?;
            contract_info.minimum_raffle_duration = time;
        }
        "minimum_raffle_timeout" => {
            let time = value.parse::<u64>()?;
            contract_info.minimum_raffle_timeout = time;
        }
        "raffle_fee" => {
            let fee = Uint128::from(value.parse::<u128>()?);
            contract_info.raffle_fee = fee;
        }
        "rand_fee" => {
            let fee = Uint128::from(value.parse::<u128>()?);
            contract_info.rand_fee = fee;
        }
        "drand_url" => {
            contract_info.drand_url = value.clone();
        }
        "verify_signature_contract" => {
            let addr = deps.api.addr_validate(&value)?;
            contract_info.verify_signature_contract = addr;
        }
        "random_pubkey" => {
            contract_info.random_pubkey = Binary::from_base64(&value).unwrap();
        }
        _ => return Err(anyhow!(ContractError::ParameterNotFound {})),
    }

    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "modify_parameter")
        .add_attribute("parameter", parameter)
        .add_attribute("value", value))
}

/// Messages triggered after random validation.
/// We wrap the random validation in a message to make sure the transaction goes through.
/// This may require too much gas for query
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response> {
    match msg.id {
        0 => Ok(verify(deps, env, msg.result)?),
        _ => bail!(ContractError::Unauthorized {}),
    }
}

/// This function is called after the randomness verifier has verified the current randomness
/// We used this architecture to make sure the verification passes (because a query may return early)
/// We verify the randomness provided matches the current state of the contract (good round, good raffle_id...)
/// We also save the new randomness in the contract
pub fn verify(deps: DepsMut, _env: Env, msg: SubMsgResult) -> Result<Response> {
    match msg {
        SubMsgResult::Ok(subcall) => {
            let event: Event = subcall
                .events
                .into_iter()
                .find(|e| e.ty == "wasm")
                .ok_or_else(|| ContractError::NotFoundError("wasm results".to_string()))?;

            let round = event
                .attributes
                .clone()
                .into_iter()
                .find(|attr| attr.key == "round")
                .map_or(
                    Err(ContractError::NotFoundError("randomness round".to_string())),
                    |round| {
                        round
                            .value
                            .parse::<u64>()
                            .map_err(|_| ContractError::ParseError("randomness round".to_string()))
                    },
                )?;

            let randomness: String = event
                .attributes
                .clone()
                .into_iter()
                .find(|attr| attr.key == "randomness")
                .map(|rand| rand.value)
                .ok_or_else(|| ContractError::NotFoundError("randomness value".to_string()))?;

            let raffle_id: u64 = event
                .attributes
                .clone()
                .into_iter()
                .find(|attr| attr.key == "raffle_id")
                .map_or(
                    Err(ContractError::NotFoundError("raffle_id".to_string())),
                    |raffle_id| {
                        raffle_id
                            .value
                            .parse::<u64>()
                            .map_err(|_| ContractError::ParseError("raffle_id".to_string()))
                    },
                )?;

            let owner = deps.api.addr_validate(
                &event
                    .attributes
                    .into_iter()
                    .find(|attr| attr.key == "owner")
                    .map(|owner| owner.value)
                    .ok_or_else(|| ContractError::NotFoundError("raffle owner".to_string()))?,
            )?;

            let mut raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;
            raffle_info.randomness = Some(Randomness {
                randomness: Binary::from_base64(&randomness)?
                    .as_slice()
                    .try_into()
                    .map_err(|_| ContractError::ParseError("randomness".to_string()))?,
                randomness_round: round,
                randomness_owner: owner.clone(),
            });

            RAFFLE_INFO.save(deps.storage, raffle_id, &raffle_info)?;

            Ok(Response::new()
                .add_attribute("action", "update_randomness")
                .add_attribute("raffle_id", raffle_id.to_string())
                .add_attribute("sender", owner))
        }
        SubMsgResult::Err(_) => bail!(StdError::generic_err("err")),
    }
}
