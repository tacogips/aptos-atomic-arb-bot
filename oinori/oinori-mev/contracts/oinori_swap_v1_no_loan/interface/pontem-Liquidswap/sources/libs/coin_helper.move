/// The `CoinHelper` module contains helper funcs to work with `AptosFramework::Coin` module.
module liquidswap::coin_helper {
    use aptos_std::comparator::{Self, Result};
    use aptos_std::type_info;

    // Errors codes.

    /// When both coins have same names and can't be ordered.
    const ERR_CANNOT_BE_THE_SAME_COIN: u64 = 3000;

    /// When provided CoinType is not a coin.
    const ERR_IS_NOT_COIN: u64 = 3001;

    // Constants.
    /// Length of symbol prefix to be used in LP coin symbol.
    const SYMBOL_PREFIX_LENGTH: u64 = 4;


    /// Compare two coins, `X` and `Y`, using names.
    /// Caller should call this function to determine the order of X, Y.
    public fun compare<X, Y>(): Result {
        let x_info = type_info::type_of<X>();
        let y_info = type_info::type_of<Y>();

        // 1. compare struct_name
        let x_struct_name = type_info::struct_name(&x_info);
        let y_struct_name = type_info::struct_name(&y_info);
        let struct_cmp = comparator::compare(&x_struct_name, &y_struct_name);
        if (!comparator::is_equal(&struct_cmp)) return struct_cmp;

        // 2. if struct names are equal, compare module name
        let x_module_name = type_info::module_name(&x_info);
        let y_module_name = type_info::module_name(&y_info);
        let module_cmp = comparator::compare(&x_module_name, &y_module_name);
        if (!comparator::is_equal(&module_cmp)) return module_cmp;

        // 3. if modules are equal, compare addresses
        let x_address = type_info::account_address(&x_info);
        let y_address = type_info::account_address(&y_info);
        let address_cmp = comparator::compare(&x_address, &y_address);

        address_cmp
    }

    /// Check that coins generics `X`, `Y` are sorted in correct ordering.
    /// X != Y && X.symbol < Y.symbol
    public fun is_sorted<X, Y>(): bool {
        let order = compare<X, Y>();
        assert!(!comparator::is_equal(&order), ERR_CANNOT_BE_THE_SAME_COIN);
        comparator::is_smaller_than(&order)
    }

}
