use crate::contract::query;
use cosmwasm_std::from_json;
use nft_loans_export::msg::CollateralResponse;
use nft_loans_export::msg::MultipleCollateralsResponse;
use nft_loans_export::msg::QueryMsg;

use cosmwasm_std::testing::{mock_dependencies, mock_env};

use nft_loans_export::state::CollateralInfo;

use utils::state::AssetInfo;

use crate::testing::tests::{add_collateral_helper, init_helper};
use utils::state::Cw721Coin;

#[test]
fn test_query_collaterals() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());

    let borrower = "anyone";

    // We deposit multiple collaterals
    add_collateral_helper(deps.as_mut(), borrower, "nft", "token", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft", "token1", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft", "token2", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft", "token3", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft1", "token1", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft1", "token2", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft1", "token3", None, None).unwrap();
    // We query them

    let collaterals: MultipleCollateralsResponse = from_json(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Collaterals {
                borrower: borrower.to_string(),
                limit: None,
                start_after: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        collaterals,
        MultipleCollateralsResponse {
            collaterals: vec![
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 6,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft1".to_string(),
                            token_id: "token3".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 5,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft1".to_string(),
                            token_id: "token2".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 4,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft1".to_string(),
                            token_id: "token1".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 3,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft".to_string(),
                            token_id: "token3".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 2,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft".to_string(),
                            token_id: "token2".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 1,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft".to_string(),
                            token_id: "token1".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 0,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft".to_string(),
                            token_id: "token".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                }
            ],
            next_collateral: None
        }
    );
}

#[test]
fn test_query_and_limit_collaterals() {
    let mut deps = mock_dependencies();
    init_helper(deps.as_mut());

    let borrower = "anyone";

    // We deposit multiple collaterals
    add_collateral_helper(deps.as_mut(), borrower, "nft", "token", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft", "token1", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft", "token2", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft", "token3", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft1", "token1", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft1", "token2", None, None).unwrap();
    add_collateral_helper(deps.as_mut(), borrower, "nft1", "token3", None, None).unwrap();
    // We query them

    let collaterals: MultipleCollateralsResponse = from_json(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Collaterals {
                borrower: borrower.to_string(),
                limit: Some(2),
                start_after: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        collaterals,
        MultipleCollateralsResponse {
            collaterals: vec![
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 6,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft1".to_string(),
                            token_id: "token3".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 5,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft1".to_string(),
                            token_id: "token2".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
            ],
            next_collateral: Some(5)
        }
    );

    let collaterals: MultipleCollateralsResponse = from_json(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Collaterals {
                borrower: borrower.to_string(),
                limit: Some(3),
                start_after: Some(5),
            },
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        collaterals,
        MultipleCollateralsResponse {
            collaterals: vec![
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 4,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft1".to_string(),
                            token_id: "token1".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 3,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft".to_string(),
                            token_id: "token3".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 2,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft".to_string(),
                            token_id: "token2".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
            ],
            next_collateral: Some(2)
        }
    );

    let collaterals: MultipleCollateralsResponse = from_json(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Collaterals {
                borrower: borrower.to_string(),
                limit: Some(10),
                start_after: Some(2),
            },
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        collaterals,
        MultipleCollateralsResponse {
            collaterals: vec![
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 1,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft".to_string(),
                            token_id: "token1".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                },
                CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id: 0,
                    collateral: CollateralInfo {
                        associated_assets: vec![AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft".to_string(),
                            token_id: "token".to_string()
                        })],
                        list_date: mock_env().block.time,
                        ..CollateralInfo::default()
                    }
                }
            ],
            next_collateral: None
        }
    );
}
