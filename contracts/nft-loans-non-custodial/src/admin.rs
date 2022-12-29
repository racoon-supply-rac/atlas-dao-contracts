use crate::state::{is_owner, CONTRACT_INFO};
use anyhow::Result;
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, Uint128};

/// Owner only function
/// Sets a new owner
/// The owner can set the parameters of the contract
/// * Owner
/// * Fee distributor contract
/// * Fee Rate
pub fn set_owner(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_owner: String,
) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    let new_owner = deps.api.addr_validate(&new_owner)?;
    contract_info.owner = new_owner.clone();
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::default()
        .add_attribute("action", "changed-contract-parameter")
        .add_attribute("parameter", "owner")
        .add_attribute("value", new_owner))
}

/// Owner only function
/// Sets a new fee-distributor contract
/// This contract distributes fees back to the projects (and Illiquidly DAO gets to keep a small amount too)
pub fn set_fee_distributor(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_distributor: String,
) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.fee_distributor = new_distributor.clone();
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::default()
        .add_attribute("action", "changed-contract-parameter")
        .add_attribute("parameter", "fee_distributor")
        .add_attribute("value", new_distributor))
}

/// Owner only function
/// Sets a new fee rate
/// fee_rate is in units of a 1/100_000th, so e.g. if fee_rate=5_000, the fee_rate is 5%
/// It correspond to the part of interests that are kept by the organisation (for redistribution and DAO purposes)
pub fn set_fee_rate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_fee_rate: Uint128,
) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.fee_rate = new_fee_rate;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "changed-contract-parameter")
        .add_attribute("parameter", "fee_rate")
        .add_attribute("value", new_fee_rate))
}
