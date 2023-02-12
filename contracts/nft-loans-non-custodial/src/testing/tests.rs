use std::str::FromStr;
use crate::contract::execute;
use crate::contract::instantiate;
use crate::error::ContractError;
use crate::state::lender_offers;
use crate::state::COLLATERAL_INFO;
use crate::state::CONTRACT_INFO;
use anyhow::Result;
use cosmwasm_std::{
    coin, coins,
    testing::{mock_dependencies, mock_env, mock_info},
    Api, BankMsg, Coin, DepsMut, Env, Response, SubMsg, Uint128, Decimal
};
use cw1155::Cw1155ExecuteMsg;

use utils::state::OwnerStruct;

use fee_contract_export::state::FeeType;
use fee_distributor_export::msg::ExecuteMsg as FeeDistributorMsg;
use nft_loans_export::msg::ExecuteMsg;
use nft_loans_export::msg::InstantiateMsg;
use nft_loans_export::state::CollateralInfo;
use nft_loans_export::state::ContractInfo;
use nft_loans_export::state::LoanState;
use nft_loans_export::state::LoanTerms;
use nft_loans_export::state::OfferState;
use utils::msg::into_cosmos_msg;
use utils::state::AssetInfo;
use utils::state::Cw1155Coin;
use utils::state::Cw721Coin;
use crate::testing::mock_querier::{mock_dependencies as mock_querier_dependencies};

pub fn assert_error(err: anyhow::Error, contract_error: ContractError) {
    assert_eq!(err.downcast::<ContractError>().unwrap(), contract_error)
}

pub fn init_helper(deps: DepsMut) {
    let instantiate_msg = InstantiateMsg {
        name: "nft-loan".to_string(),
        owner: None,
        fee_distributor: "fee_distributor".to_string(),
        fee_rate: Decimal::from_str("0.05").unwrap(),
    };
    let info = mock_info("creator", &[]);
    let env = mock_env();

    instantiate(deps, env, info, instantiate_msg).unwrap();
}

#[test]
fn test_init_sanity() {
    let mut deps = mock_dependencies();
    let instantiate_msg = InstantiateMsg {
        name: "p2p-trading".to_string(),
        owner: Some("this_address".to_string()),
        fee_distributor: "fee_distributor".to_string(),
        fee_rate: Decimal::from_str("0.05").unwrap(),
    };
    let info = mock_info("owner", &[]);
    let env = mock_env();

    let res_init = instantiate(deps.as_mut(), env.clone(), info, instantiate_msg).unwrap();
    assert_eq!(0, res_init.messages.len());

    let contract = CONTRACT_INFO.load(&deps.storage).unwrap();
    assert_eq!(
        contract,
        ContractInfo {
            name: "p2p-trading".to_string(),
            owner: OwnerStruct::new(deps.api.addr_validate("this_address").unwrap()),
            fee_distributor: deps.api.addr_validate("fee_distributor").unwrap(),
            fee_rate: Decimal::from_str("0.05").unwrap(),
            global_offer_index: 0
        }
    );

    let info = mock_info("this_address", &[]);
    let bad_info = mock_info("bad_person", &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        ExecuteMsg::SetFeeDistributor {
            fee_depositor: "new_fee_distributor".to_string(),
        },
    )
    .unwrap();
    assert_eq!(
        CONTRACT_INFO.load(&deps.storage).unwrap().fee_distributor,
        "new_fee_distributor".to_string()
    );

    let unauthorized = execute(
        deps.as_mut(),
        env.clone(),
        bad_info.clone(),
        ExecuteMsg::SetFeeDistributor {
            fee_depositor: "new_fee_distributor".to_string(),
        },
    )
    .unwrap_err();
    assert_error(unauthorized, ContractError::Unauthorized {});

    // We test changing the owner
    let unauthorized = execute(
        deps.as_mut(),
        env.clone(),
        bad_info.clone(),
        ExecuteMsg::SetOwner {
            owner: "new_owner".to_string(),
        },
    )
    .unwrap_err();
    assert_error(unauthorized, ContractError::Unauthorized {});

    // We test changing the owner
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetOwner {
            owner: "new_owner".to_string(),
        },
    )
    .unwrap();
    assert_eq!(
        CONTRACT_INFO.load(&deps.storage).unwrap().owner,
        OwnerStruct{
            owner: deps.api.addr_validate("this_address").unwrap(),
            new_owner: Some(deps.api.addr_validate("new_owner").unwrap()),
        }
    );
    // Now the new address claims the contract
    execute(
        deps.as_mut(),
        env.clone(),
        mock_info("new_owner", &[]),
        ExecuteMsg::ClaimOwnership {  }
    )
    .unwrap();
    assert_eq!(
        CONTRACT_INFO.load(&deps.storage).unwrap().owner,
        OwnerStruct{
            owner: deps.api.addr_validate("new_owner").unwrap(),
            new_owner: None
        }
    );

    let info = mock_info("new_owner", &[]);

    let unauthorized = execute(
        deps.as_mut(),
        env.clone(),
        bad_info,
        ExecuteMsg::SetFeeRate {
            fee_rate: Decimal::from_str("0.005").unwrap(),
        },
    )
    .unwrap_err();
    assert_error(unauthorized, ContractError::Unauthorized {});

    execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::SetFeeRate {
            fee_rate: Decimal::from_str("0.005").unwrap(),
        },
    )
    .unwrap();
    assert_eq!(
        CONTRACT_INFO.load(&deps.storage).unwrap().fee_rate,
        Decimal::from_str("0.005").unwrap(),
    );
}

pub fn add_collateral_helper(
    deps: DepsMut,
    creator: &str,
    address: &str,
    token_id: &str,
    value: Option<Uint128>,
    terms: Option<LoanTerms>,
) -> Result<Response> {
    let info = mock_info(creator, &[]);
    let env = mock_env();

    execute(
        deps,
        env,
        info,
        ExecuteMsg::DepositCollaterals {
            tokens: vec![if let Some(value) = value {
                AssetInfo::Cw1155Coin(Cw1155Coin {
                    address: address.to_string(),
                    token_id: token_id.to_string(),
                    value,
                })
            } else {
                AssetInfo::Cw721Coin(Cw721Coin {
                    address: address.to_string(),
                    token_id: token_id.to_string(),
                })
            }],
            terms,
            comment: None,
            loan_preview: None,
        },
    )
}

fn set_terms_helper(
    deps: DepsMut,
    borrower: &str,
    loan_id: u64,
    terms: LoanTerms,
) -> Result<Response> {
    let info = mock_info(borrower, &[]);
    let env = mock_env();

    execute(
        deps,
        env,
        info,
        ExecuteMsg::ModifyCollaterals {
            loan_id,
            terms: Some(terms),
            comment: None,
            loan_preview: None
        },
    )
}

fn make_offer_helper(
    deps: DepsMut,
    lender: &str,
    borrower: &str,
    loan_id: u64,
    terms: LoanTerms,
    coins: Vec<Coin>,
) -> Result<Response> {
    let info = mock_info(lender, &coins);
    let env = mock_env();

    execute(
        deps,
        env,
        info,
        ExecuteMsg::MakeOffer {
            borrower: borrower.to_string(),
            loan_id,
            terms,
            comment: None,
        },
    )
}

fn cancel_offer_helper(deps: DepsMut, lender: &str, global_offer_id: &str) -> Result<Response> {
    let info = mock_info(lender, &[]);
    let env = mock_env();

    execute(
        deps,
        env,
        info,
        ExecuteMsg::CancelOffer {
            global_offer_id: global_offer_id.to_string(),
        },
    )
}

fn refuse_offer_helper(deps: DepsMut, borrower: &str, global_offer_id: &str) -> Result<Response> {
    let info = mock_info(borrower, &[]);
    let env = mock_env();

    execute(
        deps,
        env,
        info,
        ExecuteMsg::RefuseOffer {
            global_offer_id: global_offer_id.to_string(),
        },
    )
}

fn accept_loan_helper(
    deps: DepsMut,
    lender: &str,
    borrower: &str,
    loan_id: u64,
    coins: Vec<Coin>,
) -> Result<Response> {
    let info = mock_info(lender, &coins);
    let env = mock_env();

    execute(
        deps,
        env,
        info,
        ExecuteMsg::AcceptLoan {
            borrower: borrower.to_string(),
            loan_id,
            comment: None,
        },
    )
}

fn accept_offer_helper(deps: DepsMut, borrower: &str, global_offer_id: &str) -> Result<Response> {
    let info = mock_info(borrower, &[]);
    let env = mock_env();

    execute(
        deps,
        env,
        info,
        ExecuteMsg::AcceptOffer {
            global_offer_id: global_offer_id.to_string(),
        },
    )
}

fn withdraw_collateral_helper(deps: DepsMut, creator: &str, loan_id: u64) -> Result<Response> {
    let info = mock_info(creator, &[]);
    let env = mock_env();

    execute(deps, env, info, ExecuteMsg::WithdrawCollaterals { loan_id })
}

fn withdraw_refused_offer_helper(
    deps: DepsMut,
    lender: &str,
    global_offer_id: &str,
) -> Result<Response> {
    let info = mock_info(lender, &[]);
    let env = mock_env();

    execute(
        deps,
        env,
        info,
        ExecuteMsg::WithdrawRefusedOffer {
            global_offer_id: global_offer_id.to_string(),
        },
    )
}
fn repay_borrowed_funds_helper(
    deps: DepsMut,
    borrower: &str,
    loan_id: u64,
    funds: Vec<Coin>,
    env: Env,
) -> Result<Response> {
    let info = mock_info(borrower, &funds);

    execute(deps, env, info, ExecuteMsg::RepayBorrowedFunds { loan_id })
}
fn withdraw_defaulted_loan_helper(
    deps: DepsMut,
    lender: &str,
    borrower: &str,
    loan_id: u64,
    env: Env,
) -> Result<Response> {
    let info = mock_info(lender, &[]);

    execute(
        deps,
        env,
        info,
        ExecuteMsg::WithdrawDefaultedLoan {
            borrower: borrower.to_string(),
            loan_id,
        },
    )
}

#[test]
fn test_add_collateral() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());
    // We make sure the collateral is deposited correctly
    let res = add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
    assert_eq!(0, res.messages.len());

    // Other collaterals
    add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "59",
        Some(Uint128::from(459u128)),
        None,
    )
    .unwrap();

    let creator_addr = deps.api.addr_validate("creator").unwrap();
    let coll_info = COLLATERAL_INFO
        .load(&deps.storage, (creator_addr.clone(), 0))
        .unwrap();
    assert_eq!(
        coll_info,
        CollateralInfo {
            associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                address: "nft".to_string(),
                token_id: "58".to_string()
            })],
            list_date: mock_env().block.time,
            ..Default::default()
        }
    );

    let coll_info = COLLATERAL_INFO
        .load(&deps.storage, (creator_addr.clone(), 1))
        .unwrap();
    assert_eq!(
        coll_info,
        CollateralInfo {
            associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                address: "nft".to_string(),
                token_id: "59".to_string()
            })],
            list_date: mock_env().block.time,
            ..Default::default()
        }
    );

    let coll_info = COLLATERAL_INFO
        .load(&deps.storage, (creator_addr, 2))
        .unwrap();
    assert_eq!(
        coll_info,
        CollateralInfo {
            associated_assets: vec![AssetInfo::Cw1155Coin(Cw1155Coin {
                address: "nft".to_string(),
                token_id: "59".to_string(),
                value: Uint128::from(459u128)
            })],
            list_date: mock_env().block.time,
            ..Default::default()
        }
    );
}

#[test]
fn test_withdraw_collateral() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());
    add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "59",
        Some(Uint128::from(459u128)),
        None,
    )
    .unwrap();

    withdraw_collateral_helper(deps.as_mut(), "creator", 1).unwrap();
    withdraw_collateral_helper(deps.as_mut(), "creator", 0).unwrap();

    let creator_addr = deps.api.addr_validate("creator").unwrap();
    let coll_info = COLLATERAL_INFO
        .load(&deps.storage, (creator_addr.clone(), 0))
        .unwrap();
    assert_eq!(
        coll_info,
        CollateralInfo {
            associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                address: "nft".to_string(),
                token_id: "58".to_string()
            })],
            list_date: mock_env().block.time,
            state: LoanState::Inactive,
            ..Default::default()
        }
    );

    let coll_info = COLLATERAL_INFO
        .load(&deps.storage, (creator_addr.clone(), 1))
        .unwrap();
    assert_eq!(
        coll_info,
        CollateralInfo {
            associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                address: "nft".to_string(),
                token_id: "59".to_string()
            })],
            list_date: mock_env().block.time,
            state: LoanState::Inactive,
            ..Default::default()
        }
    );

    let coll_info = COLLATERAL_INFO
        .load(&deps.storage, (creator_addr, 2))
        .unwrap();
    assert_eq!(
        coll_info,
        CollateralInfo {
            terms: None,
            associated_assets: vec![AssetInfo::Cw1155Coin(Cw1155Coin {
                address: "nft".to_string(),
                token_id: "59".to_string(),
                value: Uint128::from(459u128)
            })],
            list_date: mock_env().block.time,
            ..Default::default()
        }
    );
    // You shouldn't be able to repay the loan now
    let repay_err =
        repay_borrowed_funds_helper(deps.as_mut(), "creator", 0, coins(506, "luna"), mock_env())
            .unwrap_err();
    assert_error(
        repay_err,
        ContractError::WrongLoanState {
            state: LoanState::Inactive,
        },
    )
}

#[test]
fn test_accept_loan() {
    let mut deps = mock_querier_dependencies(&[]);
    deps.querier
        .with_owner_of(&[
            (&"nft - 58".to_string(), &"creator".to_string()),
            (&"nft - 59".to_string(), &"creator".to_string())
        ]);

    init_helper(deps.as_mut());
    add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "59",
        Some(Uint128::from(459u128)),
        None,
    )
    .unwrap();

    let terms = LoanTerms {
        principle: coin(456, "luna"),
        interest: Uint128::new(0),
        duration_in_blocks: 0,
    };
    set_terms_helper(deps.as_mut(), "creator", 0, terms.clone()).unwrap();

    // The funds have to match the terms
    let err =
        accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coins(123, "luna")).unwrap_err();
    assert_error(err, ContractError::FundsDontMatchTerms {});
    let err = accept_loan_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        vec![coin(123, "luna"), coin(457, "uusd")],
    )
    .unwrap_err();
    assert_error(err, ContractError::MultipleCoins {});

    let res =
        accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coins(456, "luna")).unwrap();
    assert_eq!(2, res.messages.len());

    accept_loan_helper(
        deps.as_mut(),
        "anyone_else",
        "creator",
        0,
        coins(456, "luna"),
    )
    .unwrap_err();
    let creator_addr = deps.api.addr_validate("creator").unwrap();
    let coll_info = COLLATERAL_INFO
        .load(&deps.storage, (creator_addr, 0))
        .unwrap();

    assert_eq!(
        coll_info,
        CollateralInfo {
            terms: Some(terms),
            associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                address: "nft".to_string(),
                token_id: "58".to_string()
            })],
            state: LoanState::Started,
            active_offer: Some("1".to_string()),
            start_block: Some(12345),
            offer_amount: 1,
            comment: None,
            list_date: mock_env().block.time,
            loan_preview: None
        }
    );
}

#[test]
fn test_accept_loan_and_modify() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());

    let terms = LoanTerms {
        principle: coin(456, "luna"),
        interest: Uint128::new(0),
        duration_in_blocks: 0,
    };
    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "58",
        Some(Uint128::from(8_u128)),
        Some(terms.clone()),
    )
    .unwrap();
    accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coins(456, "luna")).unwrap();

    // We try to modify the loan
    let modify_err = set_terms_helper(deps.as_mut(), "creator", 0, terms.clone()).unwrap_err();
    assert_error(modify_err, ContractError::NotModifiable {});

    // We try to counter the loan, and propose new terms
    let offer_err = make_offer_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        terms,
        coins(456, "luna"),
    )
    .unwrap_err();

    assert_error(offer_err, ContractError::NotCounterable {});
}

#[test]
fn test_repay_loan_early() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());

    let terms = LoanTerms {
        principle: coin(456, "luna"),
        interest: Uint128::new(0),
        duration_in_blocks: 0,
    };
    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "58",
        Some(Uint128::from(8_u128)),
        Some(terms),
    )
    .unwrap();
    let repay_err =
        repay_borrowed_funds_helper(deps.as_mut(), "creator", 0, coins(506, "luna"), mock_env())
            .unwrap_err();
    assert_error(
        repay_err,
        ContractError::WrongLoanState {
            state: LoanState::Published,
        },
    )
}

#[test]
fn test_make_offer() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());
    add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "59",
        Some(Uint128::from(459u128)),
        None,
    )
    .unwrap();

    let terms = LoanTerms {
        principle: coin(456, "luna"),
        interest: Uint128::new(0),
        duration_in_blocks: 0,
    };

    let err = make_offer_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        terms.clone(),
        coins(6765, "luna"),
    )
    .unwrap_err();
    assert_error(err, ContractError::FundsDontMatchTerms {});

    let err = make_offer_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        terms.clone(),
        vec![coin(456, "luna"), coin(456, "luna")],
    )
    .unwrap_err();
    assert_error(err, ContractError::MultipleCoins {});

    make_offer_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        terms,
        coins(456, "luna"),
    )
    .unwrap();
}

#[test]
fn test_cancel_offer() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());
    add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "59",
        Some(Uint128::from(459u128)),
        None,
    )
    .unwrap();

    let terms = LoanTerms {
        principle: coin(456, "luna"),
        interest: Uint128::new(0),
        duration_in_blocks: 0,
    };

    make_offer_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        terms,
        coins(456, "luna"),
    )
    .unwrap();

    cancel_offer_helper(deps.as_mut(), "anyone_else", "1").unwrap_err();

    let res = cancel_offer_helper(deps.as_mut(), "anyone", "1").unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(BankMsg::Send {
            to_address: "anyone".to_string(),
            amount: coins(456, "luna"),
        }),]
    );

    cancel_offer_helper(deps.as_mut(), "anyone", "1").unwrap_err();
}

#[test]
fn test_refuse_offer() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());
    add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "59",
        Some(Uint128::from(459u128)),
        None,
    )
    .unwrap();

    let terms = LoanTerms {
        principle: coin(456, "luna"),
        interest: Uint128::new(0),
        duration_in_blocks: 0,
    };

    make_offer_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        terms,
        coins(456, "luna"),
    )
    .unwrap();

    refuse_offer_helper(deps.as_mut(), "bad_person", "1").unwrap_err();
    refuse_offer_helper(deps.as_mut(), "creator", "1").unwrap();

    let offer = lender_offers().load(&deps.storage, "1").unwrap();

    assert_eq!(offer.state, OfferState::Refused);
}

#[test]
fn test_cancel_accepted() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());

    let terms = LoanTerms {
        principle: coin(456, "luna"),
        interest: Uint128::new(0),
        duration_in_blocks: 0,
    };

    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "58",
        Some(Uint128::new(45u128)),
        Some(terms),
    )
    .unwrap();

    accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coins(456, "luna")).unwrap();

    withdraw_collateral_helper(deps.as_mut(), "creator", 0).unwrap_err();
    cancel_offer_helper(deps.as_mut(), "anyone", "1").unwrap_err();
}

#[test]
fn test_withdraw_refused() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());

    let terms = LoanTerms {
        principle: coin(456, "luna"),
        interest: Uint128::new(0),
        duration_in_blocks: 0,
    };

    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "58",
        Some(Uint128::new(45u128)),
        Some(terms.clone()),
    )
    .unwrap();
    make_offer_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        terms.clone(),
        coins(456, "luna"),
    )
    .unwrap();

    make_offer_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        terms,
        coins(456, "luna"),
    )
    .unwrap();

    println!("{:?}", CONTRACT_INFO.load(&deps.storage).unwrap());

    withdraw_refused_offer_helper(deps.as_mut(), "anyone", "1").unwrap_err();
    withdraw_refused_offer_helper(deps.as_mut(), "anyone", "2").unwrap_err();
    let err = withdraw_refused_offer_helper(deps.as_mut(), "anyone", "87").unwrap_err();
    assert_error(err, ContractError::OfferNotFound {});

    let err = accept_offer_helper(deps.as_mut(), "creator", "87").unwrap_err();
    assert_error(err, ContractError::OfferNotFound {});
    accept_offer_helper(deps.as_mut(), "creator", "1").unwrap();

    withdraw_refused_offer_helper(deps.as_mut(), "anyone", "1").unwrap_err();
    withdraw_refused_offer_helper(deps.as_mut(), "anyone_else", "2").unwrap_err();
    withdraw_refused_offer_helper(deps.as_mut(), "anyone", "2").unwrap();
    withdraw_refused_offer_helper(deps.as_mut(), "anyone", "2").unwrap_err();
}
#[test]
fn test_accept_cancelled_offer() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());
    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "58",
        Some(Uint128::new(45u128)),
        None,
    )
    .unwrap();

    let terms = LoanTerms {
        principle: coin(456, "luna"),
        interest: Uint128::new(0),
        duration_in_blocks: 0,
    };

    make_offer_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        terms,
        coins(456, "luna"),
    )
    .unwrap();

    cancel_offer_helper(deps.as_mut(), "anyone", "1").unwrap();
    let err = accept_offer_helper(deps.as_mut(), "creator", "1").unwrap_err();
    assert_error(
        err,
        ContractError::WrongOfferState {
            state: OfferState::Cancelled,
        },
    )
}

#[test]
fn test_normal_flow() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    init_helper(deps.as_mut());

    let terms = LoanTerms {
        principle: coin(456, "luna"),
        interest: Uint128::new(50),
        duration_in_blocks: 1,
    };

    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "58",
        Some(Uint128::new(45u128)),
        Some(terms.clone()),
    )
    .unwrap();
    make_offer_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        terms,
        coins(456, "luna"),
    )
    .unwrap();

    accept_offer_helper(deps.as_mut(), "creator", "1").unwrap();
    // Loan starts

    let err =
        repay_borrowed_funds_helper(deps.as_mut(), "creator", 0, coins(456, "luna"), env.clone())
            .unwrap_err();
    assert_error(
        err,
        ContractError::FundsDontMatchTermsAndPrinciple(
            Uint128::from(506u128),
            Uint128::from(456u128),
        ),
    );
    let err = repay_borrowed_funds_helper(
        deps.as_mut(),
        "creator",
        0,
        vec![coin(456, "luna"), coin(456, "luna")],
        env.clone(),
    )
    .unwrap_err();
    assert_error(err, ContractError::MultipleCoins {});
    let err =
        repay_borrowed_funds_helper(deps.as_mut(), "creator", 0, coins(456, "uust"), env.clone())
            .unwrap_err();
    assert_error(err, ContractError::FundsDontMatchTerms {});

    repay_borrowed_funds_helper(
        deps.as_mut(),
        "bad_person",
        0,
        coins(506, "luna"),
        env.clone(),
    )
    .unwrap_err();

    let res =
        repay_borrowed_funds_helper(deps.as_mut(), "creator", 0, coins(506, "luna"), env).unwrap();
    let env = mock_env();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(BankMsg::Send {
                to_address: "anyone".to_string(),
                amount: coins(503, "luna"),
            }),
            SubMsg::new(
                into_cosmos_msg(
                    Cw1155ExecuteMsg::SendFrom {
                        from: env.contract.address.to_string(),
                        to: "creator".to_string(),
                        token_id: "58".to_string(),
                        value: Uint128::new(45u128),
                        msg: None,
                    },
                    "nft",
                    None
                )
                .unwrap()
            ),
            SubMsg::new(
                into_cosmos_msg(
                    FeeDistributorMsg::DepositFees {
                        addresses: vec!["nft".to_string()],
                        fee_type: FeeType::Funds
                    },
                    "fee_distributor",
                    Some(coins(3, "luna"))
                )
                .unwrap()
            )
        ]
    );
}

#[test]
fn test_defaulted_flow() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());

    let terms = LoanTerms {
        principle: coin(456, "luna"),
        interest: Uint128::new(0),
        duration_in_blocks: 0,
    };

    add_collateral_helper(
        deps.as_mut(),
        "creator",
        "nft",
        "58",
        Some(Uint128::new(45u128)),
        None,
    )
    .unwrap();
    make_offer_helper(
        deps.as_mut(),
        "anyone",
        "creator",
        0,
        terms,
        coins(456, "luna"),
    )
    .unwrap();

    accept_offer_helper(deps.as_mut(), "creator", "1").unwrap();
    let mut env = mock_env();
    env.block.height = 12346;
    let err =
        repay_borrowed_funds_helper(deps.as_mut(), "creator", 0, coins(456, "luna"), env.clone())
            .unwrap_err();
    assert_error(
        err,
        ContractError::WrongLoanState {
            state: LoanState::Defaulted {},
        },
    );

    let err =
        withdraw_defaulted_loan_helper(deps.as_mut(), "bad_person", "creator", 0, env.clone())
            .unwrap_err();
    assert_error(err, ContractError::Unauthorized {});
    withdraw_defaulted_loan_helper(deps.as_mut(), "anyone", "creator", 0, env.clone()).unwrap();
    withdraw_defaulted_loan_helper(deps.as_mut(), "anyone", "creator", 0, env).unwrap_err();
}

#[test]
fn test_steal_funds() {
    // modification of test_normal_flow() test case
    // reproduced in contracts/nft-loans-non-custodial/src/testing/tests.rs
    // note: attacker is both the lender and borrower
    let mut deps = mock_dependencies();
    let env = mock_env();
    init_helper(deps.as_mut());
    // malicious terms, interest set to 0 to prevent fee distribution
    let terms = LoanTerms {
        principle: coin(1000, "luna"),
        interest: Uint128::new(0),
        duration_in_blocks: 1,
    };
    // attacker deposit nft collateral
    add_collateral_helper(
        deps.as_mut(),
        "attacker",
        "nft",
        "58",
        Some(Uint128::new(1000_u128)),
        Some(terms),
    )
    .unwrap();
    // attacker accepts their own offer
    // 1. attacker send 1000 LUNA to contract
    // 2. contract takes attacker's NFT
    // 3. contract sends 1000 LUNA to attacker
    accept_loan_helper(
        deps.as_mut(),
        "attacker",
        "attacker",
        0,
        coins(1000, "luna"),
    )
    .unwrap();
    // attacker repay the funds
    // 1. attacker send 1000 LUNA to contract
    // 2. contract sends back attacker's NFT
    repay_borrowed_funds_helper(deps.as_mut(), "attacker", 0, coins(1000, "luna"), env).unwrap();
    // attacker calls `RefuseOffer` to mutate offer state to `Refused`
    let err = refuse_offer_helper(deps.as_mut(), "attacker", "1").unwrap_err();
    // The attacker can't refuse an offer that was already accepted or withdrawn, etc.
    assert_error(err, ContractError::NotRefusable {  });
}
