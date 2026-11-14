# oinori-mev engine/route_filter module

Analyzes changes in swap resources and determines the optimal filter conditions for route search. Different filtering logic is applied depending on how many swap pairs are affected (1, 2, or 3+).

---

## `oinori/oinori-mev/src/engine/route_filter/mod.rs`
- **Purpose**: main dispatcher for filtering

### `route_filter_by_affected_resource()`
- **Purpose**: dispatch to the appropriate filter handler based on the number of affected swaps
- **Logic**:
  - 1 item -> `single_affect` module
  - 2 items -> `two_affect` module (with special pattern detection)
  - 3 or more items -> `multiple_affect` module

### `thala_resource_might_affected()`
- **Purpose**: detect Thala coin involvement
- **Logic**: returns `true` if `SwapContract::Thala` or involvement of THL/MOD/ThalaAPT/AmnisApt is detected -> disable filtering (Thala is difficult to pattern-match)

---

## `oinori/oinori-mev/src/engine/route_filter/single_affect.rs`
- **Purpose**: determine filters when a single swap resource changes

### `route_filter_by_affected_resource_single()`
- **Purpose**: decide the search direction based on the characteristics of the changed swap pair
- **Logic**:

  **Case 1: APT appreciation (APT becomes scarcer)**
  - APT decreases (reserve_x/y decreases and on the APT side)
  - First swap filter: set the affected contract
  - Route direction: **APT -> other coin -> ... -> APT** (sell high, buy low)
  - Input amount: [200, 6, 0.2] APT (aggressive search)

  **Case 2: APT depreciation (APT becomes abundant)**
  - APT increases (reserve increases and on the APT side)
  - Second/third swap filters: set the affected contract
  - Route direction: **... -> other coin -> APT** (buy low, sell high)
  - Input amount: [200, 6, 0.2] APT

  **Case 3: non-APT pair**
  - Set the second swap filter
  - Set the input/output coin filter for the second step
  - Input amount: [6, 0.2] APT (normal search)

---

## `oinori/oinori-mev/src/engine/route_filter/two_affect.rs`
- **Purpose**: determine filters when two swap resources change

### `route_filter_by_affected_resource_two()`
- **Purpose**: analyze two change patterns and determine filters using either a special-pattern shortcut or general analysis
- **Logic**:

  **Thala check**: if involved, return no filter (do not constrain exploration)

  **Special-pattern shortcut** (`check_specified_route_short_cut()`):

  1. **Obric double**: both are Obric Other types -> set Obric on the second swap filter

  2. **Chain route**: pattern `depreciate_1 == appreciate_2 && depreciate_2 == APT`
     - Example: APT -> USDC, USDC -> APT
     - Build a dedicated filter for this exact pattern

  3. **Non-APT pair**: both updated coins are non-APT
     - Set the second swap filter and build the intermediate step coin filter

  **General analysis** (when no shortcut matches):

  Classify into three categories:
  - **APT appreciation count**: updates where APT decreases
  - **APT depreciation count**: updates where APT increases
  - **Non-APT pair count**: updates that do not include APT

  Set filters when the count is exactly 1:
  - `apt_appreciation == 1` -> first swap filter
  - `apt_depreciation == 1` -> third swap filter
  - `non_apt_pair == 1` -> second swap filter

---

## `oinori/oinori-mev/src/engine/route_filter/multiple_affect.rs`
- **Purpose**: determine filters when three or more swap resources change

### `route_filter_by_affected_multiple_resource()`
- **Purpose**: analyze multiple changes and determine filters
- **Logic**:
  - Same category analysis as `two_affect` (without shortcuts)
  - When Thala is involved: no filter, input amount [50, 6, 0.2] APT
  - If exactly 1 APT appreciation case -> set the first filter, input amount [50, 6, 0.2] APT
  - If exactly 1 APT depreciation case -> set the third filter
  - If exactly 1 non-APT pair case -> set the second filter

---

## Overall flow diagram

```text
route_filter_by_affected_resource()
  ├─ 1 item -> single_affect
  │    ├─ APT appreciation -> first swap filter (APT->X->...->APT)
  │    ├─ APT depreciation -> third swap filter (...->X->APT)
  │    └─ non-APT -> second swap filter + coin filter
  │
  ├─ 2 items -> two_affect
  │    ├─ Thala involved -> no filter
  │    ├─ shortcut detected -> dedicated filter
  │    └─ general analysis -> filter based on category counts
  │
  └─ 3+ items -> multiple_affect
       ├─ Thala involved -> no filter, expanded input amount
       └─ general analysis -> filter based on category counts
```
