use crate::swap::{CoinPairAndSwapDirection, SwapRoute, SwapRouteStep, SwapRouterPool};
use anyhow::Result;

pub fn optimize_route_amount_with_loan_fee<F>(
    mut original_swap_route: SwapRoute,

    max_optimize_search_depth: Option<usize>,

    liner_optimize_step_range_amount_and_step_num: (u64, usize),

    router_pool: &SwapRouterPool,
    loan_fee_calc: F,
    max_input_amount: Option<u64>,
) -> Result<SwapRoute>
where
    F: Fn(u64) -> u64,
{
    debug_assert!(!router_pool.is_empty());

    let max_input_amount = max_input_amount.unwrap_or(500_000_0000_0000);

    if original_swap_route.input_amount > max_input_amount {
        original_swap_route.input_amount = max_input_amount;
    }

    let max_optimize_search_depth = max_optimize_search_depth.unwrap_or(30);

    let swap_and_directions = original_swap_route.as_coin_pairs_and_directions(router_pool);

    // middle
    let new_middle = {
        let new_middle_input_amount = if max_input_amount < original_swap_route.input_amount {
            max_input_amount
        } else {
            original_swap_route.input_amount
        };
        calc_amounts_output(
            new_middle_input_amount,
            &swap_and_directions,
            &loan_fee_calc,
        )?
    };

    // small and greater
    let new_smaller = {
        let mut smaller_input_amount = new_middle.input_amount / 2 + 1;
        if max_input_amount < smaller_input_amount {
            smaller_input_amount = max_input_amount
        }

        calc_amounts_output(smaller_input_amount, &swap_and_directions, &loan_fee_calc)?
    };
    // greator

    let new_greater = {
        let mut greater_input_amount = new_middle.input_amount * 2 + 1;
        if max_input_amount < greater_input_amount {
            greater_input_amount = max_input_amount
        }

        calc_amounts_output(greater_input_amount, &swap_and_directions, &loan_fee_calc)?
    };

    let io_amounts = IoAmountsPoints {
        smaller: new_smaller,
        middle: new_middle,
        greater: new_greater,
    };

    let optimized_io = optimize_binary(
        io_amounts,
        swap_and_directions.as_slice(),
        0,
        &max_optimize_search_depth,
        &loan_fee_calc,
        max_input_amount,
    )?;

    debug_assert!(original_swap_route.route_steps.len() == optimized_io.step_outputs.len());

    let (liner_optimize_step_range_amount, liner_optimize_max_step) =
        liner_optimize_step_range_amount_and_step_num;
    let optimized_io = optimize_linear(
        optimized_io,
        liner_optimize_step_range_amount,
        liner_optimize_max_step,
        swap_and_directions.as_slice(),
        &loan_fee_calc,
        max_input_amount,
    )?;

    let new_steps: Vec<SwapRouteStep> = original_swap_route
        .route_steps
        .into_iter()
        .zip(optimized_io.step_outputs.into_iter())
        .map(|(step, output)| SwapRouteStep {
            swap_index_and_direction: step.swap_index_and_direction,
            out_amount: output,
        })
        .collect();
    Ok(SwapRoute {
        input_amount: optimized_io.input_amount,
        route_steps: new_steps,
    })
}

#[derive(Clone, Debug)]
struct IoAmounts {
    input_amount: u64,
    output_amount_after_subtract_fee: u64,
    step_outputs: Vec<u64>,
}
impl IoAmounts {
    fn output_profit_amount(&self) -> u64 {
        if self.input_amount > self.output_amount_after_subtract_fee {
            0
        } else {
            self.output_amount_after_subtract_fee - self.input_amount
        }
    }
}

#[derive(Debug)]
struct IoAmountsPoints {
    smaller: IoAmounts,
    middle: IoAmounts,
    greater: IoAmounts,
}

impl IoAmountsPoints {
    fn is_valid(&self) -> bool {
        if self.smaller.input_amount == 0
            || self.middle.input_amount == 0
            || self.greater.input_amount == 0
        {
            return false;
        }

        if self.middle.input_amount > self.greater.input_amount {
            return false;
        }

        if self.smaller.input_amount > self.middle.input_amount {
            return false;
        }
        true
    }
    fn is_flat(&self) -> bool {
        if self.middle.input_amount > 0
            && self.smaller.input_amount >= self.middle.input_amount - 1
            && self.greater.input_amount <= self.middle.input_amount + 1
        {
            return true;
        }

        return false;
    }

    fn get_max(self) -> IoAmounts {
        if self.smaller.output_profit_amount() >= self.middle.output_profit_amount() {
            // middle < smaller
            if self.smaller.output_profit_amount() < self.greater.output_profit_amount() {
                self.greater
            } else {
                self.smaller
            }
        } else {
            // --- middle > smaller ---

            if self.middle.output_profit_amount() < self.greater.output_profit_amount() {
                self.greater
            } else {
                self.middle
            }
        }
    }
}

fn optimize_binary<'a, F>(
    io_amounts: IoAmountsPoints,

    swap_and_directions: &[CoinPairAndSwapDirection<'a>],
    current_depth: usize,
    max_optimize_search_depth: &usize,

    loan_fee_calc: F,
    max_input_amount: u64,
) -> Result<IoAmounts>
where
    F: Fn(u64) -> u64,
{
    debug_assert!(io_amounts.is_valid());

    if current_depth > *max_optimize_search_depth {
        return Ok(io_amounts.get_max());
    }

    if io_amounts.is_flat() {
        return Ok(io_amounts.get_max());
    }

    let IoAmountsPoints {
        smaller,
        middle,
        greater,
    } = io_amounts;

    let profit_for_smaller_input = smaller.output_profit_amount();
    let protif_for_middle_input = middle.output_profit_amount();
    let profit_for_greater_input = greater.output_profit_amount();

    //println!(
    //    "profits {},{},{}",
    //    profit_for_smaller_input, protif_for_middle_input, profit_for_greater_input
    //);

    let next_amounts = if protif_for_middle_input >= profit_for_smaller_input {
        // middle > smaller
        if protif_for_middle_input > profit_for_greater_input {
            // shrink the range
            //println!(
            //    "11 shrink range output_for_middle_input  profits: {},{},{}",
            //    profit_for_smaller_input, protif_for_middle_input, profit_for_greater_input
            //);
            // middle >= greater >= smaller

            //----- IoAmountsPoints { smaller: IoAmounts { input_amount: 51372669371, output_amount_subtract_fee: 51562313572, step_outputs: [7604791043, 7599054757, 51721568847] },
            //
            //
            //middle: IoAmounts { input_amount: 68476311243, output_amount_subtract_fee: 68507183119, step_outputs: [10114834976, 10105601452, 68719459683] },
            //
            //greater: IoAmounts { input_amount: 34253591560, output_amount_subtract_fee: 34491860355, step_outputs: [5081604359, 5078580786, 34598046488] } }
            //-----asfasf 68476311243 ,34253591560
            //test swap::router::traverser::test::test_traverse_82799887 ... FAILED
            //
            //

            let new_smaller = {
                let mut new_input_amount = (smaller.input_amount + middle.input_amount) / 2 + 1;
                if max_input_amount < new_input_amount {
                    new_input_amount = max_input_amount
                }

                calc_amounts_output(new_input_amount, swap_and_directions, &loan_fee_calc)?
            };

            let new_greater = {
                let mut new_input_amount = (greater.input_amount + middle.input_amount) / 2 + 1;
                if max_input_amount < new_input_amount {
                    new_input_amount = max_input_amount
                }
                calc_amounts_output(new_input_amount, swap_and_directions, &loan_fee_calc)?
            };

            // move small
            let new_middle_input =
                middle.input_amount + ((new_greater.input_amount / middle.input_amount) / 4);
            let new_middle = if new_middle_input > max_input_amount {
                calc_amounts_output(max_input_amount, swap_and_directions, &loan_fee_calc)?
            } else {
                calc_amounts_output(new_middle_input, swap_and_directions, &loan_fee_calc)?
            };

            // shrink search range

            //println!(
            //    "   next input----  {},{},{}",
            //    new_smaller.input_amount, new_middle.input_amount, new_greater.input_amount
            //);

            IoAmountsPoints {
                smaller: new_smaller,
                middle: new_middle,
                greater: new_greater,
            }
        } else {
            //  greater >= middle >= smaller

            // move toward over the greater
            //println!(
            //    "22 move beyond greater output_for_middle_input profits: {}, {} , {}",
            //    profit_for_smaller_input, protif_for_middle_input, profit_for_greater_input
            //);

            let new_middle = {
                let mut new_input_amount = (middle.input_amount + greater.input_amount) / 2 + 1;
                if max_input_amount < new_input_amount {
                    new_input_amount = max_input_amount
                }

                calc_amounts_output(new_input_amount, swap_and_directions, &loan_fee_calc)?
            };

            let new_greater = {
                let mut new_input_amount = greater.input_amount * 2 + 1;
                if max_input_amount < new_input_amount {
                    new_input_amount = max_input_amount
                }
                calc_amounts_output(new_input_amount, swap_and_directions, &loan_fee_calc)?
            };

            let new_smaller = {
                let mut new_input_amount = (smaller.input_amount + new_middle.input_amount) / 2 + 1;
                if max_input_amount < new_input_amount {
                    new_input_amount = max_input_amount
                }

                calc_amounts_output(new_input_amount, swap_and_directions, &loan_fee_calc)?
            };

            //println!(
            //    "   next input----  {},{},{}",
            //    new_smaller.input_amount, new_middle.input_amount, new_greater.input_amount
            //);

            // go greater direction

            IoAmountsPoints {
                smaller: new_smaller,
                middle: new_middle,
                greater: new_greater,
            }
        }
    } else {
        // middle < smaller

        if profit_for_smaller_input < profit_for_greater_input {
            // greater>  smaller < middle

            //println!(
            //    "3  profits: {},{},{}",
            //    profit_for_smaller_input, protif_for_middle_input, profit_for_greater_input
            //);
            let new_smaller = {
                let mut new_input_amount = (middle.input_amount + greater.input_amount) / 2 + 1;
                if max_input_amount < new_input_amount {
                    new_input_amount = max_input_amount
                }

                calc_amounts_output(new_input_amount, swap_and_directions, &loan_fee_calc)?
            };

            let new_greater = {
                let mut new_input_amount = greater.input_amount * 2 + 1;
                if max_input_amount < new_input_amount {
                    new_input_amount = max_input_amount
                }

                calc_amounts_output(new_input_amount, swap_and_directions, &loan_fee_calc)?
            };

            let new_middle = if greater.input_amount > max_input_amount {
                calc_amounts_output(max_input_amount, swap_and_directions, &loan_fee_calc)?
            } else {
                greater
            };

            // go greater direction
            //println!(
            //    "   next input----  {},{},{}",
            //    new_smaller.input_amount, new_middle.input_amount, new_greater.input_amount
            //);

            IoAmountsPoints {
                smaller: new_smaller,
                middle: new_middle,
                greater: new_greater,
            }
        } else {
            // smaller >  middle >? greater
            //println!(
            //    "4 profits: move toward smaller  {}, {} {}",
            //    profit_for_smaller_input, protif_for_middle_input, profit_for_greater_input
            //);

            let new_smaller = {
                let mut new_input_amount = smaller.input_amount / 3 / 4 + 1;
                if max_input_amount < new_input_amount {
                    new_input_amount = max_input_amount
                }

                calc_amounts_output(new_input_amount, swap_and_directions, &loan_fee_calc)?
            };

            let new_middle = {
                let mut new_input_amount = (new_smaller.input_amount + middle.input_amount) / 2 + 1;

                if max_input_amount < new_input_amount {
                    new_input_amount = max_input_amount
                }

                calc_amounts_output(new_input_amount, swap_and_directions, &loan_fee_calc)?
            };

            let new_greater = if middle.input_amount > max_input_amount {
                calc_amounts_output(max_input_amount, swap_and_directions, &loan_fee_calc)?
            } else {
                middle
            };

            //println!(
            //    "   next input----  {},{},{}",
            //    new_smaller.input_amount, new_middle.input_amount, new_greater.input_amount
            //);

            IoAmountsPoints {
                smaller: new_smaller,
                middle: new_middle,
                greater: new_greater,
            }
        }
    };

    optimize_binary(
        next_amounts,
        swap_and_directions,
        current_depth + 1,
        max_optimize_search_depth,
        loan_fee_calc,
        max_input_amount,
    )
}

fn optimize_linear<'a, F>(
    mut io_amounts: IoAmounts,

    step_range_amount: u64,
    max_step_num: usize,

    swap_and_directions: &[CoinPairAndSwapDirection<'a>],
    loan_fee_calc: F,
    max_input_amount: u64,
) -> Result<IoAmounts>
where
    F: Fn(u64) -> u64,
{
    for _ in 0..max_step_num {
        let next_input_amount = io_amounts.input_amount + step_range_amount;
        if next_input_amount > max_input_amount {
            break;
        }

        let next_io_amounts =
            calc_amounts_output(next_input_amount, &swap_and_directions, &loan_fee_calc)?;

        if next_io_amounts.output_profit_amount() > io_amounts.output_profit_amount() {
            io_amounts = next_io_amounts
        } else {
            break;
        }
    }

    Ok(io_amounts)
}

fn subtract_fee_from_output(output_amount: u64, fee: u64) -> u64 {
    if output_amount < fee {
        0
    } else {
        output_amount - fee
    }
}
fn calc_amounts_output<'a, F>(
    input_amount: u64,
    swap_and_directions: &[CoinPairAndSwapDirection<'a>],
    loan_fee_calc: F,
) -> Result<IoAmounts>
where
    F: Fn(u64) -> u64,
{
    let step_outputs = calc_amounts(input_amount, swap_and_directions)?;
    let output_amount = step_outputs.last().unwrap();
    //println!(
    //    "input amount:{}, output amount:{}",
    //    input_amount, output_amount
    //); //TODO
    let loan_fee = loan_fee_calc(input_amount);
    let output_amount_subtract_fee = subtract_fee_from_output(*output_amount, loan_fee);

    Ok(IoAmounts {
        input_amount,
        output_amount_after_subtract_fee: output_amount_subtract_fee,
        step_outputs,
    })
}

fn calc_amounts<'a>(
    input_amount: u64,
    swap_and_directions: &[CoinPairAndSwapDirection<'a>],
) -> Result<Vec<u64>> {
    let mut result = Vec::with_capacity(swap_and_directions.len());
    let mut input_amount = input_amount;
    for each_swap_and_direction in swap_and_directions {
        if input_amount == 0 {
            result.push(0);
        } else {
            let output_amount = each_swap_and_direction.calc_amount_out(input_amount)?;
            result.push(output_amount);
            input_amount = output_amount;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod test {

    use super::*;

    macro_rules! io_amounts_inputs {
        ($small:expr,$middle:expr,$greater:expr) => {
            IoAmountsPoints {
                smaller: IoAmounts {
                    input_amount: $small,
                    output_amount_after_subtract_fee: 0,
                    step_outputs: vec![],
                },
                middle: IoAmounts {
                    input_amount: $middle,
                    output_amount_after_subtract_fee: 0,
                    step_outputs: vec![],
                },
                greater: IoAmounts {
                    input_amount: $greater,
                    output_amount_after_subtract_fee: 0,
                    step_outputs: vec![],
                },
            }
        };
    }

    #[test]
    fn test_optimize_is_flat() {
        {
            let input = io_amounts_inputs!(1, 1, 1);
            assert!(input.is_flat())
        }

        {
            let input = io_amounts_inputs!(1, 1, 2);
            assert!(input.is_flat())
        }

        {
            let input = io_amounts_inputs!(1, 2, 3);
            assert!(input.is_flat())
        }

        {
            let input = io_amounts_inputs!(1, 2, 4);
            assert!(!input.is_flat())
        }

        {
            let input = io_amounts_inputs!(2, 6, 8);
            assert!(!input.is_flat())
        }
    }
}
