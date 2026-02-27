# 📄 VRM → Second Life glTF 変換ツール仕様書（1体専用）

## 🎯 目的

VRoidで作成したVRMファイルを、Second Life で使用可能なリグ付きglTF 2.0へ変換する。

## 🛑 制約

- 1体専用（汎用性不要）
- 揺れボーン（髪・尻尾・耳・服物理）無視
- 胸揺れ・尻揺れはSecond Lifeのシェイプ仕様依存のため非対応
- 表情（BlendShape）無視
- アニメーションは出力データには含めない（プレビュー再生テスト基盤はv0.8で実装）
- Bento拡張ボーンは、目、まぶた、口、指のみ対応
- シェイプ追従非対応
- Fitted Mesh非対応
- Humanoid主要ボーンのみ対応
- **現時点ではVRoid Studio標準出力のVRMのみ対応**

## 🗺️ ロードマップ

### v0.8（先行実装）：ボーン変換基盤（最優先）

BVHアニメーション再生より先に、VRM骨格をSL想定骨格へ確実に変換する基盤を実装する。

- 必須Humanoidボーンの抽出・検証を完了
- VRM→SLボーン名変換を出力データへ確実に反映
- 変換後骨格（名前・階層・必須ボーン存在）の検証を強化
- Aポーズ→Tポーズ補正とinverseBindMatrices再生成を優先実装
- glTF出力にはアニメーションを埋め込まない（SL側で無視されるため）

### v0.9（次バージョン）：アニメーション再生基盤

v0.8で確定した変換済み骨格を前提に、BVHの読込とプレビュー再生を実装する。

- BVHアニメーションデータを `frontend/public/animations/` に配置
- three.js `BVHLoader` による直接読込を実装
- ボーン名マッピング（SL BVH骨格 ↔ 変換後GLB骨格）を実装
- プレビューでの再生テスト（walk / stand / sit）を実施
- CC BY 3.0に基づく著作権表記の明記

### v1.0（次のバージョン）：尾・耳・羽の動作実装

揺れボーン（髪・尻尾・耳・服物理）を検出・変換して、動作するボーンとして対応。
※ 胸揺れ・尻揺れはSecond Lifeのシェイプ仕様側で扱う前提のため、本ツールでは対応しない。

### v1.1（その次のバージョン）：ヘッド抽出モード

アバター全体ではなく、ヘッド部分のみを抽出して出力するモードを実装。

- ヘッドメッシュのみ抽出
- ヘッド関連ボーン（head, mEyeLeft, mEyeRight, mFaceJaw等）のみ保持
- 独立した.gdbファイルとして出力

## 🧩 入出力仕様

**入力：** VRM 1.0（.vrm ファイル）

**出力：** glTF 2.0形式（.gdbファイル、SLアップロード可能。daeは非対応）

## 🛠 技術スタック（Rust）

**使用クレート：**

- gltf
- serde
- serde_json
- nalgebra
- anyhow
- oxipng（PNG再圧縮用）
- image（テクスチャリサイズ用）

---

## 🔧 変換処理フロー

### 1️⃣ VRM読み込み

**要件**

- glTFとしてパース
- VRM拡張からHumanoidボーンマッピング抽出

**対象ボーン：**

```
hips, spine, chest, neck, head,
leftUpperArm, leftLowerArm, leftHand,
rightUpperArm, rightLowerArm, rightHand,
leftUpperLeg, leftLowerLeg, leftFoot,
rightUpperLeg, rightLowerLeg, rightFoot
```

### 🦴 必要ボーン一覧（Second Life Bento拡張）

#### 主要ボーン（VRM→SL変換）

- Hips → mPelvis
- Spine → mTorso
- Chest → mChest
- Neck → mNeck
- Head → mHead
- 両腕・両脚各部位

#### Bento拡張ボーン

**目：** mEyeLeft, mEyeRight（視線追従）
**口：** mFaceJaw（チャット・音声連動）
**まぶた：** mFaceEyeLidUpperLeft, mFaceEyeLidUpperRight（自動瞬き）
**指（両手フルセット）：**

- 左手例：mHandThumb1Left～Thumb3Left, mHandIndex1Left～Index3Left ...等
- 右手も同様

### 2️⃣ ボーン名マッピング

| VRM           | SL             |
| ------------- | -------------- |
| hips          | mPelvis        |
| spine         | mTorso         |
| chest         | mChest         |
| neck          | mNeck          |
| head          | mHead          |
| leftUpperArm  | mShoulderLeft  |
| leftLowerArm  | mElbowLeft     |
| leftHand      | mWristLeft     |
| rightUpperArm | mShoulderRight |
| rightLowerArm | mElbowRight    |
| rightHand     | mWristRight    |
| leftUpperLeg  | mHipLeft       |
| leftLowerLeg  | mKneeLeft      |
| leftFoot      | mAnkleLeft     |
| rightUpperLeg | mHipRight      |
| rightLowerLeg | mKneeRight     |
| rightFoot     | mAnkleRight    |

**補足：** upperChestは削除（chestへ統合）。不要ボーンは完全削除。

### 3️⃣ 階層再構成

**最終階層例：**

```
mPelvis
├ mTorso
│  ├ mChest
│  │  ├ mShoulderLeft → mElbowLeft → mWristLeft
│  │  └ mShoulderRight → mElbowRight → mWristRight
│  ├ mNeck → mHead
│  └ ...その他Bento拡張ボーン
├ mHipLeft → mKneeLeft → mAnkleLeft
└ mHipRight → mKneeRight → mAnkleRight
```

### 4️⃣ Aポーズ → Tポーズ補正

**問題**

VRoidはAポーズ。SLは厳密なTポーズ。

**対応**

- 上腕ボーンの現在ローカル回転を取得
- 水平（X軸）へ回転補正
- 補正クォータニオンを計算
- ボーン回転を修正
- 同時に頂点を逆方向へ回転補正

**数学処理：**

```rust
corrected_rotation = target_T_pose * inverse(current_pose)
v' = correction_matrix * v
```

### 5️⃣ inverseBindMatrices再生成

1. 各ボーンのワールド行列を計算
2. 逆行列を求める
3. skins.inverseBindMatricesへ書き込み

**計算式：**

```rust
bind_matrix = parent_world * local_transform
inverse_bind = bind_matrix.inverse()
```

### 6️⃣ ウェイト整理

- 削除ボーンに割り当てられているウェイトを親へ再分配
- 未使用ボーンをjointsから削除
- joints配列をSLボーン順に再構築

### 7️⃣ メッシュ整理

- 不要ノード削除
- Morph target削除
- Animation削除
- Extras削除
- VRM拡張削除
- 最終的に純粋なglTF 2.0へ

---

## 📐 数学仕様

**使用型：**

```rust
Matrix4<f32>
Vector3<f32>
UnitQuaternion<f32>
```

**座標系：** glTF準拠（右手座標、Y-up）

---

## 🚨 重要制約

- ボーン名完全一致必須
- 階層完全一致必須
- inverseBindMatricesとノード整合必須
- スケールは1.0固定
- 全TransformをApply済みにする

---

## 💻 アーキテクチャ

テンプレート：[tauri-vuetify-starter](https://github.com/logue/tauri-vuetify-starter)

```
┌─────────────────────────┐
│    Vue + Vuetify UI      │
│  ・VRM読み込み          │
│  ・身長/スケール設定    │
│  ・ボーンマッピング確認 │
│  ・顔アニメ設定         │
│  ・プレビュー（3D）     │
│  ・エクスポート         │
└────────────┬────────────┘
             │ Tauri IPC invoke
┌────────────▼────────────┐
│      Rust Core          │
│  ・VRM解析              │
│  ・ボーン変換           │
│  ・行列再計算           │
│  ・ウェイト処理         │
│  ・バインド再生成       │
└────────────┬────────────┘
             │
         .gdbファイル
```

---

## 🎨 GUI仕様

### 1️⃣ VRM読み込みエリア

**セクション内容**

- 📂 「VRMファイルを選択」ボタン
- VRM基本情報表示
  - モデル名
  - 作者
  - 推定身長
  - ボーン数
  - メッシュ数
- 読み込みステータス表示

### 2️⃣ 身長・スケール設定エリア

SLは身長200cm基準（VRM標準は170cm）のため倍率調整が必須。

**必須パーツ**

- VRM検出身長（自動表示、cm）
- SL目標身長入力フィールド（デフォルト：200cm）
- 自動計算スケール表示
- 手動スケール微調整スライダー
- 「等倍リセット」ボタン

**プレビュー補助**

- 床グリッド表示ON/OFF
- 170cm / 200cm ガイドライン表示トグル

### 3️⃣ ボーンマッピング確認エリア

**最低限必要ボーン**

- Hips, Spine, Chest, Neck, Head
- 両腕・両脚各部
- mEyeLeft / mEyeRight
- mFaceJaw
- 指（全20本）

**GUI要素**

- 自動マッピング結果表示
- 手動修正ドロップダウン
- 未割当ボーン警告表示
- 「必須ボーン不足」エラー表示

### 4️⃣ 顔アニメーション設定エリア

#### 👁 瞬き

- ON/OFF トグル
- 瞬き間隔スライダー
- 閉じ時間スライダー
- 片目ウィンク対応チェック

#### 👄 クチパク

- ON/OFF トグル
- タイピング連動 / チャット連動 モード選択
- 開口角度スライダー
- 開閉スピードスライダー

#### 👀 瞳

- カメラ追従ON/OFF
- ランダム視線ON/OFF
- 可動範囲スライダー（上下角度、左右角度）
- 移動速度スライダー

### 5️⃣ 指ボーン確認エリア

- 自動割当結果表示
- 各指の曲げテストボタン
- 一括ポーズテスト（グー・パー）

### 6️⃣ プレビューエリア（超重要）

**フレームワーク：** three.js想定

**基本機能**

- モデル表示（自動更新）
- 回転ドラッグ操作
- ズーム制御
- ライトON/OFF
- ワイヤーフレーム表示切り替え

**ボーン操作**

- ボーン表示/非表示
- ボーン選択ハイライト
- Tポーズ確認

**アニメーション再生テスト**

- 歩行アニメ再生
- 待機アニメ再生
- 指ポーズ再生
- 顔アニメテスト再生

**アニメーションソース：** Second Life提供BVH（`bvh_files.zip`）を `frontend/public/animations/` に静的配置（プレビュー用）  
**読込方式：** `BVHLoader` によるBVH直接読込（事前変換不要）

### 7️⃣ エクスポート設定

**出力形式選択**

- glTF（SL用、推奨）
- バイナリglb

**オプション**

- ボーン圧縮ON/OFF
- 不要ボーン削除確認
- スケール適用方法選択
- **📦 テクスチャ圧縮設定**
  - [ON] テクスチャサイズ自動縮小（推奨）
    - 1024×1024超過時に自動的に1024×1024以下へリサイズ
    - PNG再圧縮（oxipng使用）で容量最適化
      - リサイズ方法選択：バイリニア（デフォルト） / ニアレスト / バイキュービック / Gaussian / Lanczos3
  - [OFF] オリジナルサイズを保持（アップロード費用増加）

### 8️⃣ バリデーションパネル

**チェック項目**

- ✅ 未割当ボーン警告
- ✅ ウェイト異常検出
- ✅ ボーン階層エラー
- ✅ 非対応ノード検出
- ✅ ⚠️ **頂点数制限警告**（Second Life上限：65535頂点）
  - 超過時：「⛔ 頂点数オーバー（現在: xxxxx / 上限: 65535）。アップロードできません」
  - 対策案：メッシュ統合やテクスチャベイク推奨
- ✅ 💰 **テクスチャサイズと費用警告**
  - テクスチャが1024×1024を超過時：「⚠️ テクスチャサイズが大きいため、Second Lifeアップロード費用が増加します」
  - 詳細表示：各テクスチャの解像度一覧
  - 推奨：「📦 テクスチャ圧縮・自動縮小をONに設定してください」
  - 自動縮小時の費用削減目安を表示
- ✅ ポリゴン数警告

### 9️⃣ プロジェクト保存機能

- 設定JSON保存
- 設定読込
- 最後の作業状態自動復元

**仕様：** JSONスキーマを用意

### 📋 最小限構成（MVP）

今すぐ実装すべき順序：

- [x] VRM読込
- [x] 身長スケール設定
- [x] ボーンマッピング確認
- [x] 顔制御設定（簡潔版）
- [x] 指確認
- [x] 3Dプレビュー（three.js、バックエンド生成GLB表示）
- [x] エクスポート（.gdb出力）

### 🧾 実装済み（2026-02-26時点）

- [x] Rustバックエンドの解析/変換コマンド（Tauri IPC・CLI）
- [x] VRM拡張ベースのHumanoidボーン抽出（VRMC_vrm / VRMフォールバック）
- [x] 必須ボーン不足・階層不整合・頂点数・テクスチャ警告のバリデーション表示
- [x] テクスチャ自動縮小ポリシー（1024/2048）と出力後サイズレポート
- [x] プロジェクト設定の保存/読込
- [x] 2カラムUI（左:操作 / 右:プレビュー）とログ表示DOM

### 📌 残件（マイルストーン別）

#### v0.8（先行：ボーン変換基盤）

- [x] 必須Humanoidボーン抽出（VRMC_vrm / VRMフォールバック）
- [x] VRM→SLボーン名変換（基本17ボーン）
- [x] 必須ボーン不足・階層不整合の検証
- [x] ボーン変換の事前条件検証を強化（ノード参照整合・変換適用可否）
- [x] Aポーズ→Tポーズ補正の本実装（上腕ローカル回転の補正）
- [x] inverseBindMatrices再生成の本実装
- [x] ｃ
- [ ] ボーン階層の再構成（SL向け最終階層を明示的に構築）
- [ ] Bento拡張ボーン（目・口・まぶた・指）の実運用レベル対応
- [ ] Blender再読込～SLアップロードの最終検証フロー完了

#### v0.9（先行：アニメーション再生基盤）

- [x] 変換対象アニメーションを確定（walk / stand / sit / turn など）
- [x] BVH入力セットを取得（`frontend/public/animations/` に配置済み・120ファイル）
- [x] アセット配置方針を決定：BVH を `public/animations/` に静的配置、`BVHLoader` で直接読込
- [ ] 再生対象BVHをリストアップし、カテゴリ分けする（stand/walk/sit/dance など）
- [ ] `VrmPreview.vue` に `BVHLoader` を組み込みアニメーション再生を実装
- [ ] プレビューに再生UIを追加（選択・再生・停止・ループ）
- [ ] 再生テストを実施（walk / stand / sit の連続再生確認）
- [ ] ボーン名マッピング確認（SL BVH骨格 ↔ 変換後GLB骨格）
- [ ] 失敗時フォールバック挙動を実装（未読込時メッセージ表示）
- [x] 同梱配布する場合の著作権表記テンプレート整備
- [x] 同梱配布可否のライセンス最終確認（CC BY 3.0）

#### v1.0（次バージョン）

- [ ] 尾・耳・羽の動作対応
- [ ] プレビューの追加操作（グリッド/ライト/ワイヤーフレーム切替）

#### v1.1（その次のバージョン）

- [ ] ヘッド抽出モード

---

## 🔨 実装詳細仕様

### エラーハンドリング戦略

**VRoid Studio標準出力のVRMのみ対応。それ以外はエラー出力して終了。**

**対応ケース**

✅ VRoid Studioで標準出力されたVRM  
&nbsp;&nbsp;→ 必須ボーンが全て存在、標準的なボーン階層構造

**非対応ケース（エラー出力のみ）**

❌ VRoid Studio以外で作成されたVRM  
❌ 標準Humanoidボーンが欠落  
❌ 非標準的なボーン命名  
❌ カスタムボーン構造

**エラーメッセージ例：**

```
[ERROR] VRoid Studio標準のVRMのみサポートしています
[ERROR] 必須ボーン XXX が見つかりません
[ERROR] 非標準的なボーン階層です
```

### 検証チェックリスト（開発時）

- □ 全ボーンがinverseBindMatricesに含まれる
- □ joints配列が重複なく構成される
- □ ウェイト合計がプリミティブごとに1.0
- □ バウンディングボックス再計算完了

### 出力仕様

- **形式：** .gdbファイル（glTF 2.0準拠、VRM拡張なし）
- **内容：** メッシュ + ボーン + テクスチャ（画像データ込み）
- **対応：** .gdbのみ（.dae等は非対応）

### アニメーション管理戦略

**ライセンス制約**

アニメーションデータは CC BY 3.0 に基づき、著作権表記を付与したうえで本ツールのプレビュー用アセットとして配置・利用する。

**ライセンス表記（配布時）**

Contains animation data © Linden Research, Inc.  
Licensed under CC BY 3.0  
https://creativecommons.org/licenses/by/3.0/  
Modified for use in this tool.

**アニメーション準備方法**

1. **ソース：** `https://static-secondlife-com.s3.amazonaws.com/downloads/avatar/bvh_files.zip`
2. **配置：** 解凍したBVHファイルを `frontend/public/animations/` に配置（**配置済み・120ファイル**）
3. **再生：** three.js の `BVHLoader` を使い、変換不要でそのまま読み込み可能
4. **アクセス：** `fetch('/animations/xxx.bvh')` または `BVHLoader.load('/animations/xxx.bvh', ...)` でOK

**形式について**

three.js には `three/examples/jsm/loaders/BVHLoader.js` が付属しており、BVH を `AnimationClip` として直接読み込めます。  
事前変換（BVH → glTF）は不要です。

- **入力形式：** BVH形式（Second Life標準）
- **ローダー：** `BVHLoader`（three.js 同梱）
- **出力：** `{ clip: AnimationClip, skeleton: Skeleton }` → `AnimationMixer` で再生
- **glTF出力：** アニメーションは埋め込まない（Second Life ではglTFアニメーションは無視されるため）

**実装の流れ**

```
1. frontend/public/animations/*.bvh を配置（済み）
   ↓
2. VrmPreview.vue に BVHLoader を import
   ↓
3. AnimationMixer + BVHLoader.load() でアニメーション再生
   ↓
4. 再生UI（ドロップダウン選択 → 再生/停止）を追加
```

**注意点**

- SL BVHのボーン名（mHipなど）とVRM/GLBのボーン名が異なるため、再生前にボーン名マッピングが必要
- `public/` 配下はViteビルドでそのままコピーされるためTauriでも `fetch('/animations/...')` でアクセス可能
- アップデート時はBVHファイルをそのまま差し替えるだけでよい

### テクスチャ処理戦略

**自動縮小オプション（デフォルト：ON）**

テクスチャサイズが1024×1024を超える場合、以下の処理を自動実行：

**処理流れ**

```
1. テクスチャサイズ検査
   ↓
2. 1024×1024超過判定
   ├─ YES：リサイズ処理へ進む
   └─ NO：そのまま使用
   ↓
3. リサイズ処理（image クレート利用）
   - 対象：1024×1024を上限に縮小
   - 方法：バイリニア補間（デフォルト）/ ニアレスト / バイキュービック / Gaussian / Lanczos3
   ↓
4. PNG再圧縮（oxipng利用）
   - 最適化レベル：--opt 4（デフォルト）
   - 容量削減効果：入力サイズにより20-50%削減期待
   ↓
5. glTFバッファに統合
```

**テクスチャ圧縮オプション**

- **自動縮小 ON（推奨）**
  - 1024×1024超過時に自動リサイズ
  - oxipngで再圧縮（ロスレス）
  - Second Lifeアップロード費用削減
  - リサイズ品質低下を最小化

- **自動縮小 OFF**
  - オリジナルテクスチャを保持
  - Second Lifeアップロード費用が増加
  - 高品質テクスチャ維持（選択肢あり）

**費用シミュレーション表示**

バリデーションパネルに以下を表示：

```
テクスチャ統計：
- 総テクスチャ数：5枚
- 1024×1024超過：2枚（2048×2048）
- 推定アップロード費用：500L$（圧縮前）→ 200L$（圧縮後）
- 費用削減：60%
```

---

## 📦 最終成果物

### CLIツール

```bash
vrm2sl input.vrm output.gdb
```

### 出力ファイル形式

- **形式：** glTF 2.0 JSON
- **パッケージ：** Second Lifeアップロード可能（.gdbファイル）

---

## ✅ 検証と成功条件

### ❌ 対応しないもの

- 表情・ブレンドシェイプ
- 物理ボーン
- 揺れ物
- 胸揺れ・尻揺れ（Second Lifeシェイプ依存のため）
- Fitted Mesh
- モーション・アニメーションのglTF出力埋め込み（Second Life側で無視されるため非対応）
- シェイプ連動

### 🎯 成功条件

- SL内で基本アニメーションが破綻しない
- 腕のねじれがない
- 足が崩れない
- 全身動作が正常

### 🧪 検証手順

1. 変換後glTFをBlenderへ再読み込み
2. Tポーズ確認
3. アーマチュア崩壊がないか確認
4. SLへアップロード
5. アルファで元ボディを消去して動作確認

---

## ⚠️ 既知の制限事項と対応方法

### 瞳テクスチャの描画問題

**問題**

VRoidモデルの瞳は半透明テクスチャ（アルファチャンネル含む）として組み込まれています。

- **three.js（プレビュー）：** 半透明テクスチャは正常に描画可能
- **Second Life エンジン：** 半透明テクスチャの描画がサポートされていないか、正常に描画されない可能性がある

**現在の対応**

v0.1ではこの問題への自動変換は実装しません。以下の手動対応を推奨：

1. **Blenderでの焼き込み（推奨）**
   - 変換後のglTFをBlenderで開く
   - 瞳メッシュのアルファチャンネル付きテクスチャを、不透明テクスチャベイクで焼き替え
   - テクスチャを再エクスポート

2. **SL内での対応**
   - 瞳部分を別メッシュとして認識
   - SL内で瞳形状に合わせたプリムを配置
   - または、フルボディアルファマスクで瞳のアルファを調整

**将来対応（v1.2以降）**

- テクスチャ自動焼き込み機能の実装
- 瞳プリセット形状の自動生成

### テクスチャサイズとメモリ制限

**Second Lifeの制限**

- テクスチャメモリ上限：一般的に1536×1536ピクセル以下推奨
- アップロード費用：テクスチャサイズに応じて変動
  - 1024×1024以下：低価格帯
  - 1024×1024超過：費用増加（2048×2048等で2-3倍）

**このツールでの対応**

**v0.1：自動テクスチャ縮小機能（デフォルト：ON）**

エクスポート時に「📦 テクスチャ圧縮・自動縮小」オプションが利用可能：

1. **自動縮小 ON（推奨）**
   - 1024×1024超過テクスチャを自動リサイズ
   - oxipngで再圧縮（ロスレス）
   - Second Lifeアップロード費用を削減
   - バリデーションで費用削減目安を表示

2. **自動縮小 OFF**
   - オリジナルテクスチャを保持
   - 高品質を維持するが、アップロード費用が増加
   - テクスチャサイズ警告が表示される

---

## 🔥 期待される難易度

- **実装時間：** 3〜7日
- **デバッグ：** SLアップロードで調整必須
- **必須知識：** 行列演算、ボーン操作、glTF仕様理解
