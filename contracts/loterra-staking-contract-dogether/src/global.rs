use crate::state::{CONFIG, STATE};

use crate::math::decimal_summation_in_256;
use cosmwasm_std::{attr, Decimal, DepsMut, Env, Response, StdError, StdResult};

/// Increase global_index according to claimed rewards amount
/// Only hub_contract is allowed to execute
pub fn handle_update_global_index(deps: DepsMut, env: Env) -> StdResult<Response> {
    let mut state = STATE.load(deps.storage)?;
    // anybody can trigger update_global_index
    /*
    if config.lottery_contract != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }
     */

    // Zero staking balance check
    if state.total_balance.is_zero() {
        return Err(StdError::generic_err("No asset is bonded by Hub"));
    }

    let reward_denom = CONFIG.load(deps.storage)?.reward_denom;

    // Load the reward contract balance
    let balance = deps
        .querier
        .query_balance(env.contract.address, reward_denom.as_str())
        .unwrap();

    let previous_balance = state.prev_reward_balance;

    // claimed_rewards = current_balance - prev_balance;
    let claimed_rewards = balance.amount.checked_sub(previous_balance)?;

    state.prev_reward_balance = balance.amount;

    // global_index += claimed_rewards / total_balance;
    state.global_index = decimal_summation_in_256(
        state.global_index,
        Decimal::from_ratio(claimed_rewards, state.total_balance),
    );

    STATE.save(deps.storage, &state)?;

    let res = Response {
        submessages: vec![],
        messages: vec![],
        data: None,
        attributes: vec![
            attr("action", "update_global_index"),
            attr("claimed_rewards", claimed_rewards),
        ],
    };

    Ok(res)
}
