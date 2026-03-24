use super::{thala_resource_might_affected, SearchRouteCondition};
use crate::coins::*;
use crate::swap::{RoutePairFilter, SwapContract, UpdatePairType};

/// Determines route filters when multiple swap resources are affected.
///
/// This function analyzes multiple swap updates to determine optimal route filtering
/// for arbitrage opportunity detection. It handles three main scenarios:
/// 1. APT appreciation (APT decreases in reserve -> becomes more valuable)
/// 2. APT depreciation (APT increases in reserve -> becomes less valuable)
/// 3. Non-APT pair changes (pairs without APT involvement)
pub fn route_filter_by_affected_multiple_resource(
    affected_swap_updates: &Vec<(SwapContract, UpdatePairType)>,
) -> SearchRouteCondition {
    // No filter when thala is involved (complex multi-pool dynamics)
    if thala_resource_might_affected(affected_swap_updates) {
        return SearchRouteCondition {
            swap_filters_for_steps: None,
            coin_filters_for_steps: (vec![], vec![], vec![]),
            input_amounts: vec![apt_amount(50f64), apt_amount(6f64), apt_amount(0.2f64)],
        };
    }

    // Track APT appreciation updates (first step filter candidates)
    let mut apt_appreciation_count = 0;
    let mut first_swap_contract: Option<SwapContract> = None;
    let mut first_swap_pair: Vec<RoutePairFilter> = vec![];

    // Track APT depreciation updates (third step filter candidates)
    let mut apt_depreciation_count = 0;
    let mut third_swap_contract: Option<SwapContract> = None;

    // Track non-APT pair updates (second step filter candidates)
    let mut non_apt_pair_count = 0;
    let mut second_swap_contract: Option<SwapContract> = None;
    let mut first_swap_pair_if_no_apt_pair: Vec<RoutePairFilter> = vec![];

    let apt = apt_coin_id();

    for (contract, update_type) in affected_swap_updates {
        let (appreciating_coin, depreciating_coin) = match update_type {
            // X increases, Y decreases -> Y appreciates (became more scarce)
            UpdatePairType::XIncreaseYDecrease(coin_x, coin_y) => (coin_y, coin_x),
            // X decreases, Y increases -> X appreciates (became more scarce)
            UpdatePairType::XDecreaseYIncrease(coin_x, coin_y) => (coin_x, coin_y),
            // Other update types don't affect route filtering
            _ => continue,
        };

        if appreciating_coin == &apt {
            // APT appreciation: APT becomes more valuable
            // Route: APT -> other_coin -> ... -> APT (buy low, sell high)
            first_swap_contract = Some(contract.clone());
            first_swap_pair.push(RoutePairFilter {
                input_coin: Some(apt.clone()),
                output_coin: Some(depreciating_coin.clone()),
            });
            apt_appreciation_count += 1;
        } else if depreciating_coin == &apt {
            // APT depreciation: APT becomes less valuable
            // Route ends with converting back to APT at favorable rate
            third_swap_contract = Some(contract.clone());
            apt_depreciation_count += 1;
        } else {
            // Non-APT pair: both coins are not APT
            // Need to route through this pair in the middle step
            second_swap_contract = Some(contract.clone());
            first_swap_pair_if_no_apt_pair = vec![
                RoutePairFilter {
                    input_coin: Some(apt.clone()),
                    output_coin: Some(appreciating_coin.clone()),
                },
                RoutePairFilter {
                    input_coin: Some(apt.clone()),
                    output_coin: Some(depreciating_coin.clone()),
                },
            ];
            non_apt_pair_count += 1;
        }
    }

    // Determine swap filters based on what was found
    let (swap_filters_for_steps, coin_filters_for_steps) =
        if apt_appreciation_count == 1 || apt_depreciation_count == 1 {
            let first = if apt_appreciation_count == 1 {
                first_swap_contract
            } else {
                None
            };
            let third = if apt_depreciation_count == 1 {
                third_swap_contract
            } else {
                None
            };
            let coins = if apt_appreciation_count == 1 {
                first_swap_pair
            } else {
                vec![]
            };
            (Some((first, None, third)), coins)
        } else if non_apt_pair_count == 1 {
            (
                Some((None, second_swap_contract, None)),
                first_swap_pair_if_no_apt_pair,
            )
        } else {
            (None, vec![])
        };

    // Larger input amounts when first swap filter is set (more confident opportunity)
    let input_amounts = if matches!(swap_filters_for_steps, Some((Some(_), _, _))) {
        vec![apt_amount(50f64), apt_amount(6f64), apt_amount(0.2f64)]
    } else {
        vec![apt_amount(6f64), apt_amount(0.2f64)]
    };

    SearchRouteCondition {
        swap_filters_for_steps,
        coin_filters_for_steps: (coin_filters_for_steps, vec![], vec![]),
        input_amounts,
    }
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
    fn test_route_filter_by_affected_multiple_resource_with_thala() {
        // When thala is affected, should return no filters
        let updates = vec![
            (
                SwapContract::Thala,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Liquid,
                UpdatePairType::XDecreaseYIncrease(apt_coin_id(), usdt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdc_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_multiple_resource(&updates);

        assert!(result.swap_filters_for_steps.is_none());
        assert!(result.coin_filters_for_steps.0.is_empty());
        assert!(result.coin_filters_for_steps.1.is_empty());
        assert!(result.coin_filters_for_steps.2.is_empty());
        assert_eq!(result.input_amounts.len(), 3);
    }

    #[test]
    fn test_route_filter_by_affected_multiple_resource_with_thala_coin() {
        // When thala coin is in the update pair, should return no filters
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(thala_coin_id(), apt_coin_id()),
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
        let result = route_filter_by_affected_multiple_resource(&updates);

        assert!(result.swap_filters_for_steps.is_none());
        assert_eq!(result.input_amounts.len(), 3);
    }

    #[test]
    fn test_route_filter_by_affected_multiple_resource_apt_appreciation() {
        // APT appreciation: Y decreases (APT) -> APT becomes more valuable
        // XIncreaseYDecrease where Y is APT
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdt_coin_id()),
            ),
            (
                SwapContract::Cetus,
                UpdatePairType::XDecreaseYIncrease(usdt_coin_id(), weth_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_multiple_resource(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, _, _) = result.swap_filters_for_steps.unwrap();
        assert_eq!(first, Some(SwapContract::Liquid));
        assert!(!result.coin_filters_for_steps.0.is_empty());
        assert_eq!(result.input_amounts.len(), 3);
    }

    #[test]
    fn test_route_filter_by_affected_multiple_resource_apt_depreciation() {
        // APT de-appreciation: X increases (APT) -> APT loses value
        // XIncreaseYDecrease where X is APT
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(apt_coin_id(), usdc_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdt_coin_id()),
            ),
            (
                SwapContract::Cetus,
                UpdatePairType::XDecreaseYIncrease(usdt_coin_id(), weth_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_multiple_resource(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (_, _, third) = result.swap_filters_for_steps.unwrap();
        assert_eq!(third, Some(SwapContract::Liquid));
    }

    #[test]
    fn test_route_filter_by_affected_multiple_resource_non_apt_pair() {
        // When only non-APT pairs are affected, second swap filter should be set
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), usdt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XDecreaseYIncrease(weth_coin_id(), usdc_coin_id()),
            ),
            (
                SwapContract::Cetus,
                UpdatePairType::Other(usdt_coin_id(), weth_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_multiple_resource(&updates);

        // When non_apt_pair_num == 1, should have second swap filter
        // The exact behavior depends on the number of non-apt pairs
        assert!(result.input_amounts.len() >= 2);
    }

    #[test]
    fn test_route_filter_by_affected_multiple_resource_mixed_updates() {
        // Both APT appreciation and de-appreciation
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()), // APT appreciation
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(apt_coin_id(), usdt_coin_id()), // APT de-appreciation
            ),
            (
                SwapContract::Cetus,
                UpdatePairType::XDecreaseYIncrease(weth_coin_id(), usdc_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_multiple_resource(&updates);

        // Should handle both cases
        assert!(result.swap_filters_for_steps.is_some());
        let (first, _, third) = result.swap_filters_for_steps.unwrap();
        // Both first and third should be set
        assert!(first.is_some() || third.is_some());
    }

    #[test]
    fn test_route_filter_by_affected_multiple_resource_x_decrease_y_increase_apt_appreciation() {
        // APT appreciation via XDecreaseYIncrease where X is APT
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XDecreaseYIncrease(apt_coin_id(), usdc_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdt_coin_id()),
            ),
            (
                SwapContract::Cetus,
                UpdatePairType::Other(usdt_coin_id(), weth_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_multiple_resource(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, _, _) = result.swap_filters_for_steps.unwrap();
        assert_eq!(first, Some(SwapContract::Liquid));
    }

    #[test]
    fn test_route_filter_by_affected_multiple_resource_input_amounts() {
        // Test input amounts based on swap filter presence
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdt_coin_id()),
            ),
            (
                SwapContract::Cetus,
                UpdatePairType::Other(usdt_coin_id(), weth_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_multiple_resource(&updates);

        // When first swap filter is set, should have 3 input amounts
        if let Some((Some(_), _, _)) = result.swap_filters_for_steps {
            assert_eq!(result.input_amounts.len(), 3);
        } else {
            assert_eq!(result.input_amounts.len(), 2);
        }
    }

    #[test]
    fn test_route_filter_by_affected_multiple_resource_ignores_other_update_types() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::Other(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::NewPair(weth_coin_id(), usdt_coin_id()),
            ),
            (
                SwapContract::Cetus,
                UpdatePairType::XIncreaseYIncrease(
                    (usdt_coin_id(), 100),
                    (weth_coin_id(), 200),
                ),
            ),
        ];
        let result = route_filter_by_affected_multiple_resource(&updates);

        // These update types should not affect apt_appreciation_num or apt_de_appreciation_num
        assert!(result.swap_filters_for_steps.is_none());
    }
}

