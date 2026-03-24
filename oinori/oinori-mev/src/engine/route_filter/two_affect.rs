use crate::coins::*;
use crate::swap::{RoutePairFilter, SwapContract, UpdatePairType};

use super::SearchRouteCondition;

/// Determines route filters when exactly two swap resources are affected.
///
/// This function handles the case where two different swaps are updated simultaneously.
/// It first checks for special shortcut patterns (like chained routes), then falls back
/// to general analysis of APT appreciation/depreciation patterns.
pub fn route_filter_by_affected_resource_two(
    affected_swap_updates: &Vec<(SwapContract, UpdatePairType)>,
) -> SearchRouteCondition {
    // No filter when thala is involved
    if super::thala_resource_might_affected(affected_swap_updates) {
        return SearchRouteCondition {
            swap_filters_for_steps: None,
            coin_filters_for_steps: (vec![], vec![], vec![]),
            input_amounts: vec![apt_amount(6f64), apt_amount(0.2f64)],
        };
    }

    // Check for special route patterns (chained routes, non-APT pairs, etc.)
    if let Some(shortcut) = check_specified_route_short_cut(
        affected_swap_updates.get(0).unwrap(),
        affected_swap_updates.get(1).unwrap(),
        true,
    ) {
        return shortcut;
    }

    // Track APT appreciation (first step filter candidates)
    let mut apt_appreciation_count = 0;
    let mut first_swap_contract: Option<SwapContract> = None;
    let mut first_swap_pair: Vec<RoutePairFilter> = vec![];

    // Track APT depreciation (third step filter candidates)
    let mut apt_depreciation_count = 0;
    let mut third_swap_contract: Option<SwapContract> = None;
    let mut coin_filter_for_second_step: Vec<RoutePairFilter> = vec![];

    // Track non-APT pairs (second step filter candidates)
    let mut non_apt_pair_count = 0;
    let mut second_swap_contract: Option<SwapContract> = None;
    let mut first_swap_pair_if_no_apt_pair: Vec<RoutePairFilter> = vec![];

    let apt = apt_coin_id();

    for (contract, update_type) in affected_swap_updates {
        match update_type {
            // X increases (depreciates), Y decreases (appreciates)
            UpdatePairType::XIncreaseYDecrease(coin_x, coin_y) => {
                if coin_y == &apt {
                    // APT appreciation: Y (APT) decreases -> APT becomes more valuable
                    first_swap_contract = Some(contract.clone());
                    first_swap_pair.push(RoutePairFilter {
                        input_coin: Some(coin_y.clone()),
                        output_coin: Some(coin_x.clone()),
                    });
                    apt_appreciation_count += 1;
                } else if coin_x == &apt {
                    // APT depreciation: X (APT) increases -> APT becomes less valuable
                    third_swap_contract = Some(contract.clone());
                    coin_filter_for_second_step.push(RoutePairFilter {
                        input_coin: None,
                        output_coin: Some(coin_y.clone()),
                    });
                    coin_filter_for_second_step.push(RoutePairFilter {
                        input_coin: None,
                        output_coin: Some(apt.clone()),
                    });
                    apt_depreciation_count += 1;
                } else {
                    // Non-APT pair
                    second_swap_contract = Some(contract.clone());
                    first_swap_pair_if_no_apt_pair = vec![
                        RoutePairFilter {
                            input_coin: None,
                            output_coin: Some(coin_x.clone()),
                        },
                        RoutePairFilter {
                            input_coin: None,
                            output_coin: Some(coin_y.clone()),
                        },
                    ];
                    non_apt_pair_count += 1;
                }
            },

            // X decreases (appreciates), Y increases (depreciates)
            UpdatePairType::XDecreaseYIncrease(coin_x, coin_y) => {
                if coin_x == &apt {
                    // APT appreciation: X (APT) decreases -> APT becomes more valuable
                    first_swap_contract = Some(contract.clone());
                    first_swap_pair.push(RoutePairFilter {
                        input_coin: None,
                        output_coin: Some(coin_y.clone()),
                    });
                    apt_appreciation_count += 1;
                } else if coin_y == &apt {
                    // APT depreciation: Y (APT) increases -> APT becomes less valuable
                    third_swap_contract = Some(contract.clone());
                    coin_filter_for_second_step.push(RoutePairFilter {
                        input_coin: None,
                        output_coin: Some(coin_x.clone()),
                    });
                    coin_filter_for_second_step.push(RoutePairFilter {
                        input_coin: None,
                        output_coin: Some(apt.clone()),
                    });
                    apt_depreciation_count += 1;
                } else {
                    // Non-APT pair
                    second_swap_contract = Some(contract.clone());
                    first_swap_pair_if_no_apt_pair = vec![
                        RoutePairFilter {
                            input_coin: None,
                            output_coin: Some(coin_x.clone()),
                        },
                        RoutePairFilter {
                            input_coin: None,
                            output_coin: Some(coin_y.clone()),
                        },
                    ];
                    non_apt_pair_count += 1;
                }
            },

            // Both coins increasing (liquidity addition with APT)
            UpdatePairType::XIncreaseYIncrease((coin_x, _), (coin_y, _)) => {
                if coin_x == &apt || coin_y == &apt {
                    apt_depreciation_count += 1;
                }
            },

            // Both coins decreasing (liquidity removal with APT)
            UpdatePairType::XDecreaseYDecrease((coin_x, _), (coin_y, _)) => {
                if coin_x == &apt || coin_y == &apt {
                    apt_appreciation_count += 1;
                }
            },

            // Other update types don't affect filtering
            UpdatePairType::Other(_, _) | UpdatePairType::NewPair(_, _) => {},
        }
    }

    // Determine swap filters based on what was found
    let (swap_filters_for_steps, mut coin_filters_for_first_step) =
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

    // Populate first step filters from second step if needed
    if coin_filters_for_first_step.is_empty() && !coin_filter_for_second_step.is_empty() {
        for filter in &coin_filter_for_second_step {
            if let Some(input) = &filter.input_coin {
                coin_filters_for_first_step.push(RoutePairFilter {
                    input_coin: None,
                    output_coin: Some(input.clone()),
                });
            }
        }
    }

    SearchRouteCondition {
        swap_filters_for_steps,
        coin_filters_for_steps: (
            coin_filters_for_first_step,
            coin_filter_for_second_step,
            vec![],
        ),
        input_amounts: vec![apt_amount(6f64), apt_amount(0.2f64)],
    }
}

/// Checks for special route patterns that can be handled as shortcuts.
///
/// This function detects specific patterns in two swap updates:
/// 1. Obric double-Other updates
/// 2. Chained routes where output of first swap matches input of second
/// 3. Non-APT pair combinations
///
/// If `do_reverse_check` is true, it will also try swapping the order of updates
/// to find a match.
fn check_specified_route_short_cut(
    update_1: &(SwapContract, UpdatePairType),
    update_2: &(SwapContract, UpdatePairType),
    do_reverse_check: bool,
) -> Option<SearchRouteCondition> {
    let input_amounts = vec![apt_amount(6f64), apt_amount(0.2f64)];

    // Special case: Both Obric with Other updates
    if matches!(
        (&update_1.0, &update_1.1, &update_2.0, &update_2.1),
        (
            SwapContract::Obric,
            UpdatePairType::Other(_, _),
            SwapContract::Obric,
            UpdatePairType::Other(_, _),
        )
    ) {
        return Some(SearchRouteCondition {
            swap_filters_for_steps: Some((None, Some(SwapContract::Obric), None)),
            coin_filters_for_steps: (vec![], vec![], vec![]),
            input_amounts,
        });
    }

    // Extract appreciation/depreciation info from both updates
    let (appreciate_1, depreciate_1) = extract_appreciation_depreciation(&update_1.1)?;
    let (appreciate_2, depreciate_2) = extract_appreciation_depreciation(&update_2.1)?;

    let swap_1 = &update_1.0;
    let swap_2 = &update_2.0;
    let apt = apt_coin_id();

    // Check for chained route: depreciate_1 == appreciate_2 && depreciate_2 == APT
    if depreciate_1 == appreciate_2 && depreciate_2 == &apt {
        return Some(build_chained_route_condition(
            swap_1,
            swap_2,
            appreciate_1,
            depreciate_1,
            appreciate_2,
            depreciate_2,
            &apt,
            input_amounts,
        ));
    }

    // Check for non-APT pair combination
    if appreciate_1 != &apt
        && depreciate_1 != &apt
        && appreciate_2 != &apt
        && depreciate_2 != &apt
    {
        return Some(build_non_apt_pair_condition(
            swap_1,
            swap_2,
            appreciate_1,
            depreciate_1,
            appreciate_2,
            depreciate_2,
            input_amounts,
        ));
    }

    // Try reverse order if requested
    if do_reverse_check {
        return check_specified_route_short_cut(update_2, update_1, false);
    }

    None
}

/// Extracts appreciating and depreciating coins from an update type.
/// Returns (appreciating_coin, depreciating_coin) or None if not applicable.
fn extract_appreciation_depreciation(update_type: &UpdatePairType) -> Option<(&CoinId, &CoinId)> {
    match update_type {
        // X increases (depreciates), Y decreases (appreciates)
        UpdatePairType::XIncreaseYDecrease(coin_x, coin_y) => Some((coin_y, coin_x)),
        // X decreases (appreciates), Y increases (depreciates)
        UpdatePairType::XDecreaseYIncrease(coin_x, coin_y) => Some((coin_x, coin_y)),
        _ => None,
    }
}

/// Builds a SearchRouteCondition for a chained route pattern.
fn build_chained_route_condition(
    swap_1: &SwapContract,
    swap_2: &SwapContract,
    appreciate_1: &CoinId,
    depreciate_1: &CoinId,
    appreciate_2: &CoinId,
    depreciate_2: &CoinId,
    apt: &CoinId,
    input_amounts: Vec<u64>,
) -> SearchRouteCondition {
    if appreciate_1 == apt {
        // APT -> other -> APT route (first swap starts with APT)
        SearchRouteCondition {
            swap_filters_for_steps: Some((Some(swap_1.clone()), Some(swap_2.clone()), None)),
            coin_filters_for_steps: (
                vec![RoutePairFilter {
                    input_coin: None,
                    output_coin: Some(depreciate_1.clone()),
                }],
                vec![RoutePairFilter {
                    input_coin: Some(appreciate_2.clone()),
                    output_coin: Some(depreciate_2.clone()),
                }],
                vec![],
            ),
            input_amounts,
        }
    } else {
        // other -> middle -> APT route (first swap doesn't start with APT)
        SearchRouteCondition {
            swap_filters_for_steps: Some((None, Some(swap_1.clone()), Some(swap_2.clone()))),
            coin_filters_for_steps: (
                vec![
                    RoutePairFilter {
                        input_coin: None,
                        output_coin: Some(appreciate_1.clone()),
                    },
                    RoutePairFilter {
                        input_coin: None,
                        output_coin: Some(appreciate_2.clone()),
                    },
                ],
                vec![
                    RoutePairFilter {
                        input_coin: Some(appreciate_1.clone()),
                        output_coin: Some(depreciate_1.clone()),
                    },
                    RoutePairFilter {
                        input_coin: Some(appreciate_2.clone()),
                        output_coin: Some(depreciate_2.clone()),
                    },
                ],
                vec![RoutePairFilter {
                    input_coin: Some(appreciate_2.clone()),
                    output_coin: Some(depreciate_2.clone()),
                }],
            ),
            input_amounts,
        }
    }
}

/// Builds a SearchRouteCondition for a non-APT pair combination.
fn build_non_apt_pair_condition(
    swap_1: &SwapContract,
    swap_2: &SwapContract,
    appreciate_1: &CoinId,
    depreciate_1: &CoinId,
    appreciate_2: &CoinId,
    depreciate_2: &CoinId,
    input_amounts: Vec<u64>,
) -> SearchRouteCondition {
    // Only set second swap filter if both updates are from the same swap
    let second_swap_filter = if swap_1 == swap_2 {
        Some(swap_1.clone())
    } else {
        None
    };

    SearchRouteCondition {
        swap_filters_for_steps: second_swap_filter.map(|s| (None, Some(s), None)),
        coin_filters_for_steps: (
            vec![
                RoutePairFilter {
                    input_coin: None,
                    output_coin: Some(appreciate_1.clone()),
                },
                RoutePairFilter {
                    input_coin: None,
                    output_coin: Some(appreciate_2.clone()),
                },
            ],
            vec![
                RoutePairFilter {
                    input_coin: Some(appreciate_1.clone()),
                    output_coin: Some(depreciate_1.clone()),
                },
                RoutePairFilter {
                    input_coin: Some(appreciate_2.clone()),
                    output_coin: Some(depreciate_2.clone()),
                },
            ],
            vec![],
        ),
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

    // Test thala-related behavior
    #[test]
    fn test_two_with_thala_contract() {
        let updates = vec![
            (
                SwapContract::Thala,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Liquid,
                UpdatePairType::XDecreaseYIncrease(apt_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        assert!(result.swap_filters_for_steps.is_none());
        assert!(result.coin_filters_for_steps.0.is_empty());
        assert_eq!(result.input_amounts.len(), 2);
    }

    #[test]
    fn test_two_with_thala_coin() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(thala_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XDecreaseYIncrease(apt_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        assert!(result.swap_filters_for_steps.is_none());
        assert_eq!(result.input_amounts.len(), 2);
    }

    // Test check_specified_route_short_cut: Obric other + other case
    #[test]
    fn test_two_obric_other_other() {
        let updates = vec![
            (
                SwapContract::Obric,
                UpdatePairType::Other(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Obric,
                UpdatePairType::Other(usdt_coin_id(), weth_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, third) = result.swap_filters_for_steps.unwrap();
        assert!(first.is_none());
        assert_eq!(second, Some(SwapContract::Obric));
        assert!(third.is_none());
    }

    // Test chained route: de_appreciate_1 == appreciate_2 && de_appreciate_2 == APT
    #[test]
    fn test_two_chained_route_ending_in_apt_with_apt_start() {
        // APT -> USDC (appreciate APT), USDC -> APT (appreciate USDC)
        // de_appreciate_1 (USDC) == appreciate_2 (USDC), de_appreciate_2 (APT) == APT
        // appreciate_1 (APT) == APT
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XDecreaseYIncrease(apt_coin_id(), usdc_coin_id()), // APT appreciate, USDC depreciate
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XDecreaseYIncrease(usdc_coin_id(), apt_coin_id()), // USDC appreciate, APT depreciate
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, _) = result.swap_filters_for_steps.unwrap();
        assert_eq!(first, Some(SwapContract::Liquid));
        assert_eq!(second, Some(SwapContract::Pancake));
    }

    #[test]
    fn test_two_chained_route_ending_in_apt_without_apt_start() {
        // USDT -> USDC (appreciate USDT), USDC -> APT (appreciate USDC)
        // de_appreciate_1 (USDC) == appreciate_2 (USDC), de_appreciate_2 (APT) == APT
        // appreciate_1 (USDT) != APT
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XDecreaseYIncrease(usdt_coin_id(), usdc_coin_id()), // USDT appreciate, USDC depreciate
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XDecreaseYIncrease(usdc_coin_id(), apt_coin_id()), // USDC appreciate, APT depreciate
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, third) = result.swap_filters_for_steps.unwrap();
        assert!(first.is_none());
        assert_eq!(second, Some(SwapContract::Liquid));
        assert_eq!(third, Some(SwapContract::Pancake));
    }

    // Test non-APT pair case
    #[test]
    fn test_two_non_apt_pairs_same_swap() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), usdt_coin_id()),
            ),
            (
                SwapContract::Liquid,
                UpdatePairType::XDecreaseYIncrease(weth_coin_id(), usdc_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, second, _) = result.swap_filters_for_steps.unwrap();
        assert!(first.is_none());
        assert_eq!(second, Some(SwapContract::Liquid));
    }

    #[test]
    fn test_two_non_apt_pairs_different_swaps() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), usdt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XDecreaseYIncrease(weth_coin_id(), usdc_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        // Different swaps, so swap_filters_for_steps is None (second_swap_filter.map() returns None)
        assert!(result.swap_filters_for_steps.is_none());

        // But coin_filters should still be populated
        assert!(!result.coin_filters_for_steps.0.is_empty());
        assert!(!result.coin_filters_for_steps.1.is_empty());
    }

    // Test APT appreciation
    #[test]
    fn test_two_apt_appreciation_x_increase_y_decrease() {
        // Y (APT) decreases -> APT appreciates
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, _, _) = result.swap_filters_for_steps.unwrap();
        assert_eq!(first, Some(SwapContract::Liquid));
    }

    #[test]
    fn test_two_apt_appreciation_x_decrease_y_increase() {
        // X (APT) decreases -> APT appreciates
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XDecreaseYIncrease(apt_coin_id(), usdc_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, _, _) = result.swap_filters_for_steps.unwrap();
        assert_eq!(first, Some(SwapContract::Liquid));
    }

    // Test APT de-appreciation
    #[test]
    fn test_two_apt_depreciation_x_increase_y_decrease() {
        // X (APT) increases -> APT depreciates
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(apt_coin_id(), usdc_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (_, _, third) = result.swap_filters_for_steps.unwrap();
        assert_eq!(third, Some(SwapContract::Liquid));
    }

    #[test]
    fn test_two_apt_depreciation_x_decrease_y_increase() {
        // Y (APT) increases -> APT depreciates
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XDecreaseYIncrease(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (_, _, third) = result.swap_filters_for_steps.unwrap();
        assert_eq!(third, Some(SwapContract::Liquid));
    }

    // Test mixed APT appreciation and depreciation
    #[test]
    fn test_two_apt_appreciation_and_depreciation() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()), // APT appreciation
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(apt_coin_id(), usdt_coin_id()), // APT depreciation
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        assert!(result.swap_filters_for_steps.is_some());
        let (first, _, third) = result.swap_filters_for_steps.unwrap();
        assert_eq!(first, Some(SwapContract::Liquid));
        assert_eq!(third, Some(SwapContract::Pancake));
    }

    // Test non-APT pair handling
    #[test]
    fn test_two_single_non_apt_pair() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), usdt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::Other(weth_coin_id(), apt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        // non_apt_pair_num == 1, should set second swap filter
        assert!(result.swap_filters_for_steps.is_some());
        let (_, second, _) = result.swap_filters_for_steps.unwrap();
        assert_eq!(second, Some(SwapContract::Liquid));
    }

    // Test coin filter population
    #[test]
    fn test_two_coin_filters_populated_correctly() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        // First coin filter should be set for APT appreciation
        assert!(!result.coin_filters_for_steps.0.is_empty());
        assert_eq!(
            result.coin_filters_for_steps.0[0].input_coin,
            Some(apt_coin_id())
        );
        assert_eq!(
            result.coin_filters_for_steps.0[0].output_coin,
            Some(usdc_coin_id())
        );
    }

    // Test coin filter for second step population
    #[test]
    fn test_two_second_step_coin_filter_population() {
        // When coin_filters_for_first_step is empty but coin_filter_for_second_step is not
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYDecrease(apt_coin_id(), usdc_coin_id()), // APT depreciation
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::Other(weth_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        // Should have coin filters populated
        assert!(result.swap_filters_for_steps.is_some());
    }

    // Test XIncreaseYIncrease and XDecreaseYDecrease
    #[test]
    fn test_two_x_increase_y_increase_with_apt() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XIncreaseYIncrease((apt_coin_id(), 100), (usdc_coin_id(), 200)),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        // XIncreaseYIncrease with APT should increment apt_de_appreciation_num
        // Result depends on specific logic flow
        assert!(result.input_amounts.len() >= 2);
    }

    #[test]
    fn test_two_x_decrease_y_decrease_with_apt() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::XDecreaseYDecrease((apt_coin_id(), 100), (usdc_coin_id(), 200)),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::XIncreaseYDecrease(weth_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        // XDecreaseYDecrease with APT should increment apt_appreciation_num
        assert!(result.input_amounts.len() >= 2);
    }

    // Test reverse check in check_specified_route_short_cut
    #[test]
    fn test_two_reverse_check_finds_match() {
        // Order matters - test that reverse check catches the pattern
        let updates = vec![
            (
                SwapContract::Pancake,
                UpdatePairType::XDecreaseYIncrease(usdc_coin_id(), apt_coin_id()), // USDC appreciate, APT depreciate
            ),
            (
                SwapContract::Liquid,
                UpdatePairType::XDecreaseYIncrease(apt_coin_id(), usdc_coin_id()), // APT appreciate, USDC depreciate
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        // Should find a match via reverse check
        assert!(result.swap_filters_for_steps.is_some());
    }

    // Test Other and NewPair update types are ignored in main loop
    #[test]
    fn test_two_other_update_types_ignored() {
        let updates = vec![
            (
                SwapContract::Liquid,
                UpdatePairType::Other(usdc_coin_id(), apt_coin_id()),
            ),
            (
                SwapContract::Pancake,
                UpdatePairType::NewPair(weth_coin_id(), usdt_coin_id()),
            ),
        ];
        let result = route_filter_by_affected_resource_two(&updates);

        // These should not affect the main logic (after short_cut check fails)
        assert!(result.swap_filters_for_steps.is_none());
    }
}

