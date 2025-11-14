use arcis_imports::*;

#[encrypted]
mod circuits {
    use arcis_imports::*;

    pub struct BetInput {
        predicted_price: u64,
    }

    #[instruction]
    pub fn process_bet(input_ctxt: Enc<Shared, BetInput>) -> Enc<Shared, bool> {
        let input = input_ctxt.to_arcis();
        let is_valid = input.predicted_price > 0;
        input_ctxt.owner.from_arcis(is_valid)
    }

    #[instruction]
    pub fn calculate_reward_v2(
        encrypted_price_ctxt: Enc<Shared, u64>,
        actual_price: u64,
        total_pool_amount: u64,
        protocol_fee_bps: u16,
    ) -> Enc<Shared, u64> {
        let predicted = encrypted_price_ctxt.to_arcis() as i128;
        let actual = actual_price as i128;

        let diff = if predicted > actual {
            (predicted - actual) as u64
        } else {
            (actual - predicted) as u64
        };

        let accuracy_bps = if diff >= actual_price {
            0u64
        } else {
            let ratio = (diff * 10000) / actual_price;
            10000 - ratio
        };

        let protocol_fee =
            (total_pool_amount as u128 * protocol_fee_bps as u128) / 10000;
        let distributable = (total_pool_amount as u128) - protocol_fee;

        let reward_amount = if accuracy_bps == 0 {
            0u64
        } else {
            let reward = (distributable * accuracy_bps as u128) / 10000;
            reward as u64
        };

        let packed = (reward_amount as u128 * 4_294_967_296u128) + (accuracy_bps as u128);
        let packed_u64 = packed as u64;
        encrypted_price_ctxt.owner.from_arcis(packed_u64)
    }
}