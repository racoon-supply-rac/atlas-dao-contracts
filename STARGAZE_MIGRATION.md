## Analysis: Migrating `nft-loans-non-custodial` smart-contract to Stargaze

**Contract to migrate**: `https://github.com/Illiquidly/illiquidlabs-contracts/tree/master/contracts/nft-loans-non-custodial`

### Introduction
This document showcases the key points to migrate the contract and make it work with Stargaze's SDK.

_This analysis has been conducted considering Stargaze's SDK v12.0.0._

### Elements Excluded from Our Analysis
We assumed that the contract deployed on Stargaze will **not** be compatible with `cw1155` and `cw20`. In other words, collaterals will be of types `cw721` & `sg721`. Additionally, only native assets like $STARS and IBC assets will be accepted for loans.

### Rust Packages
Several packages within the `nft-loans-non-custodial` contract were outdated and need to be updated. Below are our propositions for the packages:

#### Package Removals
The packages `strum` & `strum-macro` are not necessary and we propose to remove them. They were mainly used for `traits` which can be replaced by `cw_serde` trait.

#### Package Transitions
As of version 12.0.0, Stargaze supports `cosmwasm_1_3` but will support `cosmwasm_1_5` in v12.1.0. We, therefore, propose the following package upgrades:

* `cw2` = From "0.13.0" -> To "1.1.0"
* `cw721` = From "0.13.0" -> To "0.18.0"
* `cosmwasm-schema` = From "1.1.0" -> To "1.2.1"
* `cw-storage-plus` = "0.13.0" -> To "1.1.0"
* `cosmwasm-std` = From "1.1.0" -> To "1.1.2"
* `schemars` = From "0.8.1" -> To "0.8.12"
* `serde` = From "1.0.103"-> To -> "1.0.145"
* `thiserror` = From "1.0.23" -> To "1.0.43"
* `anyhow` = From "1.0" -> To "1.0.71"

The above are compatible with Stargaze.

#### Package Additions
Stargaze's SDK is slightly different when it comes to specific elements like messages and tests. Therefore, the following packages should be added to be compatible with Stargaze's SDK:

_This list is preliminary and non-exclusive. Other packages can be added later on._

* `sg721` -> "3.3.0"
* `sg721-base` -> "3.3.0"
* `sg-std` -> "3.2.0"
* `cw-multi-test` -> "^0.16.2"
* `sg-multi-test` -> "3.1.0"

### Code: Logic
* `cw20` and `cw1155` need to be excluded, and the code's logic needs to be adjusted consequently.
* Need to confirm if we want to add that a borrower != lender when a lender accepts an offer.
* When a loan is accepted, the timer is triggered automatically. A notification to the borrower could make it more fair or a cooldown period could help.
* When the borrowed funds are repaid, there could be more validation regarding the amount.

### Code: Parts to Be Modified
* Error handling logic needs to be refactored:
  * Relies too much on `anyhow` package and we do not want that.
  * `bail` has some issues when returning a `Result<Response, ContractError>`. This can be changed by using `ensure` from Cosmwasm.
  * All the `anyhow` usage needs to be removed and replaced with the appropriate error handling
  * Several helper functions suffer from this problem and they all need to be changed.
* All the `Result` outcomes need to be changed to fit Stargaze's SDK.
  * `Result<Response>` -> `Result<Response, ContractError>`
* States need to be updated
  * MultiIndex and IndexedMap works differently in the most recent packages version
  * `.update` method on the states need to be modified
  * Other state management needs to be addressed
* NFT interactions need to be migrated to potential `sg721`'s methodology.
* All the tests need to be refactored to fit Stargaze's SDK
  * They use an old methodology

### Conclusion
In conclusion, the packages update & upgrade to fit Stargaze's SDK will require a medium effort as we do not expect any significant breaking changes when updating and adding packages.

When it comes to the current logic of the code, there are some changes to add regarding the `cw20` and `cw1155`, but it is not expected to develop into major issues. As for the other elements, it will be up to AtlasDAO to see if they want to add some logic to improve the security/logic of the contract.

Finally, most of the work will be related to contract testing, blockchain messages, error handling, and use Stargaze's SDK packages.

This migration has a low to medium difficulty level, and we do not expect any major issues to arise.

### Estimated Work to Execute the Migration
Depending on what exactly AtlasDAO wants as support regarding the contract, the price of the migration might vary.

1. Migrating the contract, some calls to discuss potential changes but without any additional support: 2750 USD
2. Same as 1. with additional support to deploy the contract, explain how it works and make testnet testing with the interface: 3500 USD (max 10h support)
3. Any new features, changes to the code would be billed separately.

In any case, we strongly recommend an independent audit of the new contract. We want to emphasize that we, as developers, are not responsible for ANY potential loss of funds, issues, unexpected problems, or any other adverse outcomes. The smart contract is provided "as is" without any guarantee, and all usage is at the user's own risk.