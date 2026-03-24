# oinori の技術的工夫 — Aptos MEV Bot の実装戦略

oinoriはAptos上のアトミックアービトラージBotであり、MEVの性質上、**高速なトランザクション取得**、**安定した接続維持**、**高速なルート探索**、そして**効率的なオンチェーン実行**が収益に直結する。本記事では、oinoriが施した4つの主要な技術的工夫について解説する。

---

## 工夫1: コネクションID回転とNoiseプロトコルのハック

### 課題: バリデータからのban

oinoriはバリデータに対してAptosの独自P2Pプロトコルで通信を行う。MEVボットの性質上、シミュレーション失敗やタイミングの問題で**エラーを含むリクエストが高頻度で発生する**。Aptosのバリデータは、過度なエラーリクエストを送信してくるクライアントを検知すると、そのクライアントからの接続を遮断（rate limit / ban）する仕組みを持っている。

通常のfullnodeは固定のidentity（x25519鍵ペア）で接続するため、一度banされると復帰が難しい。

### 解決策: ランダムIDによる定期再接続

oinoriの解決策は、**接続のたびに新しいx25519秘密鍵をランダム生成し、異なるPeerIDで再接続する**というものである。これにより、バリデータ側からは常に異なるクライアントに見える。

#### 実装の全体像

```
ConnectivityManager
  │
  ├── oinori_new_self_peer_id()     ← 毎回ランダムな x25519鍵 + PeerID を生成
  ├── oinori_peer_connected_log     ← 各ピアとの接続時刻を記録
  ├── oinori_reconnect_ticker       ← 3秒間隔の再接続タイマー
  └── oinori_reconnect_peers()      ← 10秒経過した接続を切断→新IDで再接続
        │
        ▼
  ConnectionRequestSender::dial_peer(self_peer_private_key, self_peer_id, ...)
        │
        ▼
  AptosNetTransport::dial(oinori_self_peer_private_key, oinori_self_peer_id, ...)
        │
        ▼
  oinori_upgrade_outbound()          ← カスタムNoiseハンドシェイク
        │
        ▼
  NoiseUpgrader::oinori_upgrade_outbound()  ← カスタム秘密鍵でNoise IKハンドシェイク実行
```

#### なぜNoiseプロトコルのハックが必要か

Aptosのノード間通信は[Noise Protocol Framework](https://noiseprotocol.org/)のIKパターンで暗号化されている。通常の`upgrade_outbound`では、ノード起動時に固定された自身の秘密鍵（`self.noise_config`）でハンドシェイクを行う。

oinoriでは接続ごとにIDを変えたいため、**ハンドシェイク時に使う秘密鍵をパラメータとして外部から渡せるようにした**。これが`oinori_upgrade_outbound`メソッドである。

```rust
// 通常のupgrade_outbound: 固定のself.noise_configを使用
pub async fn upgrade_outbound(&self, socket, remote_peer_id, remote_public_key, ...)

// oinori版: 任意の秘密鍵をパラメータとして受け取る
pub async fn oinori_upgrade_outbound(&self, socket,
    self_peer_private_key: x25519::PrivateKey,  // ← 毎回異なるランダム鍵
    self_peer_id: PeerId,                        // ← 鍵から導出したPeerID
    remote_peer_id, remote_public_key, ...)
```

内部では:
```rust
// oinori: 渡された秘密鍵で新しいNoiseConfigを生成
let self_noise_config = noise::NoiseConfig::new(self_peer_private_key);

// この一時的なNoiseConfigでIKハンドシェイクを実行
let initiator_state = self_noise_config
    .initiate_connection(&mut rng, prologue_msg, remote_public_key, ...)?;
```

#### 改修範囲

この変更は、aptos-coreのネットワークスタック全層にわたる:

| レイヤー | ファイル | 変更内容 |
|---|---|---|
| 暗号化 | `crates/aptos-crypto/src/x25519.rs` | `PrivateKey`に`Clone`を追加 |
| Noiseハンドシェイク | `network/framework/src/noise/handshake.rs` | `oinori_upgrade_outbound`メソッド追加 |
| Transport | `network/framework/src/transport/mod.rs` | `oinori_upgrade_outbound`関数追加、`dial`シグネチャ変更 |
| Transport trait | `network/netcore/src/transport/mod.rs` | `Transport::dial`にID引数追加 |
| PeerManager | `network/framework/src/peer_manager/mod.rs` | `DialPeer`にID引数追加 |
| PeerManager型 | `network/framework/src/peer_manager/types.rs` | `ConnectionRequest::DialPeer`型拡張 |
| Sender | `network/framework/src/peer_manager/senders.rs` | `dial_peer`シグネチャ変更 |
| NetworkSender | `network/framework/src/protocols/network/mod.rs` | `dial_peer`シグネチャ変更 |
| ConnectivityManager | `network/framework/src/connectivity_manager/mod.rs` | 再接続ロジック一式追加 |

**oinoriの実装で最も工夫した箇所**であり、最も改修コストが高かった部分でもある。

#### Colocationによる物理レイテンシ削減

oinoriでは、プロトコル改修だけでなく**サーバ配置そのもの**でもレイテンシを削っている。運用時は、**validator数の多いEurope圏のAWS EC2上にノードを配置し、接続・取得・送信の対象もEurope validatorに寄せる**ことで、物理距離由来のRTTを短くしている。

これは「ban回避」のためのID回転とは別の工夫であり、目的はより単純で、**トランザクション取得とbroadcastを近いvalidator群に寄せることによる速度向上**である。

実装上は`ConnectivityManager::choose_peers_to_dial()`でeligible peerを列挙した後、`oinori_filter_sort_eligible_peers()`で接続対象を地域でフィルタする:

- **本番運用のデフォルト**: Europe validatorのみを対象にする
- **ローカル環境**: 環境変数`KOREAN_VALIDATOR=1`を設定すると、韓国・日本・シンガポール近辺のvalidator群を対象に切り替える

この切り替えは、ローカル開発時には日本から近いvalidatorに寄せ、本番では**validator数が多く競争上有利なEurope圏にノードを置く**ためのものだと理解すると全体像が掴みやすい。

---

## 工夫2: DataSummaryを迂回する独自トランザクションストリーム

### 課題: 標準state-syncのDataSummaryによる制約

Aptosの標準的なstate-sync（状態同期）パイプラインは以下の構造で動作する:

```
DataStreamingService → StreamEngine → AptosDataClient → ピア
                         ▲
                         │
                    GlobalDataSummary  ← DataSummaryPollerがピアから定期収集
                         ▲
                         │
                    StorageServerSummary / DataSummary  ← 各ピアが公開
```

ここで重要なのが **`DataSummary`** 構造体（`state-sync/storage-service/types/src/responses.rs`）である:

```rust
pub struct DataSummary {
    /// ストレージ内の最高同期済みバージョンに対応するledger info
    pub synced_ledger_info: Option<LedgerInfoWithSignatures>,
    pub epoch_ending_ledger_infos: Option<CompleteDataRange<Epoch>>,
    pub states: Option<CompleteDataRange<Version>>,
    pub transactions: Option<CompleteDataRange<Version>>,
    pub transaction_outputs: Option<CompleteDataRange<Version>>,
}
```

各ピアは自身の`StorageServerSummary`（`DataSummary`を含む）をネットワークに公開する。`DataSummaryPoller`がこれを定期的に収集し、`GlobalDataSummary`としてまとめる。

`StreamEngine`（`ContinuousTransactionOutputStreamEngine`）はトランザクションストリームのターゲットを決定する際、この`GlobalDataSummary`を参照する:

```rust
// state-sync/data-streaming-service/src/stream_engine.rs:489-494
if let Some(highest_synced_ledger_info) = advertised_data.highest_synced_ledger_info() {
    let (next_request_version, _) = self.next_request_version_and_epoch;
    if next_request_version > highest_synced_ledger_info.ledger_info().version() {
        Ok(None) // ターゲットなし → リクエストしない
    } else {
        Ok(Some(highest_synced_ledger_info))
    }
}
```

**この仕組みにはMEVにとって致命的な制約がある:**

1. **逐次処理**: `StreamEngine`はバージョンを順番に処理する。中間バージョンをスキップして最新に飛ぶことができない
2. **ポーリング遅延**: `DataSummaryPoller`がピアの`StorageServerSummary`を収集するまでにラグがある。ピアが最新データを持っていても、`GlobalDataSummary`に反映されるまでリクエストが発行されない
3. **`AptosDataClient`レベルの制約**: リクエスト送信時にもピア選択で`StorageServerSummary.can_service()`が評価され、ピアの公開済みデータ範囲外のリクエストは送信先が見つからない

**要するに: 標準パイプラインでは、ネットワーク上で「公開済み」かつ「同期済み」のデータしかリクエストできず、最新のトランザクションを最速で取得することが構造的にできない。**

### 解決策: StreamEngineを迂回する独自ストリーム

oinoriの`transaction_stream`モジュール（`oinori/oinori-mev/src/transaction_stream/`）は、`DataStreamingService`/`StreamEngine`を**完全に迂回**し、`AptosDataClient`のAPIを直接呼び出す。

```
oinori transaction_stream
  │
  ├── TransactionOutputStreamStatus  ← 独自の状態管理
  │     latest_fetched_version        ← 最新取得済みバージョン
  │     flying_request_num            ← 同時リクエスト数
  │
  ├── start_tx_stream()              ← メインループ（tick + notify の二重トリガー）
  │
  └── AptosDataClient                ← StreamEngineを経由せず直接呼び出し
        .get_transaction_outputs_with_proof(epoch, start, end, 45ms)
```

#### 核心: DataSummaryに依存しないバージョン管理

```rust
// oinori/oinori-mev/src/transaction_stream/stream_status.rs
fn as_param_versions(&self, latest_fetched_version: u64) -> (u64, u64, u64) {
    let start_version = latest_fetched_version + 1;
    let end_version = start_version + self.additional_get_tx_num as u64;
    let epoch_version = end_version;
    (start_version, end_version, epoch_version)
}
```

oinoriは**自身の`latest_fetched_version`だけを基準に**、次に取得すべきバージョン範囲を決定する。`GlobalDataSummary`も`DataSummary`も一切参照しない。`additional_get_tx_num`分だけ先のバージョンを**投機的に**リクエストする。

- リクエスト先のバリデータがそのバージョンまで同期済みであれば、データが返る
- まだ同期されていなければエラーが返る → 45msタイムアウトで即座にリトライ

#### 高速化の仕組み

1. **二重トリガー方式**: 定期的なtickに加え、前回のフェッチ完了をnotifyシグナルとして即座に次のフェッチを開始する。これにより、tick間隔に依存しない連続フェッチが可能
2. **並行リクエスト**: `flying_request_num`で同時リクエスト数を管理し、複数のフェッチを並行実行
3. **超短タイムアウト**: 45ms（`CLIENT_TIMEOUT_MS`）。応答がなければすぐに次のリクエストへ
4. **確率的フェッチサイズ**: `stochastic_additional_tx_fetch_size`オプションで、フェッチサイズにランダム幅を持たせる。これにより、固定パターンの検出を避けつつ、異なるバージョン範囲を投機的にカバーする

#### エラーハンドリング

投機的リクエストのため、エラーは頻発する。oinoriはこれを想定した上で、ノイズとなるエラーを抑制している:

```rust
// "cannot be satisfied" → ピアにデータがない（想定内）
// "Rpc timed out"       → タイムアウト（想定内）
// "dropped response"    → 応答ドロップ（想定内）
if !err_msg.contains("cannot be satisfied")
    && !err_msg.contains("Rpc timed out")
    && !err_msg.contains("dropped response")
{
    println!("---- oinori stream request error ...");
}
```

---

## 工夫3: ルートフィルタリングと最適入力量探索

### 課題: 全探索は時間的に不可能

アトミックアービトラージでは、トランザクションで変動したスワッププールの状態から利益の出るルート（APT → コインA → コインB → APT のような循環経路）を見つける必要がある。しかし複数DEX×複数ペアの全組み合わせを探索すると、**15msのタイムアウト内に間に合わない**。

### 解決策1: 変動パターンに基づくルートフィルタリング (`route_filter`)

oinoriは、トランザクションが**どのスワップリソースをどう変化させたか**を分析し、探索範囲を事前に絞り込む。

```
トランザクション → affected_swap_updates（変動したスワップ一覧）
  │
  ├── 1件 → single_affect:  変動DEX + コインの増減方向から3ステップのどこに配置するか決定
  ├── 2件 → two_affect:     チェインルート検出 + 逆順チェック付きショートカット判定
  └── 3件以上 → multiple_affect: APT増価/減価/非APTペアに分類して集約
```

核心は**コインの「増価 (appreciate)」「減価 (depreciate)」の方向**を見ること:

- **APTが増価**（プールからAPT流出）→ そのDEXを**ルート起点**（Step1）に配置。APTで他コインを安く買える
- **APTが減価**（プールにAPT流入）→ そのDEXを**ルート終点**（Step3）に配置。他コインでAPTを安く買い戻せる
- **非APTペア** → **中間ステップ**（Step2）に配置

```rust
// 3ステップルートの各ステップにフィルタを設定
SearchRouteCondition {
    swap_filters_for_steps: Some((Some(Liquid), None, Some(Pancake))),
    //                             ↑ Step1       Step2    ↑ Step3
    coin_filters_for_steps: (first_filters, second_filters, third_filters),
    input_amounts: vec![200 APT, 6 APT, 0.2 APT],
}
```

APTが直接関与するペアの変動は利益確度が高いため、**eager入力**（200 APT含む3パターン）で探索する。

2件変動時には**チェインルート検出**も行う。「DEX1: APT→USDC, DEX2: USDC→APT」のように出力コインが次の入力コインに連鎖するパターンを検出し、Step1=DEX1, Step2=DEX2と直接指定する。

Thalaは複数プール間の連動が複雑なため、Thala関連の変動を検出した場合は**フィルタリングを無効化**し、全探索にフォールバックする。

### 解決策2: 並行探索 + タイムアウト付きベスト選択

`find_route.rs`の`find_best_route`は、**複数の入力金額 × 各DEXの開始ペア**を組み合わせて並行に探索する:

```
入力金額 [200APT, 6APT, 0.2APT] × DEX起点 [Liquid, Pancake, Cetus, ...]
  │
  ├── 各組み合わせを tokio::task::spawn で並行起動
  ├── 再帰的にmax_depth=3まで深さ優先探索
  └── 見つかったルートを unbounded_channel で集約
        │
        ▼
   tokio::select! で 15ms タイムアウト or 全完了 → ベストルート選択
```

タイムアウト内に見つかった中から最も利益の大きいルートを採用する。全探索の完了を待たない。

### 解決策3: 二段階の入力量最適化

ベストルートが見つかった後、**入力金額を最適化**して利益を最大化する:

```
1. バイナリ探索（最大30回）
   smaller = input/2, middle = input, greater = input*2
   → 利益が最大となるinputの範囲を絞り込む

2. リニア探索（最大500ステップ × 0.3 APT刻み）
   → バイナリ探索の結果から微調整し、最終的な最適入力量を決定
```

利益関数はAMM曲線の性質上単峰でないこともあるため、三点（smaller/middle/greater）の利益比較で探索方向を動的に切り替える。

### フラッシュローンの動的選択

最適入力量が手持ちAPTを超える場合、**フラッシュローン**を使用する。ルート内でLiquidSwapのどのペアを使っているかを確認し、衝突しないカウンターコイン（USDC→USDT→BTC→APTOGE）の順で自動選択する。

---

## 4つの工夫の関係

これら4つの工夫は、MEVパイプライン全体で連携している:

```
工夫1: コネクションID回転       工夫2: 独自トランザクションストリーム
  「安定した接続を維持する」       「最新データを最速で取得する」
        │                              │
        │  高頻度エラーリクエスト ←───────┘
        │  （投機的フェッチの副作用）       │
        ▼                              ▼
  ID回転でbanを回避              最新TXを取得
        │                              │
        └──── 接続維持 ─────────────────┘
                                        │
                                        ▼
                              工夫3: ルートフィルタリング
                              「15ms以内に最適ルートを発見する」
                                        │
                                 変動パターン分析 → 探索範囲絞込 → 並行探索 → 入力量最適化
                                        │
                                        ▼
                              工夫4: Moveスクリプト + フラッシュローン
                              「最適ルートをアトミックかつ資本効率良く実行する」
                                        │
                                 カウンターコイン選択 → スワップ実行 → ローン返済
```

工夫2で最速取得したトランザクションの**スワップリソース変動**が、工夫3のルートフィルタへの入力となる。工夫3は変動パターンを即座に分析して探索空間を絞り込み、15ms以内にアービトラージルートを発見・最適化する。工夫4はその最適ルートを、フラッシュローンにより手持ち資金を超える入力量でもアトミックに実行する。工夫1のID回転がなければ工夫2の投機的フェッチは維持できず、工夫2の高速取得がなければ工夫3の発見したルートは他のボットに先を越される。工夫3が見つけた最適入力量が手持ちAPTを超える場合でも、工夫4のフラッシュローンにより実行可能となる。**4つの工夫が一体となって初めてMEVの競争優位が成立する。**

---

## 工夫4: Moveスクリプトによるアトミック実行とフラッシュローン

### Moveコントラクトの設計思想

oinoriのオンチェーン実行部分は、Move **スクリプト（script）** として実装されている。Moveにはモジュール（module）とスクリプト（script）の2種類のコード単位があるが、oinoriがスクリプトを選択したのは以下の理由による:

- **スクリプトはオンチェーンにデプロイ不要**: モジュールはアカウントにパブリッシュされ、誰でもそのインターフェースを参照できる。スクリプトはトランザクションごとにバイトコードとして送信されるため、**ロジックがオンチェーンに残らない**
- **アトミック実行の保証**: スクリプト内の全操作は単一トランザクションとしてアトミックに実行される。途中で失敗すれば全変更がロールバックされる

### コントラクトの4バリアント構成

oinoriは4種類のコントラクトを持つ:

```
contracts/
  ├── oinori_swap_v1/           ← フラッシュローンあり（pool_type固定=0）
  ├── oinori_swap_v1_no_loan/   ← フラッシュローンなし（pool_type固定=0）
  ├── oinori_swap_v2/           ← フラッシュローンあり（pool_type可変）
  └── oinori_swap_v2_no_loan/   ← フラッシュローンなし（pool_type可変）
```

| バージョン | pool_type | フラッシュローン | 用途 |
|---|---|---|---|
| v1 | 固定（`POOL_TYPE = 0`、AMM only） | あり | Thala以外のAMMプール向け |
| v1_no_loan | 固定（`POOL_TYPE = 0`） | なし | 手持ちAPT内でのアービトラージ |
| v2 | 可変（引数で指定） | あり | Thalaなどpool_typeが異なるDEX対応 |
| v2_no_loan | 可変（引数で指定） | なし | 手持ちAPT内 + Thala対応 |

v1ではpool_typeがコントラクト内定数`POOL_TYPE = 0`で固定されているため、全ステップがAMMプールのみに制限される。v2ではpool_typeをステップごとに引数として受け取るため、Thalaのstable poolなど異なるpool_typeにも対応できる。

### スクリプトの型パラメータ

```move
fun main<
    Y,   // Step1の出力コイン（= Step2の入力コイン）
    Z,   // Step2の出力コイン（= Step3の入力コイン）
    C1,  // Step1のAMMカーブ型
    C2,  // Step2のAMMカーブ型
    C3,  // Step3のAMMカーブ型
    BY,  // フラッシュローンのカウンターコイン型
    BC   // フラッシュローンプールのカーブ型
>(...)
```

入力コイン（X）と最終出力コイン（OutCoin）はともに`0x1::aptos_coin::AptosCoin`にハードコードされている。これはアトミックアービトラージの特性上、APTで開始しAPTで終了する循環経路しか扱わないためである。

### フラッシュローンの仕組み

#### DeFiにおけるフラッシュローンとは

フラッシュローンは**単一トランザクション内で借入・返済を完了させる無担保ローン**である。担保が不要な代わりに、同一トランザクション内で返済できなければトランザクション全体が失敗（ロールバック）する。

#### Liquidswapのフラッシュローン実装

oinoriはLiquidswap（Pontem DEX）のフラッシュローン機能を利用する。その核心は **`Flashloan`リソース** にある:

```move
/// コピー不可、保存不可、ドロップ不可のリソース
struct Flashloan<phantom X, phantom Y, phantom Curve> {
    x_loan: u64,
    y_loan: u64
}
```

このリソースはMoveの**アビリティ制約**により、`copy`（複製不可）、`store`（永続保存不可）、`drop`（暗黙破棄不可）のいずれも持たない。つまり:

- コピーして二重に使うことはできない
- グローバルストレージに保存して別トランザクションで使うことはできない
- **何もせずに捨てることができない** → `pay_flashloan`で消費する以外に処理する方法がない

```move
// 借入: Flashloanリソースが返される
public fun flashloan<X, Y, Curve>(x_loan: u64, y_loan: u64)
    : (Coin<X>, Coin<Y>, Flashloan<X, Y, Curve>)
{
    // ...プールからコインを引き出す
    pool.locked = true;  // リエントランシー防止
    (x_loaned, y_loaned, Flashloan<X, Y, Curve> { x_loan, y_loan })
}

// 返済: Flashloanリソースを消費（destructure）する
public fun pay_flashloan<X, Y, Curve>(
    x_in: Coin<X>, y_in: Coin<Y>, loan: Flashloan<X, Y, Curve>
) {
    let Flashloan { x_loan, y_loan } = loan;  // ← リソースを分解・消費
    // ...LP値が減少していないことを検証
    pool.locked = false;
}
```

**`Flashloan`リソースはトランザクション内で必ず`pay_flashloan`に渡して消費しなければならない。** これがMoveの型システムによるフラッシュローンの安全性保証であり、Solidity（EVM）の`require`ベースのチェックとは根本的に異なるアプローチである。

#### oinoriスクリプト内のフラッシュローンフロー

フラッシュローン版スクリプト（v1/v2）の実行フローは以下の通り:

```
1. flashloan<BY, AptosCoin, BC>(0, coin_amount_in)
   │  → APTを借入、Flashloanリソースを受取
   │  → プールがロック（リエントランシー防止）
   │
2. coin::deposit(signer, coin_borrowed)
   │  → 借りたAPTを自身のアカウントにデポジット
   │
3. aggregator::swap_direct<AptosCoin, Y, Z, AptosCoin, C1, C2, C3>(...)
   │  → 最大3ステップのDEXスワップを実行
   │  → APT → Y → Z → APT の循環経路
   │
4. assert!(output >= min_out_amount)
   │  → 最低出力量の検証
   │
5. loan_fee = borrowed_amount * 31 / 10000   （手数料 0.31%）
   repay_amount = borrowed_amount + loan_fee
   │  → フラッシュローン手数料の計算
   │
6. 中間コイン（Y, Z の残余）をアカウントにデポジット
   │
7. coin::withdraw<AptosCoin>(signer, repay_amount)
   │  → 返済額をアカウントから引き出し
   │
8. pay_flashloan(coin_zero, repay_coin, loan)
      → Flashloanリソースを消費して返済完了
      → プールのロック解除
```

`coin_borrowd_zero`（変数名はtypoだが原文ママ）はフラッシュローン呼び出し時のX側コイン（カウンターコイン）であり、借入額0で取得されるため空のコインである。返済時にはこの空コインと、実際の返済APTコインをそれぞれ`pay_flashloan`に渡す。

### カウンターコインの動的選択

フラッシュローンはLiquidswapの流動性プールから借入するため、`flashloan<BY, AptosCoin, BC>`の`BY`にはプール内のカウンターコイン型を指定する必要がある。oinoriではアービトラージルート内でLiquidswapのどのペアが使われているかを確認し、**フラッシュローンのプールとスワップのプールが衝突しないように**カウンターコインを動的に選択する:

```rust
// oinori/oinori-mev/src/engine/find_route.rs
fn loan_info(best_route, swap_router_pool, not_use_loan_max_amount)
    -> (u64, LoanCounterCoin)
{
    // ルート内でLiquidswapの各ペアが使われているか確認
    let (has_usdc_pair, has_usdt_pair, has_btc_pair, has_aptoge_pair) = ...;

    // 衝突しないペアを優先度順に選択
    if !has_usdc_pair {
        (32000_0000_0000, LoanCounterCoin::Usdc)    // USDC/APTプールから最大32000APT
    } else if !has_usdt_pair {
        (2000_0000_0000, LoanCounterCoin::Usdt)     // USDT/APTプールから最大2000APT
    } else if !has_btc_pair {
        (1936_0000_0000, LoanCounterCoin::Btc)      // BTC/APTプールから最大1936APT
    } else if !has_aptoge_pair {
        (1880_0000_0000, LoanCounterCoin::Aptoge)   // APTOGE/APTプールから最大1880APT
    } else {
        (not_use_loan_max_amount, LoanCounterCoin::Usdc)  // 全て衝突 → 手持ち資金のみ
    }
}
```

USDC/APTプールが最も流動性が高いため最優先で選択され、最大32,000 APTの借入が可能。ルート内でLiquidswapのUSDC/APTペアを使用している場合は、プールのロック機構によりフラッシュローンとスワップが衝突するため、次の候補（USDT）にフォールバックする。

### DEXアグリゲーターインターフェース

スワップの実行には**Hippo Aggregator**のインターフェースを利用する。このアグリゲーターは複数DEXへのルーティングを統一的に扱う:

```move
// DEX識別子
const DEX_PONTEM: u8 = 3;   // Liquidswap
const DEX_CETUS: u8 = 10;   // Cetus
const DEX_PANCAKE: u8 = 11; // PancakeSwap
const DEX_OBRIC: u8 = 12;   // Obric
// ... 他多数

// 1〜3ステップのルーティング
public fun swap_direct<X, Y, Z, OutCoin, E1, E2, E3>(
    num_steps: u8,        // 1, 2, または 3
    first_dex_type: u8,   // Step1のDEX
    first_pool_type: u64, // Step1のプール種別
    first_is_x_to_y: bool,
    // ... Step2, Step3 の引数
    x_in: Coin<X>
): (Option<Coin<X>>, Option<Coin<Y>>, Option<Coin<Z>>, Coin<OutCoin>)
```

`swap_direct`は内部で`get_intermediate_output`を再帰的に呼び出し、各ステップのDEXに応じたスワップを実行する。中間コインは`Option<Coin<T>>`として返され、未使用ステップのコインは`option::destroy_none`で安全に破棄される。

### no_loan版との違い

フラッシュローンなし版（`oinori_swap_v1_no_loan`、`oinori_swap_v2_no_loan`）は、借入・返済ロジックが一切なく、ユーザーの手持ちAPTでスワップを実行する:

```move
// no_loan版: フラッシュローンなし
script {
    use hippo_aggregator::aggregator;

    fun main<Y, Z, C1, C2, C3, BY, BC>(
        signer: &signer, coin_amount_in: u64, min_out_amount: u64, ...
    ) {
        // 手持ちAPTからスワップ実行
        aggregator::swap<AptosCoin, Y, Z, AptosCoin, C1, C2, C3>(
            signer, num_steps, ...
            coin_amount_in, min_out_amount
        );
    }
}
```

Rustエンジン側で入力量が手持ちAPTを超える場合にフラッシュローン版を、超えない場合にno_loan版を使い分ける:

```rust
// oinori/oinori-mev/src/engine/find_route.rs
let (use_loan, loan_fee) = if input_amount > not_use_loan_max_amount {
    (true, calc_flashloan_fee(input_amount))  // 0.31%の手数料
} else {
    (false, 0)  // 手数料なし
};
```

フラッシュローン手数料（0.31%）はRust側でも事前に計算され、利益計算に組み込まれる:

```rust
// oinori/oinori-mev/src/swap/specifics/liquid/flashloan.rs
pub fn calc_flashloan_fee(amount: u64) -> u64 {
    amount * 31 / 10_000
}
```

### 実行直前のmempool最適化

アトミックアービトラージでは、ルートを見つけた後の**送信経路の遅延**も勝率に直結する。oinoriはここでもAptos標準の安全寄りの処理を一部緩め、**自ノードのinternal mempoolに投入した直後に、選択したバリデータピアへ即時broadcast**するようにしている。

```
oinori engine
  │
  └── MempoolClientRequest::SubmitTransaction(...)
        │
        ▼
   shared mempool
     1. local mempoolに挿入
     2. 低レイテンシpeerを選択
     3. 即時broadcast
```

通常のAptos mempoolは、クライアント送信トランザクションに対してVM validationや重複更新チェックを行い、その後に通常のbroadcastスケジュールに乗せる。oinoriでは、このうち**mempool投入前の高コストな検証をかなり省略**している。

- **VM validationの省略**: `shared_mempool::validate_and_add_transactions()`では、本来行うVM validationの代わりにハッシュ計算とサイズ計算だけを行い、`status=None`の擬似的な成功結果を返してそのままmempoolへ入れる
- **重複トランザクション更新チェックの省略**: `transaction_store.insert()`にあった「同一アカウント・同一シーケンス番号の既存TXとpayload/max_gas/expirationが一致するか」を見る重複チェックを削除
- **即時broadcast**: `SubmitTransaction`処理の完了直後に、oinori用に選んだ低レイテンシpeerへ`execute_broadcast()`を呼び、通常の周期broadcastを待たずに直接流す

ただし、**全てのチェックを消しているわけではない**。少なくとも以下は残している:

- **古いシーケンス番号の拒否**: `txn.sequence_number() < db_sequence_number` の場合は拒否
- **アカウント単位の容量上限**: `capacity_per_user`を超える場合は拒否

要するに、oinoriは「最終的にはvalidator側で検証される」ことを前提に、**自ノードmempoolでの事前検証コストを削り、伝播開始を最優先にした**。これはフラッシュローンの有無に関係なく、`fire_found_route_contract()`から送るすべてのアービトラージトランザクションに共通する実行最適化である。

### Moveの型安全性が果たす役割

oinoriのコントラクト設計において、Moveの型システムは以下の安全性を自動的に保証する:

1. **フラッシュローンの返済保証**: `Flashloan`リソースはdropできないため、`pay_flashloan`で消費しなければコンパイルエラー
2. **コインの保全**: `Coin<T>`もdropできないため、全コインが明示的にdepositまたは消費される必要がある。コインの「消失」がコンパイル時に防止される
3. **リエントランシー防止**: プールのロックフラグにより、フラッシュローン中に同じプールへの追加操作が禁止される
4. **型パラメータによるコインペアの正確性**: ジェネリクスにより、Rustエンジンが構築した型情報がそのままオンチェーン実行に使われ、型の不一致はVM実行時に即座に拒否される

---

## まとめ

| 工夫 | 課題 | 解決策 | 改修範囲 |
|---|---|---|---|
| コネクションID回転 | バリデータからのban | Noiseハンドシェイクにカスタム秘密鍵を注入し、定期的にIDを変えて再接続。加えてEurope/Korean validatorへの地域限定接続で物理レイテンシも削減 | ネットワークスタック全層（9ファイル） |
| 独自トランザクションストリーム | DataSummaryによる最新TX取得の制約 | StreamEngineを迂回し、AptosDataClientを直接使用して投機的にフェッチ | oinori-mev内の独自モジュール |
| ルートフィルタリング + 最適化 | 全探索が15ms制約に収まらない | 変動パターンでの探索枝刈り + 並行探索 + 二段階入力量最適化 | route_filter/, find_route.rs, optimization.rs |
| Moveスクリプト + フラッシュローン | 手持ち資金を超える入力量での実行 | Liquidswapフラッシュローンの動的カウンターコイン選択 + 4バリアントのスクリプト構成 + mempool事前検証の省略と即時broadcast | contracts/oinori_swap_v{1,2}{,_no_loan}/, mempool/, state-sync/ |

これらの工夫により、oinoriは**低レイテンシのトランザクション取得**から**高速なルート探索・最適化**、そして**フラッシュローンを活用した資本効率の最大化**までを一貫して実現し、アービトラージ機会の検出・実行速度で競争優位を確保していた。
