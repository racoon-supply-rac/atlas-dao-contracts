use std::str::FromStr;
use anyhow::Result;
extern crate rustc_serialize as serialize;
use serialize::base64::{self, ToBase64};
use serialize::hex::FromHex;

use cosmwasm_std::{
    coin, coins, from_binary,
    testing::{mock_dependencies, mock_env, mock_info, MOCK_CONTRACT_ADDR},
    Api, BankMsg, Binary, Coin, DepsMut, Event, Response, SubMsg, SubMsgResponse, SubMsgResult,
    Uint128, Decimal
};
use utils::state::OwnerStruct;

use raffles_export::msg::{
    into_cosmos_msg, AllRafflesResponse, DrandRandomness, ExecuteMsg, InstantiateMsg, QueryFilters,
    QueryMsg, RaffleResponse, VerifierExecuteMsg,
};
use raffles_export::state::{
    AssetInfo, ContractInfo, Cw721Coin, RaffleInfo, RaffleOptions, RaffleOptionsMsg, RaffleState,
    Randomness,
};

use crate::contract::{execute, instantiate, query, verify};
use crate::error::ContractError;
use crate::state::{CONTRACT_INFO, RAFFLE_INFO};

use cw1155::Cw1155ExecuteMsg;
use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use crate::testing::mock_querier::mock_querier_dependencies;

const HEX_PUBKEY: &str = "868f005eb8e6e4ca0a47c8a77ceaa5309a47978a7c71bc5cce96366b5d7a569937c529eeda66c7293784a9402801af31";

pub fn assert_error(err: anyhow::Error, contract_error: ContractError) {
    assert_eq!(err.downcast::<ContractError>().unwrap(), contract_error)
}

fn init_helper(deps: DepsMut) {
    let instantiate_msg = InstantiateMsg {
        name: "nft-raffle".to_string(),
        owner: None,
        random_pubkey: HEX_PUBKEY.from_hex().unwrap().to_base64(base64::STANDARD),
        drand_url: None,
        verify_signature_contract: "verifier".to_string(),
        fee_addr: None,
        minimum_raffle_timeout: None,
        minimum_raffle_duration: None,
        raffle_fee: Some(Decimal::from_str("0.0002").unwrap()),
        rand_fee: None,
        max_participant_number: None,
    };
    let info = mock_info("creator", &[]);
    let env = mock_env();

    instantiate(deps, env, info, instantiate_msg).unwrap();
}

fn create_raffle(deps: DepsMut) -> Result<Response> {
    let info = mock_info("creator", &[]);
    let env = mock_env();

    Ok(execute(
        deps,
        env,
        info,
        ExecuteMsg::CreateRaffle {
            owner: None,
            assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                address: "nft".to_string(),
                token_id: "token_id".to_string(),
            })],
            raffle_options: RaffleOptionsMsg::default(),
            raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
        },
    )?)
}

fn cancel_raffle_helper(deps: DepsMut, caller: &str, raffle_id: u64) -> Result<Response> {
    let info = mock_info(caller, &[]);
    let env = mock_env();

    Ok(execute(
        deps,
        env,
        info,
        ExecuteMsg::CancelRaffle { raffle_id }
    )?)
}

fn create_raffle_comment(deps: DepsMut, comment: &str) -> Result<Response> {
    let info = mock_info("creator", &[]);
    let env = mock_env();

    Ok(execute(
        deps,
        env,
        info,
        ExecuteMsg::CreateRaffle {
            owner: None,
            assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                address: "nft".to_string(),
                token_id: "token_id".to_string(),
            })],
            raffle_options: RaffleOptionsMsg {
                comment: Some(comment.to_string()),
                ..Default::default()
            },

            raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
        },
    )?)
}

fn create_raffle_cw20(deps: DepsMut) -> Result<Response> {
    let info = mock_info("creator", &[]);
    let env = mock_env();

    Ok(execute(
        deps,
        env,
        info,
        ExecuteMsg::CreateRaffle {
            owner: None,
            assets: vec![AssetInfo::cw721("nft", "token_id")],
            raffle_options: RaffleOptionsMsg::default(),
            raffle_ticket_price: AssetInfo::cw20(10000u128, "address"),
        },
    )?)
}

fn create_raffle_cw1155(deps: DepsMut) -> Result<Response> {
    let info = mock_info("creator", &[]);
    let env = mock_env();

    Ok(execute(
        deps,
        env,
        info,
        ExecuteMsg::CreateRaffle {
            owner: None,
            assets: vec![AssetInfo::cw1155("nft", "token_id", 675u128)],
            raffle_options: RaffleOptionsMsg::default(),
            raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
        },
    )?)
}

fn buy_ticket_coin(
    deps: DepsMut,
    raffle_id: u64,
    buyer: &str,
    c: Coin,
    delta: u64,
    ticket_number: Option<u32>,
) -> Result<Response> {
    let info = mock_info(buyer, &[c.clone()]);
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(delta);
    Ok(execute(
        deps,
        env,
        info,
        ExecuteMsg::BuyTicket {
            raffle_id,
            sent_assets: AssetInfo::Coin(c),
            ticket_number: ticket_number.unwrap_or(1u32),
        },
    )?)
}

fn buy_ticket_cw20(
    deps: DepsMut,
    raffle_id: u64,
    buyer: &str,
    amount: u128,
    address: &str,
    delta: u64,
) -> Result<Response> {
    let info = mock_info(buyer, &[]);
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(delta);
    Ok(execute(
        deps,
        env,
        info,
        ExecuteMsg::BuyTicket {
            raffle_id,
            sent_assets: AssetInfo::cw20(amount, address),
            ticket_number: 1,
        },
    )?)
}

fn claim_nft(deps: DepsMut, raffle_id: u64, time_delta: u64) -> Result<Response> {
    let info = mock_info("creator", &[]);
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(time_delta);
    Ok(execute(deps, env, info, ExecuteMsg::ClaimNft { raffle_id })?)
}

#[test]
fn test_init_sanity() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());
}

#[test]
fn test_create_raffle() {
    let mut deps = mock_querier_dependencies(&[]);
    deps.querier
        .with_owner_of(&[
            (&"nft - token_id".to_string(), &"creator".to_string())
        ]);
        
    init_helper(deps.as_mut());
    let response = create_raffle(deps.as_mut()).unwrap();

    assert_eq!(
        response.messages,
        vec![SubMsg::new(
            into_cosmos_msg(
                Cw721ExecuteMsg::TransferNft {
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                    token_id: "token_id".to_string(),
                },
                "nft"
            )
            .unwrap()
        )]
    );
}

#[test]
fn test_create_and_cancel_raffle() {
    let mut deps = mock_querier_dependencies(&[]);
    deps.querier
        .with_owner_of(&[
            (&"nft - token_id".to_string(), &"creator".to_string())
        ]);
        
    init_helper(deps.as_mut());
    create_raffle(deps.as_mut()).unwrap();

    let err = cancel_raffle_helper(deps.as_mut(), "bad_person", 0).unwrap_err();

    assert_error(
        err,
        ContractError::Unauthorized {  }
    );
    let response = cancel_raffle_helper(deps.as_mut(), "creator", 0).unwrap();

    assert_eq!(
        response.messages,
        vec![SubMsg::new(
            into_cosmos_msg(
                Cw721ExecuteMsg::TransferNft {
                    recipient: "creator".to_string(),
                    token_id: "token_id".to_string(),
                },
                "nft"
            )
            .unwrap()
        )]
    );

    // Can't cancel twice
    let err = cancel_raffle_helper(deps.as_mut(), "creator", 0).unwrap_err();
    assert_error(err, ContractError::WrongStateForCancel { status: RaffleState::Cancelled })

}

#[test]
fn test_claim_raffle() {
    let mut deps = mock_querier_dependencies(&[]);
    deps.querier
        .with_owner_of(&[
            (&"nft - token_id".to_string(), &"creator".to_string())
        ]);

    init_helper(deps.as_mut());
    create_raffle(deps.as_mut()).unwrap();

    // Update the randomness internally
    let mut raffle_info = RAFFLE_INFO.load(&deps.storage, 0).unwrap();

    let mut randomness: [u8; 32] = [0; 32];
    hex::decode_to_slice(
        "89580f6a639add6c90dcf3d222e35415f89d9ee2cd6ef6fc4f23134cdffa5d1e",
        randomness.as_mut_slice(),
    )
    .unwrap();

    let claim_too_soon_error = claim_nft(deps.as_mut(), 0, 0u64).unwrap_err();
    assert_eq!(
        claim_too_soon_error.downcast::<ContractError>().unwrap(),
        ContractError::WrongStateForClaim {
            status: RaffleState::Started
        }
    );

    let claim_too_soon_error = claim_nft(deps.as_mut(), 0, 1000u64).unwrap_err();
    assert_eq!(
        claim_too_soon_error.downcast::<ContractError>().unwrap(),
        ContractError::WrongStateForClaim {
            status: RaffleState::Closed
        }
    );

    raffle_info.randomness = Some(Randomness {
        randomness,
        randomness_round: 2098475u64,
        randomness_owner: deps.api.addr_validate("rand_provider").unwrap(),
    });
    RAFFLE_INFO
        .save(deps.as_mut().storage, 0, &raffle_info)
        .unwrap();

    claim_nft(deps.as_mut(), 0, 1000u64).unwrap();

    claim_nft(deps.as_mut(), 0, 1000u64).unwrap_err();
}

#[test]
fn test_ticket_and_claim_raffle() {
    let mut deps = mock_querier_dependencies(&[]);
    deps.querier
        .with_owner_of(&[
            (&"nft - token_id".to_string(), &"creator".to_string())
        ]);
        
    init_helper(deps.as_mut());
    create_raffle(deps.as_mut()).unwrap();

    //Buy some tickets
    buy_ticket_coin(deps.as_mut(), 0, "first", coin(10, "uluna"), 0u64, None).unwrap_err();
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(1000000, "uluna"),
        0u64,
        None,
    )
    .unwrap_err();
    buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 0u64, None).unwrap();
    buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 0u64, None).unwrap();
    buy_ticket_coin(deps.as_mut(), 0, "second", coin(10000, "uluna"), 0u64, None).unwrap();
    buy_ticket_coin(deps.as_mut(), 0, "third", coin(10000, "uluna"), 0u64, None).unwrap();
    buy_ticket_coin(deps.as_mut(), 0, "fourth", coin(10000, "uluna"), 0u64, None).unwrap();

    // Update the randomness internally
    let mut raffle_info = RAFFLE_INFO.load(&deps.storage, 0).unwrap();

    let mut randomness: [u8; 32] = [0; 32];
    hex::decode_to_slice(
        "89580f6a639add6c90dcf3d222e35415f89d9ee2cd6ef6fc4f23134cdffa5d1e",
        randomness.as_mut_slice(),
    )
    .unwrap();
    raffle_info.randomness = Some(Randomness {
        randomness,
        randomness_round: 2098475u64,
        randomness_owner: deps.api.addr_validate("rand_provider").unwrap(),
    });
    RAFFLE_INFO
        .save(deps.as_mut().storage, 0, &raffle_info)
        .unwrap();

    let response = claim_nft(deps.as_mut(), 0, 1000u64).unwrap();

    assert_eq!(
        response.messages,
        vec![
            SubMsg::new(
                into_cosmos_msg(
                    Cw721ExecuteMsg::TransferNft {
                        recipient: "third".to_string(),
                        token_id: "token_id".to_string()
                    },
                    "nft".to_string()
                )
                .unwrap()
            ),
            SubMsg::new(BankMsg::Send {
                to_address: "rand_provider".to_string(),
                amount: coins(5, "uluna")
            }),
            SubMsg::new(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: coins(10, "uluna")
            }),
            SubMsg::new(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: coins(49985u128, "uluna")
            }),
        ]
    );

    // You can't buy tickets when the raffle is over
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(10000, "uluna"),
        100u64,
        None,
    )
    .unwrap_err();
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(10000, "uluna"),
        1000u64,
        None,
    )
    .unwrap_err();
}

#[test]
fn test_multiple_tickets_and_claim_raffle() {
    let mut deps = mock_querier_dependencies(&[]);
    deps.querier
        .with_owner_of(&[
            (&"nft - token_id".to_string(), &"creator".to_string())
        ]);
        
    init_helper(deps.as_mut());
    create_raffle(deps.as_mut()).unwrap();

    //Buy some tickets
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(10000, "uluna"),
        0u64,
        Some(5),
    )
    .unwrap_err();
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(40000, "uluna"),
        0u64,
        Some(5),
    )
    .unwrap_err();
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(20000, "uluna"),
        0u64,
        Some(2),
    )
    .unwrap();
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(10000, "uluna"),
        0u64,
        Some(1),
    )
    .unwrap();
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(20000, "uluna"),
        0u64,
        Some(2),
    )
    .unwrap();

    // Update the randomness internally
    let mut raffle_info = RAFFLE_INFO.load(&deps.storage, 0).unwrap();

    let mut randomness: [u8; 32] = [0; 32];
    hex::decode_to_slice(
        "89580f6a639add6c90dcf3d222e35415f89d9ee2cd6ef6fc4f23134cdffa5d1e",
        randomness.as_mut_slice(),
    )
    .unwrap();
    raffle_info.randomness = Some(Randomness {
        randomness,
        randomness_round: 2098475u64,
        randomness_owner: deps.api.addr_validate("rand_provider").unwrap(),
    });
    RAFFLE_INFO
        .save(deps.as_mut().storage, 0, &raffle_info)
        .unwrap();

    let response = claim_nft(deps.as_mut(), 0, 1000u64).unwrap();

    assert_eq!(
        response.messages,
        vec![
            SubMsg::new(
                into_cosmos_msg(
                    Cw721ExecuteMsg::TransferNft {
                        recipient: "first".to_string(),
                        token_id: "token_id".to_string()
                    },
                    "nft".to_string()
                )
                .unwrap()
            ),
            SubMsg::new(BankMsg::Send {
                to_address: "rand_provider".to_string(),
                amount: coins(5, "uluna")
            }),
            SubMsg::new(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: coins(10, "uluna")
            }),
            SubMsg::new(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: coins(49985u128, "uluna")
            }),
        ]
    );
}

#[test]
fn test_ticket_and_claim_raffle_cw20() {
    let mut deps = mock_querier_dependencies(&[]);
    deps.querier
        .with_owner_of(&[
            (&"nft - token_id".to_string(), &"creator".to_string())
        ]);
        
    init_helper(deps.as_mut());
    create_raffle_cw20(deps.as_mut()).unwrap();

    //Buy some tickets

    buy_ticket_cw20(deps.as_mut(), 0, "first", 100u128, "address", 0u64).unwrap_err();
    buy_ticket_cw20(deps.as_mut(), 0, "first", 1000000000u128, "address", 0u64).unwrap_err();

    let response = buy_ticket_cw20(deps.as_mut(), 0, "first", 10000u128, "address", 0u64).unwrap();
    assert_eq!(
        response.messages,
        vec![SubMsg::new(
            into_cosmos_msg(
                Cw20ExecuteMsg::Transfer {
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                    amount: Uint128::from(10000u128),
                },
                "address".to_string()
            )
            .unwrap()
        )]
    );

    buy_ticket_cw20(deps.as_mut(), 0, "first", 10000u128, "address", 0u64).unwrap();
    buy_ticket_cw20(deps.as_mut(), 0, "second", 10000u128, "address", 0u64).unwrap();
    buy_ticket_cw20(deps.as_mut(), 0, "third", 10000u128, "address", 0u64).unwrap();
    buy_ticket_cw20(deps.as_mut(), 0, "fourth", 10000u128, "address", 0u64).unwrap();

    // Update the randomness internally
    let mut raffle_info = RAFFLE_INFO.load(&deps.storage, 0).unwrap();

    let mut randomness: [u8; 32] = [0; 32];
    hex::decode_to_slice(
        "89580f6a639add6c90dcf3d222e35415f89d9ee2cd6ef6fc4f23134cdffa5d1e",
        randomness.as_mut_slice(),
    )
    .unwrap();
    raffle_info.randomness = Some(Randomness {
        randomness,
        randomness_round: 2098475u64,
        randomness_owner: deps.api.addr_validate("rand_provider").unwrap(),
    });
    RAFFLE_INFO
        .save(deps.as_mut().storage, 0, &raffle_info)
        .unwrap();

    let response = claim_nft(deps.as_mut(), 0, 1000u64).unwrap();

    assert_eq!(
        response.messages,
        vec![
            SubMsg::new(
                into_cosmos_msg(
                    Cw721ExecuteMsg::TransferNft {
                        recipient: "third".to_string(),
                        token_id: "token_id".to_string()
                    },
                    "nft".to_string()
                )
                .unwrap()
            ),
            SubMsg::new(
                into_cosmos_msg(
                    Cw20ExecuteMsg::Transfer {
                        recipient: "rand_provider".to_string(),
                        amount: Uint128::from(5u128)
                    },
                    "address".to_string()
                )
                .unwrap()
            ),
            SubMsg::new(
                into_cosmos_msg(
                    Cw20ExecuteMsg::Transfer {
                        recipient: "creator".to_string(),
                        amount: Uint128::from(10u128)
                    },
                    "address".to_string()
                )
                .unwrap()
            ),
            SubMsg::new(
                into_cosmos_msg(
                    Cw20ExecuteMsg::Transfer {
                        recipient: "creator".to_string(),
                        amount: Uint128::from(49985u128)
                    },
                    "address".to_string()
                )
                .unwrap()
            ),
        ]
    );

    // You can't buy tickets when the raffle is over
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(10000, "uluna"),
        100u64,
        None,
    )
    .unwrap_err();
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(10000, "uluna"),
        1000u64,
        None,
    )
    .unwrap_err();
}
#[test]
fn test_ticket_and_claim_raffle_cw1155() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());
    let response = create_raffle_cw1155(deps.as_mut()).unwrap();

    assert_eq!(
        response.messages,
        vec![SubMsg::new(
            into_cosmos_msg(
                Cw1155ExecuteMsg::SendFrom {
                    from: "creator".to_string(),
                    to: MOCK_CONTRACT_ADDR.to_string(),
                    token_id: "token_id".to_string(),
                    value: Uint128::from(675u128),
                    msg: None,
                },
                "nft"
            )
            .unwrap()
        )]
    );

    //Buy some tickets
    buy_ticket_coin(deps.as_mut(), 0, "first", coin(10, "uluna"), 0u64, None).unwrap_err();
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(1000000, "uluna"),
        0u64,
        None,
    )
    .unwrap_err();
    buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 0u64, None).unwrap();
    buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 0u64, None).unwrap();
    buy_ticket_coin(deps.as_mut(), 0, "second", coin(10000, "uluna"), 0u64, None).unwrap();
    buy_ticket_coin(deps.as_mut(), 0, "third", coin(10000, "uluna"), 0u64, None).unwrap();
    buy_ticket_coin(deps.as_mut(), 0, "fourth", coin(10000, "uluna"), 0u64, None).unwrap();

    // Update the randomness internally
    let mut raffle_info = RAFFLE_INFO.load(&deps.storage, 0).unwrap();

    let mut randomness: [u8; 32] = [0; 32];
    hex::decode_to_slice(
        "89580f6a639add6c90dcf3d222e35415f89d9ee2cd6ef6fc4f23134cdffa5d1e",
        randomness.as_mut_slice(),
    )
    .unwrap();
    raffle_info.randomness = Some(Randomness {
        randomness,
        randomness_round: 2098475u64,
        randomness_owner: deps.api.addr_validate("rand_provider").unwrap(),
    });
    RAFFLE_INFO
        .save(deps.as_mut().storage, 0, &raffle_info)
        .unwrap();

    let response = claim_nft(deps.as_mut(), 0, 1000u64).unwrap();

    assert_eq!(
        response.messages,
        vec![
            SubMsg::new(
                into_cosmos_msg(
                    Cw1155ExecuteMsg::SendFrom {
                        from: MOCK_CONTRACT_ADDR.to_string(),
                        to: "third".to_string(),
                        token_id: "token_id".to_string(),
                        value: Uint128::from(675u128),
                        msg: None,
                    },
                    "nft".to_string()
                )
                .unwrap()
            ),
            SubMsg::new(BankMsg::Send {
                to_address: "rand_provider".to_string(),
                amount: coins(5, "uluna")
            }),
            SubMsg::new(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: coins(10, "uluna")
            }),
            SubMsg::new(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: coins(49985u128, "uluna")
            }),
        ]
    );

    // You can't buy tickets when the raffle is over
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(10000, "uluna"),
        100u64,
        None,
    )
    .unwrap_err();
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(10000, "uluna"),
        1000u64,
        None,
    )
    .unwrap_err();
}

// #[test]
// fn test_randomness_provider() {
//     let mut deps = mock_dependencies();
//     init_helper(deps.as_mut());
//     create_raffle_cw1155(deps.as_mut()).unwrap();
//     let mut env = mock_env();
//     env.block.time = env.block.time.plus_seconds(2u64);
//     let info = mock_info("anyone", &[]);
//     let mut randomness = DrandRandomness {
//         round: 90,
//         signature: Binary::from_base64("quid").unwrap(),
//         previous_signature: Binary::from_base64("quid").unwrap(),
//     };
//     let response = execute(
//         deps.as_mut(),
//         env.clone(),
//         info.clone(),
//         ExecuteMsg::UpdateRandomness {
//             raffle_id: 0,
//             randomness: randomness.clone(),
//         },
//     )
//     .unwrap();
//     let msg = VerifierExecuteMsg::Verify {
//         randomness: randomness.clone(),
//         pubkey: Binary::from_base64(&HEX_PUBKEY.from_hex().unwrap().to_base64(base64::STANDARD))
//             .unwrap(),
//         raffle_id: 0,
//         owner: "anyone".to_string(),
//     };

//     assert_eq!(
//         response.messages,
//         vec![SubMsg::reply_on_success(
//             into_cosmos_msg(msg, "verifier".to_string()).unwrap(),
//             0
//         )]
//     );
//     let random = "iVgPamOa3WyQ3PPSIuNUFfidnuLNbvb8TyMTTN/6XR4=";

//     verify(
//         deps.as_mut(),
//         env.clone(),
//         SubMsgResult::Ok(SubMsgResponse {
//             events: vec![Event::new("wasm")
//                 .add_attribute("round", 90u128.to_string())
//                 .add_attribute("owner", "anyone")
//                 .add_attribute("randomness", random)
//                 .add_attribute("raffle_id", 0u128.to_string())],
//             data: None,
//         }),
//     )
//     .unwrap();

//     randomness.round = 76;
//     let another_randomness = execute(
//         deps.as_mut(),
//         env.clone(),
//         info.clone(),
//         ExecuteMsg::UpdateRandomness {
//             raffle_id: 0,
//             randomness: randomness.clone(),
//         },
//     )
//     .unwrap_err();

//     assert_eq!(
//         another_randomness.downcast::<ContractError>().unwrap(),
//         ContractError::RandomnessNotAccepted { current_round: 90 }
//     );

//     randomness.round = 90;
//     let another_randomness = execute(
//         deps.as_mut(),
//         env.clone(),
//         info.clone(),
//         ExecuteMsg::UpdateRandomness {
//             raffle_id: 0,
//             randomness: randomness.clone(),
//         },
//     )
//     .unwrap_err();

//     assert_eq!(
//         another_randomness.downcast::<ContractError>().unwrap(),
//         ContractError::RandomnessNotAccepted { current_round: 90 }
//     );

//     randomness.round = 100;
//     execute(
//         deps.as_mut(),
//         env,
//         info,
//         ExecuteMsg::UpdateRandomness {
//             raffle_id: 0,
//             randomness,
//         },
//     )
//     .unwrap();
// }

// Admin functions
#[test]
fn test_renounce() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());
    let info = mock_info("bad_person", &[]);
    let env = mock_env();
    execute(deps.as_mut(), env.clone(), info, ExecuteMsg::ChangeParameter { parameter: "owner".to_string(), value: env.contract.address.to_string() }).unwrap_err();

    let info = mock_info("creator", &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        ExecuteMsg::ChangeParameter { parameter: "owner".to_string(), value: env.contract.address.to_string() },
    )
    .unwrap();
    // Still admin
    execute(deps.as_mut(), env.clone(), info.clone(), ExecuteMsg::ChangeParameter { parameter: "owner".to_string(), value: env.contract.address.to_string() }).unwrap();

    // Claim ownership
    execute(deps.as_mut(), env.clone(), mock_info(&env.contract.address.to_string(), &[]), ExecuteMsg::ClaimOwnership {  }).unwrap();

    // Not admin anymore
    execute(deps.as_mut(), env.clone(), info, ExecuteMsg::ChangeParameter { parameter: "owner".to_string(), value: env.contract.address.to_string() }).unwrap_err();

}

#[test]
fn test_lock() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());
    assert!(!CONTRACT_INFO.load(&deps.storage).unwrap().lock);

    let info = mock_info("bad_person", &[]);
    let env = mock_env();
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ToggleLock { lock: false },
    )
    .unwrap_err();

    let info = mock_info("creator", &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        ExecuteMsg::ToggleLock { lock: true },
    )
    .unwrap();
    assert!(CONTRACT_INFO.load(&deps.storage).unwrap().lock);

    execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::ToggleLock { lock: false },
    )
    .unwrap();
    assert!(!CONTRACT_INFO.load(&deps.storage).unwrap().lock);
}

#[test]
fn test_change_parameter() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());

    let info = mock_info("bad_person", &[]);
    let env = mock_env();
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ChangeParameter {
            parameter: "any".to_string(),
            value: "any".to_string(),
        },
    )
    .unwrap_err();

    let info = mock_info("creator", &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        ExecuteMsg::ChangeParameter {
            parameter: "any".to_string(),
            value: "any".to_string(),
        },
    )
    .unwrap_err();

    execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::ChangeParameter {
            parameter: "fee_addr".to_string(),
            value: "any".to_string(),
        },
    )
    .unwrap();

    assert_eq!(
        CONTRACT_INFO
            .load(&deps.storage)
            .unwrap()
            .fee_addr
            .to_string(),
        "any"
    );
}

// Query tests

#[test]
fn test_query_contract_info() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());
    let env = mock_env();
    let response = query(deps.as_ref(), env, QueryMsg::ContractInfo {}).unwrap();
    assert_eq!(
        from_binary::<ContractInfo>(&response).unwrap(),
        ContractInfo {
            name: "nft-raffle".to_string(),
            owner: OwnerStruct::new(deps.api.addr_validate("creator").unwrap()),
            fee_addr: deps.api.addr_validate("creator").unwrap(),
            last_raffle_id: None,
            minimum_raffle_duration: 1u64,
            minimum_raffle_timeout: 120u64,
            raffle_fee: Decimal::from_str("0.0002").unwrap(),
            rand_fee: Decimal::from_str("0.0001").unwrap(),
            lock: false,
            drand_url: "https://api.drand.sh/".to_string(),
            verify_signature_contract: deps.api.addr_validate("verifier").unwrap(),
            random_pubkey: Binary::from_base64(
                &HEX_PUBKEY.from_hex().unwrap().to_base64(base64::STANDARD)
            )
            .unwrap()
        }
    );
}

#[test]
fn test_query_raffle_info() {
    let mut deps = mock_querier_dependencies(&[]);
    deps.querier
        .with_owner_of(&[
            (&"nft - token_id".to_string(), &"creator".to_string())
        ]);
        
    init_helper(deps.as_mut());

    create_raffle(deps.as_mut()).unwrap();
    create_raffle_comment(deps.as_mut(), "random things my dude").unwrap();

    let env = mock_env();
    let response = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::RaffleInfo { raffle_id: 1 },
    )
    .unwrap();

    assert_eq!(
        from_binary::<RaffleResponse>(&response).unwrap(),
        RaffleResponse {
            raffle_id: 1,
            raffle_state: RaffleState::Started,
            raffle_info: Some(RaffleInfo {
                owner: deps.api.addr_validate("creator").unwrap(),
                assets: vec![AssetInfo::cw721("nft", "token_id")],
                raffle_options: RaffleOptions {
                    raffle_start_timestamp: env.block.time,
                    raffle_duration: 1u64,
                    raffle_timeout: 120u64,
                    comment: Some("random things my dude".to_string()),
                    max_participant_number: None,
                    max_ticket_per_address: None,
                    raffle_preview: 0
                },
                raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
                number_of_tickets: 0u32,
                randomness: None,
                winner: None,
                is_cancelled: false,
            })
        }
    );
}

#[test]
fn test_query_all_raffle_info() {
    let mut deps = mock_querier_dependencies(&[]);
    deps.querier
        .with_owner_of(&[
            (&"nft - token_id".to_string(), &"creator".to_string())
        ]);
        
    init_helper(deps.as_mut());

    create_raffle(deps.as_mut()).unwrap();
    create_raffle_comment(deps.as_mut(), "random things my dude").unwrap();

    let env = mock_env();
    // Testing the general function
    let response = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::AllRaffles {
            start_after: None,
            limit: None,
            filters: None,
        },
    )
    .unwrap();

    assert_eq!(
        from_binary::<AllRafflesResponse>(&response)
            .unwrap()
            .raffles,
        vec![
            RaffleResponse {
                raffle_id: 1,
                raffle_state: RaffleState::Started,
                raffle_info: Some(RaffleInfo {
                    owner: deps.api.addr_validate("creator").unwrap(),
                    assets: vec![AssetInfo::cw721("nft", "token_id")],
                    raffle_options: RaffleOptions {
                        raffle_start_timestamp: env.block.time,
                        raffle_duration: 1u64,
                        raffle_timeout: 120u64,
                        comment: Some("random things my dude".to_string()),
                        max_participant_number: None,
                        max_ticket_per_address: None,
                        raffle_preview: 0
                    },
                    raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
                    number_of_tickets: 0u32,
                    randomness: None,
                    winner: None,
                    is_cancelled: false,
                })
            },
            RaffleResponse {
                raffle_id: 0,
                raffle_state: RaffleState::Started,
                raffle_info: Some(RaffleInfo {
                    owner: deps.api.addr_validate("creator").unwrap(),
                    assets: vec![AssetInfo::cw721("nft", "token_id")],
                    raffle_options: RaffleOptions {
                        raffle_start_timestamp: env.block.time,
                        raffle_duration: 1u64,
                        raffle_timeout: 120u64,
                        comment: None,
                        max_participant_number: None,
                        max_ticket_per_address: None,
                        raffle_preview: 0
                    },
                    raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
                    number_of_tickets: 0u32,
                    randomness: None,
                    winner: None,
                    is_cancelled: false,
                })
            }
        ]
    );

    // Testing the limit parameter
    let response = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::AllRaffles {
            start_after: None,
            limit: Some(1u32),
            filters: None,
        },
    )
    .unwrap();

    assert_eq!(
        from_binary::<AllRafflesResponse>(&response)
            .unwrap()
            .raffles,
        vec![RaffleResponse {
            raffle_id: 1,
            raffle_state: RaffleState::Started,
            raffle_info: Some(RaffleInfo {
                owner: deps.api.addr_validate("creator").unwrap(),
                assets: vec![AssetInfo::cw721("nft", "token_id")],
                raffle_options: RaffleOptions {
                    raffle_start_timestamp: env.block.time,
                    raffle_duration: 1u64,
                    raffle_timeout: 120u64,
                    comment: Some("random things my dude".to_string()),
                    max_participant_number: None,
                    max_ticket_per_address: None,
                    raffle_preview: 0
                },
                raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
                number_of_tickets: 0u32,
                randomness: None,
                winner: None,
                is_cancelled: false,
            })
        }]
    );

    // Testing the start_after parameter
    let response = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::AllRaffles {
            start_after: Some(1u64),
            limit: None,
            filters: None,
        },
    )
    .unwrap();

    assert_eq!(
        from_binary::<AllRafflesResponse>(&response)
            .unwrap()
            .raffles,
        vec![RaffleResponse {
            raffle_id: 0,
            raffle_state: RaffleState::Started,
            raffle_info: Some(RaffleInfo {
                owner: deps.api.addr_validate("creator").unwrap(),
                assets: vec![AssetInfo::cw721("nft", "token_id")],
                raffle_options: RaffleOptions {
                    raffle_start_timestamp: env.block.time,
                    raffle_duration: 1u64,
                    raffle_timeout: 120u64,
                    comment: None,
                    max_participant_number: None,
                    max_ticket_per_address: None,
                    raffle_preview: 0
                },
                raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
                number_of_tickets: 0u32,
                randomness: None,
                winner: None,
                is_cancelled: false,
            })
        }]
    );

    // Testing the filter parameter
    buy_ticket_coin(deps.as_mut(), 1, "actor", coin(10000, "uluna"), 0u64, None).unwrap();
    let response = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::AllRaffles {
            start_after: None,
            limit: None,
            filters: Some(QueryFilters {
                states: None,
                owner: None,
                ticket_depositor: Some("actor".to_string()),
                contains_token: None,
            }),
        },
    )
    .unwrap();
    let raffles = from_binary::<AllRafflesResponse>(&response)
        .unwrap()
        .raffles;
    assert_eq!(raffles.len(), 1);
    assert_eq!(raffles[0].raffle_id, 1);

    buy_ticket_coin(deps.as_mut(), 0, "actor", coin(10000, "uluna"), 0u64, None).unwrap();
    buy_ticket_coin(deps.as_mut(), 0, "actor1", coin(10000, "uluna"), 0u64, None).unwrap();
    let response = query(
        deps.as_ref(),
        env,
        QueryMsg::AllRaffles {
            start_after: None,
            limit: None,
            filters: Some(QueryFilters {
                states: None,
                owner: None,
                ticket_depositor: Some("actor".to_string()),
                contains_token: None,
            }),
        },
    )
    .unwrap();
    let raffles = from_binary::<AllRafflesResponse>(&response)
        .unwrap()
        .raffles;
    assert_eq!(raffles.len(), 2);
}

#[test]
fn test_multiple_tickets() {
    let mut deps = mock_querier_dependencies(&[]);
    deps.querier
        .with_owner_of(&[
            (&"nft - token_id".to_string(), &"creator".to_string())
        ]);
        
    init_helper(deps.as_mut());
    create_raffle(deps.as_mut()).unwrap();

    //Buy some tickets
    buy_ticket_coin(
        deps.as_mut(),
        0,
        "first",
        coin(30000, "uluna"),
        0u64,
        Some(3),
    )
    .unwrap();

    // Update the randomness internally
    let mut raffle_info = RAFFLE_INFO.load(&deps.storage, 0).unwrap();

    let mut randomness: [u8; 32] = [0; 32];
    hex::decode_to_slice(
        "89580f6a639add6c90dcf3d222e35415f89d9ee2cd6ef6fc4f23134cdffa5d1e",
        randomness.as_mut_slice(),
    )
    .unwrap();
    raffle_info.randomness = Some(Randomness {
        randomness,
        randomness_round: 2098475u64,
        randomness_owner: deps.api.addr_validate("rand_provider").unwrap(),
    });
    RAFFLE_INFO
        .save(deps.as_mut().storage, 0, &raffle_info)
        .unwrap();

    let response = claim_nft(deps.as_mut(), 0, 1000u64).unwrap();

    assert_eq!(
        response.messages,
        vec![
            SubMsg::new(
                into_cosmos_msg(
                    Cw721ExecuteMsg::TransferNft {
                        recipient: "first".to_string(),
                        token_id: "token_id".to_string()
                    },
                    "nft".to_string()
                )
                .unwrap()
            ),
            SubMsg::new(BankMsg::Send {
                to_address: "rand_provider".to_string(),
                amount: coins(3, "uluna")
            }),
            SubMsg::new(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: coins(6, "uluna")
            }),
            SubMsg::new(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: coins(29991u128, "uluna")
            }),
        ]
    );
}
