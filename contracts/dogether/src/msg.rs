use cosmwasm_std::{Binary, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub code_id_cw20: u64,
    pub message_cw20: Binary,
    pub label_cw20: String,
    pub code_id_staking: u64,
    pub message_staking: Binary,
    pub label_staking: String,
    pub money_market_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Pool UST to anchor yield in order to get automated ticket with earning
    Pool {},
    /// UnPool tokens and get back UST, un-bonding period required!
    UnPool { amount: Uint128 },
    /// Withdraw unPool tokens after un-bonding period succeed
    ClaimUnPool {},
    /// Redeem earning from anchor
    RedeemEarning {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Not used to be called directly

}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    /// Deposit stable to anchor
    DepositStable {},
    /// Return stable coins to a user
    /// according to exchange rate
    RedeemStable {},
    /// Query Epoch from anchor
    EpochState {
        block_height: Option<u64>,
        distributed_interest: Option<Uint256>,
    },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CountResponse {
    pub count: i32,
}
// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct EpochStateResponse {
    pub exchange_rate: Decimal256,
    pub aterra_supply: Uint256,
}
