//! Route filtering for atomic arbitrage based on affected swap resources.
//!
//! This module determines optimal route search conditions based on which swap
//! resources have been affected by a transaction. The filtering strategy varies
//! based on the number of affected swaps:
//!
//! - Single swap: Uses `single_affect` module
//! - Two swaps: Uses `two_affect` module with special pattern detection
//! - Multiple swaps: Uses `multiple_affect` module

mod multiple_affect;
mod single_affect;
mod two_affect;

use crate::coins::*;
use crate::swap::{RoutePairFilter, SwapContract, UpdatePairType};

use multiple_affect::*;
use single_affect::*;
use two_affect::*;

/// Conditions for searching arbitrage routes based on affected resources.
///
/// This struct defines filters for a 3-step swap route:
/// - Step 1: Initial swap from APT to intermediate coin
/// - Step 2: Middle swap between intermediate coins
/// - Step 3: Final swap back to APT
pub struct SearchRouteCondition {
    /// Swap contract filters for each step (first, second, third).
    /// If a step's filter is Some, only that swap contract will be considered.
    pub swap_filters_for_steps: Option<(
        Option<SwapContract>,
        Option<SwapContract>,
        Option<SwapContract>,
    )>,

    /// Coin pair filters for each step (first, second, third).
    /// Used to constrain which coin pairs are considered at each step.
    pub coin_filters_for_steps: (
        Vec<RoutePairFilter>,
        Vec<RoutePairFilter>,
        Vec<RoutePairFilter>,
    ),

    /// Input amounts to try for the arbitrage (in octas).
    /// Multiple amounts allow testing different trade sizes.
    pub input_amounts: Vec<u64>,
}

/// Determines route filters based on affected swap resources.
///
/// Dispatches to the appropriate handler based on the number of affected swaps:
/// - 1 swap: Single resource handler
/// - 2 swaps: Two resource handler (with special pattern detection)
/// - 3+ swaps: Multiple resource handler
pub fn route_filter_by_affected_resource(
    affected_swap_updates: &Vec<(SwapContract, UpdatePairType)>,
) -> SearchRouteCondition {
    match affected_swap_updates.len() {
        1 => route_filter_by_affected_resource_single(
            affected_swap_updates.first().unwrap().clone(),
        ),
        2 => route_filter_by_affected_resource_two(affected_swap_updates),
        _ => route_filter_by_affected_multiple_resource(affected_swap_updates),
    }
}

/// Checks if any Thala-related resources might be affected.
///
/// Thala has complex multi-pool dynamics that require special handling,
/// so when Thala is involved, we typically disable filtering to ensure
/// we don't miss opportunities.
///
/// Returns true if:
/// - The Thala swap contract is directly affected
/// - Any Thala-related coin (THL, MOD, ThalaAPT, AmnisApt) is in an affected pair
pub fn thala_resource_might_affected(
    affected_swap_updates: &Vec<(SwapContract, UpdatePairType)>,
) -> bool {
    let thala_coins = [
        thala_coin_id(),
        thala_mod_coin_id(),
        thala_thapt_coin_id(),
        thala_amapt_coin_id(),
    ];

    for (swap, pair_type) in affected_swap_updates {
        // Direct Thala swap involvement
        if swap == &SwapContract::Thala {
            return true;
        }

        // Check if any Thala coin is in the pair
        let (coin_x, coin_y) = match pair_type {
            UpdatePairType::XIncreaseYDecrease(x, y)
            | UpdatePairType::XDecreaseYIncrease(x, y) => (x, y),
            _ => continue,
        };

        if thala_coins.contains(coin_x) || thala_coins.contains(coin_y) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usdc_coin_id() -> CoinId {
        CoinId::from_addresses_str(
            "0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa",
            "asset",
            "USDC",
        )
    }

    fn usdt_coin_id() -> CoinId {
        CoinId::from_addresses_str(
            "0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa",
            "asset",
            "USDT",
        )
    }

    fn weth_coin_id() -> CoinId {
        CoinId::from_addresses_str(
            "0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa",
            "asset",
            "WETH",
        )
    }

    #[test]
    fn test_thala_resource_might_affected_with_thala_contract() {
        let updates = vec![(
            SwapContract::Thala,
            UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
        )];
        assert!(thala_resource_might_affected(&updates));
    }

    #[test]
    fn test_thala_resource_might_affected_with_thala_coin_x_increase_y_decrease() {
        let updates = vec![(
            SwapContract::Liquid,
            UpdatePairType::XIncreaseYDecrease(thala_coin_id(), apt_coin_id()),
        )];
        assert!(thala_resource_might_affected(&updates));

        let updates = vec![(
            SwapContract::Liquid,
            UpdatePairType::XIncreaseYDecrease(apt_coin_id(), thala_mod_coin_id()),
        )];
        assert!(thala_resource_might_affected(&updates));
    }

    #[test]
    fn test_thala_resource_might_affected_with_thala_coin_x_decrease_y_increase() {
        let updates = vec![(
            SwapContract::Liquid,
            UpdatePairType::XDecreaseYIncrease(thala_thapt_coin_id(), apt_coin_id()),
        )];
        assert!(thala_resource_might_affected(&updates));

        let updates = vec![(
            SwapContract::Liquid,
            UpdatePairType::XDecreaseYIncrease(apt_coin_id(), thala_amapt_coin_id()),
        )];
        assert!(thala_resource_might_affected(&updates));
    }

    #[test]
    fn test_thala_resource_might_affected_false_for_non_thala() {
        let updates = vec![(
            SwapContract::Liquid,
            UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
        )];
        assert!(!thala_resource_might_affected(&updates));

        let updates = vec![(
            SwapContract::Pancake,
            UpdatePairType::XDecreaseYIncrease(usdc_coin_id(), usdt_coin_id()),
        )];
        assert!(!thala_resource_might_affected(&updates));
    }

    #[test]
    fn test_thala_resource_might_affected_ignores_other_update_types() {
        let updates = vec![(
            SwapContract::Liquid,
            UpdatePairType::Other(thala_coin_id(), apt_coin_id()),
        )];
        assert!(!thala_resource_might_affected(&updates));

        let updates = vec![(
            SwapContract::Liquid,
            UpdatePairType::NewPair(thala_coin_id(), apt_coin_id()),
        )];
        assert!(!thala_resource_might_affected(&updates));
    }

    #[test]
    fn test_route_filter_by_affected_resource_dispatches_single() {
        let updates = vec![(
            SwapContract::Liquid,
            UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
        )];
        let result = route_filter_by_affected_resource(&updates);
        assert!(result.swap_filters_for_steps.is_some());
    }

    #[test]
    fn test_route_filter_by_affected_resource_dispatches_two() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XDecreaseYIncrease(apt_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource(&updates);
        assert!(result.input_amounts.len() >= 2);
    }

    #[test]
    fn test_route_filter_by_affected_resource_dispatches_multiple() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XDecreaseYIncrease(apt_coin_id(), usdt_coin_id()),
            ),
            (
                SwapContract::Cetus,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdc_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource(&updates);
        assert!(result.input_amounts.len() >= 2);
    }
}
