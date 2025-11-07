use arcis_imports::*;

#[encrypted]
mod circuits {
    use arcis_imports::*;

    pub struct BetInput {
        predicted_price: u64,
    }

    pub struct RewardInput {
        predicted_price: u64,
        actual_price: u64,
        total_pool_amount: u64,
        protocol_fee_bps: u16,
    }

    pub struct RewardOutput {
        reward_amount: u64,
        accuracy_bps: u64,
    }

    #[instruction]
    pub fn process_bet(input_ctxt: Enc<Shared, BetInput>) -> Enc<Shared, bool> {
        let input = input_ctxt.to_arcis();

        let is_valid = input.predicted_price > 0;

        input_ctxt.owner.from_arcis(is_valid)
    }

    #[instruction]
    pub fn calculate_reward(input_ctxt: Enc<Shared, RewardInput>) -> Enc<Shared, u64> {
        let input = input_ctxt.to_arcis();

        let predicted = input.predicted_price as i128;
        let actual = input.actual_price as i128;

        let diff = if predicted > actual {
            (predicted - actual) as u64
        } else {
            (actual - predicted) as u64
        };

        let accuracy_bps = if diff >= input.actual_price {
            0u64
        } else {
            let ratio = (diff * 10000) / input.actual_price;
            10000 - ratio
        };

        let protocol_fee =
            (input.total_pool_amount as u128 * input.protocol_fee_bps as u128) / 10000;
        let distributable = (input.total_pool_amount as u128) - protocol_fee;

        let reward_amount = if accuracy_bps == 0 {
            0u64
        } else {
            let reward = (distributable * accuracy_bps as u128) / 10000;
            reward as u64
        };

        let packed = (reward_amount as u128 * 4_294_967_296u128) + (accuracy_bps as u128);
        let packed_u64 = packed as u64;
        input_ctxt.owner.from_arcis(packed_u64)
    }
}
