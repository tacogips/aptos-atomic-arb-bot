# oinori-mev engine モジュール

MEVエンジンのコア。トランザクション監視→機会検出→ルート最適化→取引実行までの全パイプラインを管理する。

---

## `oinori/oinori-mev/src/engine/mod.rs`
- **目的**: MEVエンジンのメイン構造体とトランザクション処理ループ

### `Engine`構造体
エンジンの状態を保持:
- 送信者情報（秘密鍵・シーケンス番号）
- APT残高
- 利用可能なスワップ一覧
- トランザクションベースペイロード（フラッシュローンあり/なし）
- 直近MEVキャッシュ

### `start_mev_engine()`
- **目的**: MPSCチャネルからユーザートランザクションを受信し、各トランザクションに対して`run_mev()`を実行するメインループ
- **ロジック**: `AtomicU64`による処理済みバージョン追跡で重複処理を防止

---

## `oinori/oinori-mev/src/engine/mev_runner/mod.rs`
- **目的**: MEV機会検出・実行のメインパイプライン

### `run_mev()`
- **目的**: 個々のユーザートランザクションからアービトラージ機会を検出し実行する
- **ロジック**:
  1. **プール更新**: `swap_pool.update_pool_with_resources()`でユーザーTXのスワップ変更を反映
  2. **フィルタ生成**: `route_filter_by_affected_resource()`で影響を受けたスワップペアから探索条件を決定
  3. **Thala除外**: Thalaが関与しない場合、探索対象から除外（不要な探索を防止）
  4. **ルート探索**: `search_route()`で利益のあるルートを探索
     - `threshold_profit = 0.001 APT`
     - `loan_threshold = 90 APT`
     - `input_amounts = [200, 6, 0.2] APT`（eager）or `[6, 0.2] APT`
  5. **実行**: `fire_found_route_contract()`でトランザクション送信

### `fire_found_route_contract()`
- **目的**: 発見したルートの安全性チェックと実行
- **ロジック**:
  1. **キャッシュチェック**: 同一ルート（入力額・出力額・使用コントラクト）がキャッシュに存在するか確認
  2. **レートリミット**: 1秒以内の複数コントラクト呼び出しをスキップ
  3. **残高ガード**: アカウント残高 < 9 APTの場合スキップ（資金損失防止）
  4. **実行**: `call_contacat()`でトランザクション送信
  5. **キャッシュ更新**: 実行したルートをキャッシュに記録

---

## `oinori/oinori-mev/src/engine/find_route.rs`
- **目的**: アービトラージルートの発見と利益検証

### `search_route()`
- **目的**: ルート探索→ローン戦略→金額最適化→利益計算の一連のフローを実行
- **ロジック**:

  **1. ルート探索**:
  `find_best_route()`を15msタイムアウト・3ステップ上限で実行

  **2. ローン戦略分析**:
  ルートにフラッシュローンペアが含まれるか分析し、ローンコインを戦略的に選択:
  - USDCペアなし → 最大32000 APT（USDC loan強制）
  - USDTペアなし → 最大2000 APT
  - BTCペアなし → 最大1936 APT
  - APTOGEペアなし → 最大1880 APT

  **3. 金額最適化**:
  `optimize_route_amount_with_loan_fee()`で最適入力額を探索:
  - ステップ幅: 0.3 APT
  - 最大ステップ数: 500（最大150 APT）

  **4. 利益計算**:
  ```
  profit = return_amount - input_amount
  return_amount = output - min(output, loan_fee + gas_fee)
  loan_fee = flashloan手数料（input > thresholdの場合）
  gas_fee = gas_unit_price × estimated_max_gas
  ```

  **5. フィルタリング**:
  - `output ≤ input + loan_fee` → 利益なしとして棄却
  - `profit < 0.001 APT` → 閾値未満として棄却
  - `input > 5000 APT` → 最大制約超過として棄却

  **6. ガスユニット価格計算**:
  - 粗利益の23.5%をガスに充当（大口取引）
  - Thala関与時30%減額
  - 全体に40%の減額ファクター
  - 上限: 0.000011 APT/gas unit
  - 範囲: [100, 642.03] octas

---

## `oinori/oinori-mev/src/engine/call_order_contract.rs`
- **目的**: 署名済みトランザクションのmempool送信と確認追跡

### `call_contacat()`
- **目的**: トランザクションに署名し、選択したピアにブロードキャストする
- **ロジック**:
  1. ロックを取得（並行コントラクト呼び出し防止）
  2. ランダムに2つのピアを選択
  3. 署名済みトランザクションを構築（ガス価格・シーケンス番号・タイムアウト1秒）
  4. `MempoolClientRequest`経由でmempoolに送信
  5. 確認監視タスクを非同期起動

### `start_check_transaction()`
- **目的**: トランザクション確認を監視し、状態を更新する
- **ロジック**:
  - 500msごとにDBをポーリングしてハッシュでトランザクション検索
  - 見つかった場合: シーケンス番号インクリメント、ロック解除
  - タイムアウト（1秒）の場合: RPCから最新シーケンス番号を取得して同期
  - ロックにより、トランザクション決済まで次のコントラクト呼び出しを防止

---

## `oinori/oinori-mev/src/engine/gas_calculation.rs`
- **目的**: ルート構成に基づくガス推定

### `estimate_max_gas()`
- **目的**: ルートに必要なガス総量を推定
- **ロジック**:
  1. `ROUTE_TO_GAS_MAP`で正確なルートコントラクト組み合わせのガスコストを検索
  2. 見つからない場合、ルート長に基づいて推定:
     - 基本: 60ガス（2ステップ）、92ガス（3ステップ以上）
     - Thalaペア: コインペアに応じて100-700ガス追加
     - フラッシュローン: 5ガス追加

---

## `oinori/oinori-mev/src/engine/cache.rs`
- **目的**: 同一ルートの短期間内重複実行を防止するキャッシュ
- **TTL**: 1200秒（20分）
- **キー**: `(入力額, 出力額, 使用コントラクトの順序リスト)`

---

## `oinori/oinori-mev/src/engine/setup_and_keep_sync.rs`
- **目的**: エンジンの初期化とブロックチェーン同期維持

### `start_up_engine()`
- **目的**: エンジンの初期ブートストラップ
- **ロジック**:
  1. 同期完了を待機（`synced_version + 50000 >= remote_version`）
  2. RPCからアカウントシーケンス番号取得
  3. DBからスワッププールをロード
  4. ブートストラップ完了フラグをセット

### `keep_sync_data_diffs()`
- **目的**: 40秒ごとの定期同期
- **ロジック**:
  1. RPCからシーケンス番号同期（最後のMEV実行から30秒以上経過時のみ適用）
  2. DBからAPT残高同期
  3. **停滞検知**: トランザクション受信が5分以上途絶えた場合panicで強制再起動

---

## `oinori/oinori-mev/src/engine/sync_engine.rs`
- **目的**: ノードのブロックチェーン同期状態チェック
- **ロジック**: ローカル同期バージョンとリモートバージョンを50,000ブロックのバッファで比較

---

## `oinori/oinori-mev/src/engine/sender_set.rs`
- **目的**: TOMLファイルからトランザクション送信者の認証情報をロード
