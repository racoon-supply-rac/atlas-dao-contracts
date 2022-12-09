
#[cfg(not(feature = "library"))]
use anyhow::{anyhow, Result};
use cosmwasm_std::{entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response};

use serde::Serialize;
use utils::state::OwnerStruct;

use nft_loans_export::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use nft_loans_export::state::ContractInfo;

use crate::admin::{set_fee_distributor, set_fee_rate, set_owner};
use crate::admin::claim_ownership;
use crate::execute::accept_loan;
use crate::execute::accept_offer;
use crate::execute::cancel_offer;
use crate::execute::deposit_collaterals;
use crate::execute::make_offer;
use crate::execute::modify_collaterals;
use crate::execute::refuse_offer;
use crate::execute::repay_borrowed_funds;
use crate::execute::withdraw_collateral;
use crate::execute::withdraw_defaulted_loan;
use crate::execute::withdraw_refused_offer;

use crate::query::{
    query_all_collaterals, query_borrower_info, query_collateral_info, query_collaterals,
    query_contract_info, query_lender_offers, query_offer_info, query_offers,
};
use crate::state::CONTRACT_INFO;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response> {
    // Verify the contract name and the sent fee rates
    msg.validate()?;
    // store token info
    let data = ContractInfo {
        name: msg.name,
        owner: OwnerStruct::new(
            deps
                .api
                .addr_validate(&msg.owner.unwrap_or_else(|| info.sender.to_string()))?),
        fee_distributor: deps.api.addr_validate(&msg.fee_distributor)?,
        fee_rate: msg.fee_rate,
        global_offer_index: 0,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default()
        .add_attribute("action", "initialization")
        .add_attribute("contract", "p2p-loans"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        ExecuteMsg::DepositCollaterals {
            tokens,
            terms,
            comment,
        } => deposit_collaterals(deps, env, info, tokens, terms, comment),
        ExecuteMsg::ModifyCollaterals {
            loan_id,
            terms,
            comment,
        } => modify_collaterals(deps, env, info, loan_id, terms, comment),
        ExecuteMsg::WithdrawCollaterals { loan_id } => {
            withdraw_collateral(deps, env, info, loan_id)
        }

        ExecuteMsg::AcceptLoan {
            borrower,
            loan_id,
            comment,
        } => accept_loan(deps, env, info, borrower, loan_id, comment),

        ExecuteMsg::AcceptOffer { global_offer_id } => {
            accept_offer(deps, env, info, global_offer_id)
        }
        ExecuteMsg::MakeOffer {
            borrower,
            loan_id,
            terms,
            comment,
        } => make_offer(deps, env, info, borrower, loan_id, terms, comment),

        ExecuteMsg::CancelOffer { global_offer_id } => {
            cancel_offer(deps, env, info, global_offer_id)
        }

        ExecuteMsg::RefuseOffer { global_offer_id } => {
            refuse_offer(deps, env, info, global_offer_id)
        }

        ExecuteMsg::WithdrawRefusedOffer { global_offer_id } => {
            withdraw_refused_offer(deps, env, info, global_offer_id)
        }

        ExecuteMsg::RepayBorrowedFunds { loan_id } => {
            repay_borrowed_funds(deps, env, info, loan_id)
        }
        ExecuteMsg::WithdrawDefaultedLoan { borrower, loan_id } => {
            withdraw_defaulted_loan(deps, env, info, borrower, loan_id)
        }

        // Internal Contract Logic
        ExecuteMsg::SetOwner { owner } => set_owner(deps, env, info, owner),
        ExecuteMsg::ClaimOwnership { } => claim_ownership(deps, env, info),

        ExecuteMsg::SetFeeDistributor { fee_depositor } => {
            set_fee_distributor(deps, env, info, fee_depositor)
        }

        ExecuteMsg::SetFeeRate { fee_rate } => set_fee_rate(deps, env, info, fee_rate),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response> {
    // No state migrations performed, just returned a Response
    Ok(Response::default())
}

fn to_anyhow_binary<T: Serialize>(message: &T) -> Result<Binary> {
    to_binary(message).map_err(|err| anyhow!(err))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary> {
    match msg {
        QueryMsg::ContractInfo {} => to_anyhow_binary(&query_contract_info(deps)?),
        QueryMsg::BorrowerInfo { borrower } => {
            to_anyhow_binary(&query_borrower_info(deps, borrower)?)
        }
        QueryMsg::CollateralInfo { borrower, loan_id } => {
            to_anyhow_binary(&query_collateral_info(deps, borrower, loan_id)?)
        }

        QueryMsg::Collaterals {
            borrower,
            start_after,
            limit,
        } => to_anyhow_binary(&query_collaterals(deps, borrower, start_after, limit)?),

        QueryMsg::AllCollaterals { start_after, limit } => {
            to_anyhow_binary(&query_all_collaterals(deps, start_after, limit)?)
        }

        QueryMsg::OfferInfo { global_offer_id } => {
            to_anyhow_binary(&query_offer_info(deps, global_offer_id)?)
        }

        QueryMsg::Offers {
            borrower,
            loan_id,
            start_after,
            limit,
        } => to_anyhow_binary(&query_offers(deps, borrower, loan_id, start_after, limit)?),

        QueryMsg::LenderOffers {
            lender,
            start_after,
            limit,
        } => to_anyhow_binary(&query_lender_offers(deps, lender, start_after, limit)?),
    }
}
