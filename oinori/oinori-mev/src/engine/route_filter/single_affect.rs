use crate::coins::*;
use crate::swap::{RoutePairFilter, SwapContract, UpdatePairType};

use super::SearchRouteCondition;

/// Determines route filters when a single swap resource is affected.
///
/// This function analyzes a single swap update to determine optimal route filtering.
/// The filtering strategy depends on whether APT is involved and which coin is
/// appreciating vs depreciating:
///
/// - APT appreciation (APT decreases): First step filter with eager input amounts
/// - APT depreciation (APT increases): Second step filter with eager input amounts
/// - Non-APT pair: Second step filter with normal input amounts
pub fn route_filter_by_affected_resource_single(
    affected_swap_updates: (SwapContract, UpdatePairType),
) -> SearchRouteCondition {
    let (contract, update_type) = affected_swap_updates;
    let apt = apt_coin_id();

    let (swap_filters, first_filter, second_filter, is_eager) = match update_type {
        // X increases (depreciates), Y decreases (appreciates)
        UpdatePairType::XIncreaseYDecrease(depreciating, appreciating) => {
            process_appreciation_depreciation(&contract, &apt, appreciating, depreciating)
        },

        // X decreases (appreciates), Y increases (depreciates)
        UpdatePairType::XDecreaseYIncrease(appreciating, depreciating) => {
            process_appreciation_depreciation(&contract, &apt, appreciating, depreciating)
        },

        // Unknown direction - filter on both coins if non-APT pair
        UpdatePairType::Other(coin_x, coin_y) => {
            if coin_x != apt && coin_y != apt {
                (
                    Some((None, Some(contract), None)),
                    vec![
                        RoutePairFilter {
                            input_coin: None,
                            output_coin: Some(coin_x),
                        },
                        RoutePairFilter {
                            input_coin: None,
                            output_coin: Some(coin_y),
                        },
                    ],
                    vec![],
                    false,
                )
            } else {
                (None, vec![], vec![], false)
            }
        },

        // New pair - no filtering needed
        UpdatePairType::NewPair(_, _) => (None, vec![], vec![], false),

        // Both coins increasing - indicates liquidity addition
        UpdatePairType::XIncreaseYIncrease((coin_x, _), (coin_y, _)) => {
            process_both_direction_change(&contract, &apt, coin_x, coin_y)
        },

        // Both coins decreasing - indicates liquidity removal
        UpdatePairType::XDecreaseYDecrease((coin_x, _), (coin_y, _)) => {
            process_both_direction_change(&contract, &apt, coin_x, coin_y)
        },
    };

    let input_amounts = if is_eager {
        vec![apt_amount(200f64), apt_amount(6f64), apt_amount(0.2f64)]
    } else {
        vec![apt_amount(6f64), apt_amount(0.2f64)]
    };

    SearchRouteCondition {
        swap_filters_for_steps: swap_filters,
        coin_filters_for_steps: (first_filter, second_filter, vec![]),
        input_amounts,
    }
}

/// Process appreciation/depreciation pair to determine filters.
///
/// Returns (swap_filters, first_coin_filter, second_coin_filter, is_eager)
fn process_appreciation_depreciation(
    contract: &SwapContract,
    apt: &CoinId,
    appreciating: CoinId,
    depreciating: CoinId,
) -> (
    Option<(Option<SwapContract>, Option<SwapContract>, Option<SwapContract>)>,
    Vec<RoutePairFilter>,
    Vec<RoutePairFilter>,
    bool,
) {
    if &appreciating == apt {
        // APT appreciation: APT becomes more valuable (its reserve decreased)
        // Use as first step filter - buy other coin with APT
        (
            Some((Some(contract.clone()), None, None)),
            vec![RoutePairFilter {
                input_coin: None,
                output_coin: Some(depreciating),
            }],
            vec![],
            true,
        )
    } else if &depreciating == apt {
        // APT depreciation: APT becomes less valuable (its reserve increased)
        // Use as second step filter - buy APT with other coin
        (
            Some((None, Some(contract.clone()), None)),
            vec![RoutePairFilter {
                input_coin: None,
                output_coin: Some(appreciating.clone()),
            }],
            vec![RoutePairFilter {
                input_coin: Some(appreciating),
                output_coin: Some(apt.clone()),
            }],
            true,
        )
    } else {
        // Non-APT pair: route through this pair in second step
        (
            Some((None, Some(contract.clone()), None)),
            vec![RoutePairFilter {
                input_coin: None,
                output_coin: Some(appreciating.clone()),
            }],
            vec![RoutePairFilter {
                input_coin: Some(appreciating),
                output_coin: Some(depreciating),
            }],
            false,
        )
    }
}

/// Process cases where both coins change in the same direction (both increase or both decrease).
///
/// Returns (swap_filters, first_coin_filter, second_coin_filter, is_eager)
fn process_both_direction_change(
    contract: &SwapContract,
    apt: &CoinId,
    coin_x: CoinId,
    coin_y: CoinId,
) -> (
    Option<(Option<SwapContract>, Option<SwapContract>, Option<SwapContract>)>,
    Vec<RoutePairFilter>,
    Vec<RoutePairFilter>,
    bool,
) {
    if &coin_x == apt || &coin_y == apt {
        // APT involved - use as first step filter with eager input
        (Some((Some(contract.clone()), None, None)), vec![], vec![], true)
    } else {
        // Non-APT pair - use as second step filter
        (
            Some((None, Some(contract.clone()), None)),
            vec![
                RoutePairFilter {
                    input_coin: None,
                    output_coin: Some(coin_x),
                },
                RoutePairFilter {
                    input_coin: None,
                    output_coin: Some(coin_y),
                },
            ],
            vec![],
            false,
        )
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

    // Test XIncreaseYDecrease cases
    #[test]
    fn test_single_x_increase_y_decrease_apt_appreciation() {
        // Y (APT) decreases -> APT appreciates
        let update = (
            SwapContract::Liquid,
            UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, third) = result.swap_filters_for_steps.unwrap();
        assert_eq!(first, Some(SwapContract::Liquid));
        assert!(second.is_none());
        assert!(third.is_none());

        // First filter should target output coin (the increased coin)
        assert_eq!(result.coin_filters_for_steps.0.len(), 1);
        assert_eq!(
            result.coin_filters_for_steps.0[0].output_coin,
            Some(usdc_coin_id())
        );

        // Eager input amounts (3 elements)
        assert_eq!(result.input_amounts.len(), 3);
    }

    #[test]
    fn test_single_x_increase_y_decrease_apt_depreciation() {
        // X (APT) increases -> APT depreciates
        let update = (
            SwapContract::Liquid,
            UpdatePairType::XIncreaseYDecrease(apt_coin_id(), usdc_coin_id()),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, third) = result.swap_filters_for_steps.unwrap();
        assert!(first.is_none());
        assert_eq!(second, Some(SwapContract::Liquid));
        assert!(third.is_none());

        // First filter targets output to the decreased coin
        assert_eq!(result.coin_filters_for_steps.0.len(), 1);
        assert_eq!(
            result.coin_filters_for_steps.0[0].output_coin,
            Some(usdc_coin_id())
        );

        // Second filter has input/output
        assert_eq!(result.coin_filters_for_steps.1.len(), 1);
        assert_eq!(
            result.coin_filters_for_steps.1[0].input_coin,
            Some(usdc_coin_id())
        );
        assert_eq!(
            result.coin_filters_for_steps.1[0].output_coin,
            Some(apt_coin_id())
        );

        // Eager input amounts (3 elements)
        assert_eq!(result.input_amounts.len(), 3);
    }

    #[test]
    fn test_single_x_increase_y_decrease_non_apt_pair() {
        // Neither coin is APT - coin_id_y is appreciated
        let update = (
            SwapContract::Liquid,
            UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), usdt_coin_id()),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, third) = result.swap_filters_for_steps.unwrap();
        assert!(first.is_none());
        assert_eq!(second, Some(SwapContract::Liquid));
        assert!(third.is_none());

        // First filter targets the decreasing coin
        assert_eq!(result.coin_filters_for_steps.0.len(), 1);
        assert_eq!(
            result.coin_filters_for_steps.0[0].output_coin,
            Some(usdt_coin_id())
        );

        // Second filter: input = decreasing coin, output = increasing coin
        assert_eq!(result.coin_filters_for_steps.1.len(), 1);
        assert_eq!(
            result.coin_filters_for_steps.1[0].input_coin,
            Some(usdt_coin_id())
        );
        assert_eq!(
            result.coin_filters_for_steps.1[0].output_coin,
            Some(usdc_coin_id())
        );

        // Non-eager input amounts (2 elements)
        assert_eq!(result.input_amounts.len(), 2);
    }

    // Test XDecreaseYIncrease cases
    #[test]
    fn test_single_x_decrease_y_increase_apt_appreciation() {
        // X (APT) decreases -> APT appreciates
        let update = (
            SwapContract::Pancake,
            UpdatePairType::XDecreaseYIncrease(apt_coin_id(), usdc_coin_id()),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, third) = result.swap_filters_for_steps.unwrap();
        assert_eq!(first, Some(SwapContract::Pancake));
        assert!(second.is_none());
        assert!(third.is_none());

        // First filter targets the increasing coin
        assert_eq!(result.coin_filters_for_steps.0.len(), 1);
        assert_eq!(
            result.coin_filters_for_steps.0[0].output_coin,
            Some(usdc_coin_id())
        );

        // Eager input amounts
        assert_eq!(result.input_amounts.len(), 3);
    }

    #[test]
    fn test_single_x_decrease_y_increase_apt_depreciation() {
        // Y (APT) increases -> APT depreciates
        let update = (
            SwapContract::Pancake,
            UpdatePairType::XDecreaseYIncrease(usdc_coin_id(), apt_coin_id()),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, third) = result.swap_filters_for_steps.unwrap();
        assert!(first.is_none());
        assert_eq!(second, Some(SwapContract::Pancake));
        assert!(third.is_none());

        // First filter targets the decreasing coin
        assert_eq!(result.coin_filters_for_steps.0.len(), 1);
        assert_eq!(
            result.coin_filters_for_steps.0[0].output_coin,
            Some(usdc_coin_id())
        );

        // Second filter: input = decreasing coin, output = APT
        assert_eq!(result.coin_filters_for_steps.1.len(), 1);
        assert_eq!(
            result.coin_filters_for_steps.1[0].input_coin,
            Some(usdc_coin_id())
        );
        assert_eq!(
            result.coin_filters_for_steps.1[0].output_coin,
            Some(apt_coin_id())
        );

        // Eager input amounts
        assert_eq!(result.input_amounts.len(), 3);
    }

    #[test]
    fn test_single_x_decrease_y_increase_non_apt_pair() {
        // Neither coin is APT - coin_x is appreciated
        let update = (
            SwapContract::Pancake,
            UpdatePairType::XDecreaseYIncrease(usdc_coin_id(), usdt_coin_id()),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, third) = result.swap_filters_for_steps.unwrap();
        assert!(first.is_none());
        assert_eq!(second, Some(SwapContract::Pancake));
        assert!(third.is_none());

        // First filter targets the decreasing coin
        assert_eq!(result.coin_filters_for_steps.0.len(), 1);
        assert_eq!(
            result.coin_filters_for_steps.0[0].output_coin,
            Some(usdc_coin_id())
        );

        // Second filter: input = decreasing coin, output = increasing coin
        assert_eq!(result.coin_filters_for_steps.1.len(), 1);
        assert_eq!(
            result.coin_filters_for_steps.1[0].input_coin,
            Some(usdc_coin_id())
        );
        assert_eq!(
            result.coin_filters_for_steps.1[0].output_coin,
            Some(usdt_coin_id())
        );

        // Non-eager input amounts
        assert_eq!(result.input_amounts.len(), 2);
    }

    // Test Other case
    #[test]
    fn test_single_other_non_apt_pair() {
        let update = (
            SwapContract::Cetus,
            UpdatePairType::Other(usdc_coin_id(), usdt_coin_id()),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, third) = result.swap_filters_for_steps.unwrap();
        assert!(first.is_none());
        assert_eq!(second, Some(SwapContract::Cetus));
        assert!(third.is_none());

        // Two filters for both coins
        assert_eq!(result.coin_filters_for_steps.0.len(), 2);
        assert_eq!(
            result.coin_filters_for_steps.0[0].output_coin,
            Some(usdc_coin_id())
        );
        assert_eq!(
            result.coin_filters_for_steps.0[1].output_coin,
            Some(usdt_coin_id())
        );
    }

    #[test]
    fn test_single_other_apt_pair_does_nothing() {
        let update = (
            SwapContract::Cetus,
            UpdatePairType::Other(apt_coin_id(), usdc_coin_id()),
        );
        let result = route_filter_by_affected_resource_single(update);

        // When APT is in the pair, no filter is set
        assert!(result.swap_filters_for_steps.is_none());
        assert!(result.coin_filters_for_steps.0.is_empty());
    }

    // Test NewPair case
    #[test]
    fn test_single_new_pair_does_nothing() {
        let update = (
            SwapContract::Obric,
            UpdatePairType::NewPair(usdc_coin_id(), usdt_coin_id()),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_none());
        assert!(result.coin_filters_for_steps.0.is_empty());
    }

    // Test XIncreaseYIncrease cases
    #[test]
    fn test_single_x_increase_y_increase_with_apt() {
        let update = (
            SwapContract::Liquid,
            UpdatePairType::XIncreaseYIncrease((apt_coin_id(), 100), (usdc_coin_id(), 200)),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, _, _) = result.swap_filters_for_steps.unwrap();
        assert_eq!(first, Some(SwapContract::Liquid));

        // Eager input amounts
        assert_eq!(result.input_amounts.len(), 3);
    }

    #[test]
    fn test_single_x_increase_y_increase_non_apt() {
        let update = (
            SwapContract::Liquid,
            UpdatePairType::XIncreaseYIncrease((usdc_coin_id(), 100), (usdt_coin_id(), 200)),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, _) = result.swap_filters_for_steps.unwrap();
        assert!(first.is_none());
        assert_eq!(second, Some(SwapContract::Liquid));

        // Two filters for both coins
        assert_eq!(result.coin_filters_for_steps.0.len(), 2);
    }

    // Test XDecreaseYDecrease cases
    #[test]
    fn test_single_x_decrease_y_decrease_with_apt() {
        let update = (
            SwapContract::Aux,
            UpdatePairType::XDecreaseYDecrease((apt_coin_id(), 100), (usdc_coin_id(), 200)),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, _, _) = result.swap_filters_for_steps.unwrap();
        assert_eq!(first, Some(SwapContract::Aux));

        // Eager input amounts
        assert_eq!(result.input_amounts.len(), 3);
    }

    #[test]
    fn test_single_x_decrease_y_decrease_non_apt() {
        let update = (
            SwapContract::Aux,
            UpdatePairType::XDecreaseYDecrease((usdc_coin_id(), 100), (usdt_coin_id(), 200)),
        );
        let result = route_filter_by_affected_resource_single(update);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, _) = result.swap_filters_for_steps.unwrap();
        assert!(first.is_none());
        assert_eq!(second, Some(SwapContract::Aux));

        // Two filters for both coins
        assert_eq!(result.coin_filters_for_steps.0.len(), 2);
    }

    // Test different swap contracts
    #[test]
    fn test_single_different_swap_contracts() {
        let contracts = vec![
            SwapContract::Liquid,
            SwapContract::Aux,
            SwapContract::Anime,
            SwapContract::Pancake,
            SwapContract::Cetus,
            SwapContract::Obric,
            SwapContract::AptoSwap,
        ];

        for contract in contracts {
            let update = (
                contract.clone(),
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
            );
            let result = route_filter_by_affected_resource_single(update);

            let (first, _, _) = result.swap_filters_for_steps.unwrap();
            assert_eq!(first, Some(contract));
        }
    }
}

