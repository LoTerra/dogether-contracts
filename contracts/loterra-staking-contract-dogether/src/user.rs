use crate::state::{
    read_holder, read_holders, store_holder, Config, Holder, State, CONFIG, PREFIXED_COMBINATIONS,
    STATE,
};

use cosmwasm_std::{
    from_binary, to_binary, Coin, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128, WasmMsg, WasmQuery,
};

use crate::claim::{claim_tokens, create_claim};
use crate::math::{
    decimal_multiplication_in_256, decimal_subtraction_in_256, decimal_summation_in_256,
};
use crate::msg::{AccruedRewardsResponse, HolderResponse, HoldersResponse, ReceiveMsg};
use crate::taxation::deduct_tax;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, Expiration};
use loterra;

pub fn handle_get_ticket(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    recipient: String,
    combination: Vec<String>,
) -> StdResult<Response> {
    if combination.is_empty() {
        return Err(StdError::generic_err("No combination found"));
    }
    let holder_addr_raw = deps.api.addr_canonicalize(&recipient.as_str())?;

    let mut holder: Holder = read_holder(deps.storage, &holder_addr_raw)?;
    let mut state: State = STATE.load(deps.storage)?;
    let config: Config = CONFIG.load(deps.storage)?;

    let reward_with_decimals =
        calculate_decimal_rewards(state.global_index, holder.index, holder.balance)?;

    let all_reward_with_decimals =
        decimal_summation_in_256(reward_with_decimals, holder.pending_rewards);

    // let decimals = get_decimals(all_reward_with_decimals).unwrap();

    let rewards = all_reward_with_decimals * Uint128::from(1_u128);

    if rewards.is_zero() {
        return Err(StdError::generic_err("No rewards have accrued yet"));
    }

    /*
       Query price per tickets on lottery contract
       Multiply price per ticket and combination.len()
       Check if balance is > combination wanted
       Loop combination and check if exist if yes return error if not save to PREFIXED_COMBINATIONS
    */
    /*
       Query the price per tickets
    */
    let query = loterra::msg::QueryMsg::Config {};
    let msg_query = WasmQuery::Smart {
        contract_addr: deps.api.addr_humanize(&config.loterra_addr)?.to_string(),
        msg: to_binary(&query)?,
    };
    let query_loterra: loterra::msg::ConfigResponse = deps.querier.query(&msg_query.into())?;
    let price_per_ticket = query_loterra.price_per_ticket_to_register;
    // Total ticket cost
    let total_ticket_cost = Uint128::from(price_per_ticket.u128() * combination.len() as u128);
    // Total ticket cost minus fees
    let total_ticket_cost_net = deduct_tax(
        &deps.querier,
        Coin {
            denom: config.reward_denom.clone(),
            amount: total_ticket_cost,
        },
    )?
    .amount;
    // Total network fees
    let total_fee = total_ticket_cost
        .checked_sub(total_ticket_cost_net)
        .unwrap();
    // Total ticket cost + fees summation
    let total_ticket_with_fees = total_ticket_cost.checked_add(total_fee).unwrap();

    // Check if enough rewards to buy tickets
    if rewards < total_ticket_with_fees {
        return Err(StdError::generic_err(format!(
            "Not enough funds, you want to buy {}UST + {}UST network fees tickets and you only have {}UST",
            total_ticket_cost, total_fee, rewards
        )));
    }

    /*
       Check if it is the more efficient way to check combination exist
    */
    for combo in combination.clone() {
        match PREFIXED_COMBINATIONS.may_load(
            deps.storage,
            (
                &query_loterra.lottery_counter.to_be_bytes(),
                &deps.api.addr_canonicalize(&recipient.as_str())?.as_slice(),
                &combo.as_bytes(),
            ),
        )? {
            None => {
                PREFIXED_COMBINATIONS.save(
                    deps.storage,
                    (
                        &query_loterra.lottery_counter.to_be_bytes(),
                        &deps.api.addr_canonicalize(&recipient.as_str())?.as_slice(),
                        &combo.as_bytes(),
                    ),
                    &combo,
                )?;
            }
            Some(_) => {
                return Err(StdError::generic_err(format!(
                    "Combination {} already exist",
                    combo
                )));
            }
        }
    }

    let new_balance = (state
        .prev_reward_balance
        .checked_sub(total_ticket_with_fees))?;

    state.prev_reward_balance = new_balance;
    STATE.save(deps.storage, &state)?;

    /*
       New calculation pending rewards adding new decimals
    */
    holder.pending_rewards = Decimal::from_ratio(
        rewards.checked_sub(total_ticket_with_fees).unwrap(),
        Uint128::from(1_u128),
    );
    holder.index = state.global_index;
    store_holder(deps.storage, &holder_addr_raw, &holder)?;

    let msg_loterra = loterra::msg::ExecuteMsg::Register {
        address: Some(deps.api.addr_validate(&recipient.clone())?),
        altered_bonus: None,
        combination: combination.clone(),
    };
    let execute = WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&config.loterra_addr)?.to_string(),
        msg: to_binary(&msg_loterra)?,
        funds: vec![deduct_tax(
            &deps.querier,
            Coin {
                denom: config.reward_denom,
                amount: total_ticket_with_fees,
            },
        )?],
    };

    let res = Response::new()
        .add_message(execute)
        .add_attribute("action", "get_ticket")
        .add_attribute("player_address", recipient)
        .add_attribute("ticket_number", combination.len().to_string());
    Ok(res)
}

pub fn handle_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    wrapper: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // only loterra cw20 contract can send receieve msg
    if info.sender != deps.api.addr_humanize(&config.cw20_token_addr)? {
        return Err(StdError::generic_err(
            "only loterra contract can send receive messages",
        ));
    }

    let msg: ReceiveMsg = from_binary(&wrapper.msg)?;
    match msg {
        ReceiveMsg::BondStake {} => handle_bond(deps, env, info, wrapper.sender, wrapper.amount),
    }
}

pub fn handle_bond(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    holder_addr: String,
    amount: Uint128,
) -> StdResult<Response> {
    if !info.funds.is_empty() {
        return Err(StdError::generic_err("Do not send funds with stake"));
    }
    if amount.is_zero() {
        return Err(StdError::generic_err("Amount required"));
    }

    let address_raw = deps.api.addr_canonicalize(&holder_addr.as_str())?;

    let mut state: State = STATE.load(deps.storage)?;
    let mut holder: Holder = read_holder(deps.storage, &address_raw)?;

    // get decimals
    let rewards = calculate_decimal_rewards(state.global_index, holder.index, holder.balance)?;

    holder.index = state.global_index;
    holder.pending_rewards = decimal_summation_in_256(rewards, holder.pending_rewards);
    holder.balance += amount;
    state.total_balance += amount;

    store_holder(deps.storage, &address_raw, &holder)?;
    STATE.save(deps.storage, &state)?;

    let res = Response::new()
        .add_attribute("action", "bond_stake")
        .add_attribute("holder_address", holder_addr)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn handle_unbound(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    address: String,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    if config.admin != deps.api.addr_canonicalize(&info.sender.as_str())? {
        return Err(StdError::generic_err("Not authorized"));
    }

    let address_raw = deps.api.addr_canonicalize(&address)?;

    if !info.funds.is_empty() {
        return Err(StdError::generic_err("Do not send funds with stake"));
    }
    if amount.is_zero() {
        return Err(StdError::generic_err("Amount required"));
    }

    let mut state: State = STATE.load(deps.storage)?;
    let mut holder: Holder = read_holder(deps.storage, &address_raw)?;
    if holder.balance < amount {
        return Err(StdError::generic_err(format!(
            "Decrease amount cannot exceed user balance: {}",
            holder.balance
        )));
    }

    let rewards = calculate_decimal_rewards(state.global_index, holder.index, holder.balance)?;

    holder.index = state.global_index;
    holder.pending_rewards = decimal_summation_in_256(rewards, holder.pending_rewards);
    holder.balance = holder.balance.checked_sub(amount)?;
    state.total_balance = state.total_balance.checked_sub(amount)?;

    store_holder(deps.storage, &address_raw, &holder)?;
    STATE.save(deps.storage, &state)?;

    // create claim
    let release_height = Expiration::AtHeight(env.block.height + config.unbonding_period);
    create_claim(deps.storage, address_raw, amount, release_height)?;

    let res = Response::new()
        .add_attribute("action", "unbond_stake")
        .add_attribute("holder_address", info.sender)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn handle_withdraw_stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cap: Option<Uint128>,
    address: String,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    if config.admin != deps.api.addr_canonicalize(&info.sender.as_str())? {
        return Err(StdError::generic_err("Not authorized"));
    }

    let address_raw = deps.api.addr_canonicalize(&address.as_str())?;

    let amount = claim_tokens(deps.storage, address_raw, &env.block, cap)?;
    if amount.is_zero() {
        return Err(StdError::generic_err("Wait for the unbonding period"));
    }

    let cw20_human_addr = deps.api.addr_humanize(&config.cw20_token_addr)?;

    let cw20_burn_msg = Cw20ExecuteMsg::Burn { amount };
    let msg = WasmMsg::Execute {
        contract_addr: cw20_human_addr.to_string(),
        msg: to_binary(&cw20_burn_msg)?,
        funds: vec![],
    };

    let res = Response::new()
        .add_message(msg)
        .add_attribute("action", "withdraw_stake")
        .add_attribute("recipient", address)
        .add_attribute("withdraw", amount);
    Ok(res)
}

pub fn query_accrued_rewards(deps: Deps, address: String) -> StdResult<AccruedRewardsResponse> {
    let global_index = STATE.load(deps.storage)?.global_index;

    let holder: Holder = read_holder(deps.storage, &deps.api.addr_canonicalize(&address)?)?;
    let reward_with_decimals =
        calculate_decimal_rewards(global_index, holder.index, holder.balance)?;
    let all_reward_with_decimals =
        decimal_summation_in_256(reward_with_decimals, holder.pending_rewards);

    let rewards = all_reward_with_decimals * Uint128::from(1_u128);

    Ok(AccruedRewardsResponse { rewards })
}

pub fn query_holder(deps: Deps, address: String) -> StdResult<HolderResponse> {
    let holder: Holder = read_holder(deps.storage, &deps.api.addr_canonicalize(&address)?)?;
    Ok(HolderResponse {
        address,
        balance: holder.balance,
        index: holder.index,
        pending_rewards: holder.pending_rewards,
    })
}

pub fn query_holders(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<HoldersResponse> {
    let start_after = if let Some(start_after) = start_after {
        Some(deps.api.addr_validate(&start_after)?)
    } else {
        None
    };

    let holders: Vec<HolderResponse> = read_holders(deps, start_after, limit)?;

    Ok(HoldersResponse { holders })
}

// calculate the reward based on the sender's index and the global index.
fn calculate_decimal_rewards(
    global_index: Decimal,
    user_index: Decimal,
    user_balance: Uint128,
) -> StdResult<Decimal> {
    let decimal_balance = Decimal::from_ratio(user_balance, Uint128::from(1_u128));
    Ok(decimal_multiplication_in_256(
        decimal_subtraction_in_256(global_index, user_index),
        decimal_balance,
    ))
}

// calculate the reward with decimal
/*
fn get_decimals(value: Decimal) -> StdResult<Decimal> {
    let stringed: &str = &*value.to_string();
    let parts: &[&str] = &*stringed.split('.').collect::<Vec<&str>>();
    match parts.len() {
        1 => Ok(Decimal::zero()),
        2 => {
            let decimals = Decimal::from_str(&*("0.".to_owned() + parts[1]))?;
            Ok(decimals)
        }
        _ => Err(StdError::generic_err("Unexpected number of dots")),
    }
}*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn proper_calculate_rewards() {
        let global_index = Decimal::from_ratio(Uint128::from(9_u128), Uint128::from(100_u128));
        let user_index = Decimal::zero();
        let user_balance = Uint128::from(1000_u128);
        let reward = calculate_decimal_rewards(global_index, user_index, user_balance).unwrap();
        assert_eq!(reward.to_string(), "90");
    }

    /*#[test]
    pub fn proper_get_decimals() {
        let global_index = Decimal::from_ratio(Uint128(9999999), Uint128(100000000));
        let user_index = Decimal::zero();
        let user_balance = Uint128(10);
        let reward = get_decimals(
            calculate_decimal_rewards(global_index, user_index, user_balance).unwrap(),
        )
        .unwrap();
        assert_eq!(reward.to_string(), "0.9999999");
    }*/
}
