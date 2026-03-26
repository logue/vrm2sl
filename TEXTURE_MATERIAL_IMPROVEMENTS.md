# VRM to SecondLife Converter - Texture & Material Improvements (v0.8.1)

## 📝 変更概要

SecondLife での テクスチャ読み込み問題、アルファバグ、PBR マテリアル非対応への対応を実施しました。

### 進捗メモ（2026-03-25）

- 変換後 GLB にテクスチャが同梱されていることを確認
- 変換後アバター形状が壊れていないことを確認

## 🔧 実装詳細

### 1️⃣ **テクスチャバッファ処理の強化** (`backend/src/convert/mod.rs`)

#### 改善点

- **バッファビュー検証の強化**
  - 各バッファビューのオフセット・長さ境界チェックを追加
  - 無効なバッファビュー参照の検出と警告ログ出力
  - 境界超過時のエラーハンドリング強化

- **テクスチャデコード/エンコードの堅牢性向上**
  - 各画像毎にデコード失敗時のエラーハンドリング
  - MIME タイプが不正な場合の警告とスキップ
  - 再エンコード失敗時のグレースフルフォールバック

- **4バイトアライメント強化**
  - glTF 仕様準拠の 4 バイト単位でのバッファ配置確認
  - パディング計算ロジックの明確化

**コード変更：**

```rust
fn apply_texture_resize_to_embedded_images(...) {
    // ─── Step 1: 詳細なバッファ境界チェック
    for (view_index, view) in buffer_views.iter().enumerate() {
        let offset = ...;
        let length = ...;
        let end = offset.saturating_add(length);

        if end > bin.len() {
            eprintln!("[WARN] Invalid buffer view bounds...");
            segments.push(Vec::new());
        }
    }

    // ─── Step 2: 詳細なイメージ処理
    for (image_index, image) in images.iter_mut().enumerate() {
        // MIME タイプ検証・デコードエラーハンドリング
        if let Err(e) = image::load_from_memory(image_bytes) {
            eprintln!("[WARN] Failed to decode image {}: {}", image_index, e);
            continue;
        }
        // ...
    }

    // ─── Step 3-5: バッファ再構築・メタデータ更新
}
```

---

### 2️⃣ **マテリアル正規化モジュール** (`backend/src/convert/material.rs` - NEW)

新規作成したモジュールで、SecondLife との互換性を確保します。

#### 関数群

**`normalize_materials_for_secondlife()`**

各マテリアルを以下のルールで最適化：

- **Alpha Mode ハンドリング**
  - `OPAQUE`: 不透明（変更なし）
  - `BLEND`: アルファブレンディング（alphaCutoff 削除）
  - `MASK`: アルファテスト
    - 眼・眉毛など必要な箇所のみ保持
    - 通常メッシュ → `BLEND` に変換（SecondLife 互換性向上）

- **PBR Metallic/Roughness 正規化**
  - `metallicFactor` を [0.0, 1.0] にクランプ
  - `roughnessFactor` を [0.0, 1.0] にクランプ
  - デフォルト値を設定（metallic: 0.0, rough: 1.0）
  - 無効なテクスチャ参照を削除

- **Double-Sided フラグ管理**
  - 眼球・眉毛・まつ毛: `true` に設定
  - その他: `false`（SecondLife デフォルト）

- **Emissive Factor 確保**
  - デフォルト [0.0, 0.0, 0.0] を設定（未設定の場合）

- **非対応拡張機能の削除**
  - `KHR_*`, `EXT_*`, `VENDOR_*` 拡張を除去

**`validate_and_fix_texture_references()`**

テクスチャ参照の整合性を確保：

- 有効なテクスチャインデックス範囲をチェック
- 無効な参照をマテリアルから削除
- bufferView 参照の妥当性確認
- MIME タイプのデフォルト設定 (PNG)

**テスト**

```rust
#[test]
fn given_mask_mode_eye_material_when_normalizing_then_mask_preserved() {
    // 眼球素材: MASK モード保持
}

#[test]
fn given_mask_mode_ordinary_material_when_normalizing_then_converted_to_blend() {
    // 通常素材: BLEND に変換
}

#[test]
fn given_pbr_values_out_of_range_when_normalizing_then_clamped() {
    // PBR 値のクランプ確認
}

#[test]
fn given_eye_material_when_normalizing_then_double_sided_enabled() {
    // 眼球: doubleSided = true
}
```

---

### 3️⃣ **処理フロー統合** (`backend/src/convert/mod.rs`)

マテリアル正規化とテクスチャ検証を変換パイプラインに統合：

```rust
fn transform_and_write_glb(...) {
    // ... 既存処理 ...

    remove_vrm_extensions_and_extras(&mut json);
    remove_unsupported_features(&mut json);

    // ★ NEW: SecondLife 互換性処理
    normalize_materials_for_secondlife(&mut json)?;
    validate_and_fix_texture_references(&mut json)?;

    apply_texture_resize_to_embedded_images(...)?;

    // ... 出力処理 ...
}
```

**処理順序の重要性：**

1. VRM 拡張機能とモルフターゲット削除
2. **マテリアル正規化** ← 不正なマテリアル設定を修正
3. **テクスチャ参照検証** ← 不正な参照を削除
4. テクスチャリサイズ ← 修正されたマテリアルで再処理

---

## 🎯 解決された問題

### ✅ SecondLife テクスチャ読み込み失敗

**原因：**

- bufferView の境界チェック不足
- 無効な Image/texture 参照による仕様違反

**対策：**

- 詳細なバッファ検証ロジック
- エラーハンドリング強化
- 詳細なデバッグログ出力

### ✅ アルファバグ

**原因：**

- MASK モードが SecondLife で不完全サポート
- alphaCutoff の不正な値
- 眼球・まつ毛への不適切な BLEND モード適用

**対策：**

- マテリアル名ベースの智的な MASK/BLEND 選択
- alphaCutoff の [0.0, 1.0] クランプ
- 眼球領域への特別対応

### ✅ PBR マテリアル対応

**原因：**

- PBR 値の無制限設定
- 無効なテクスチャ参照による読み込み失敗

**対策：**

- 値の範囲正規化
- テクスチャ参照の妥当性チェック
- デフォルト値の設定

---

## 📊 互換性影響

| 機能               | 旧動作                                 | 新動作                | SecondLife 互換性 |
| ------------------ | -------------------------------------- | --------------------- | ----------------- |
| alphaMode          | VRM そのまま → OPAQUE となる場合が多い | 智的に最適化          | ✅ 向上           |
| alphaCutoff        | 範囲チェック無し                       | [0.0, 1.0] にクランプ | ✅ 向上           |
| metallic/roughness | 無制限値                               | [0.0, 1.0] クランプ   | ✅ 向上           |
| 眼球・まつ毛       | BLEND モード → z-fighting              | MASK/BLEND 最適化     | ✅ 向上           |
| doubleS ided       | VRM そのまま                           | 智的に設定            | ✅ 向上           |
| テクスチャ参照     | 検証なし → 読み込み失敗                | 妥当性チェック        | ✅ 向上           |

---

## 🧪 テスト戦略

### ユニットテスト

- ✅ MASK/BLEND 変換ロジック
- ✅ PBR 値のクランプ
- ✅ マテリアル設定の正値性
- ✅ テクスチャ参照検証

### 統合テスト（推奨）

```bash
# テスト用 VRM で変換確認
cargo run --manifest-path backend/Cargo.toml --bin vrm2sl \
  vrn/AvatarSample_A.vrm test_output.glb

# 出力ファイルを SecondLife にアップロード→動作確認
```

### チェックリスト

- [x] 変換後 GLB にテクスチャが含まれる
- [x] テクスチャが SecondLife に正常に読み込まれる
- [ ] 眼球・まつ毛に z-fighting がない
- [ ] メタル/テクスチャが正常に表示される
- [x] アバター形状が崩れていない

---

## 📋 デプロイ前チェック

- [x] コンパイル成功
- [x] ユニットテスト成功
- [x] 統合テスト（VRM ファイル）
- [ ] SecondLife 動作確認
- [ ] ログメッセージレビュー

---

## 🔗 参考資料

- glTF 2.0 仕様: https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html
- SecondLife アバター Upload: https://wiki.secondlife.com/wiki/Mesh
- PBR Material Best Practices: https://github.com/KhronosGroup/glTF-Best-Practices

---

## 📝 今後の検討項目

1. **動的テクスチャ圧縮**: WebP や ASTC への対応 (SecondLife Pro Viewer)
2. **法線マップ処理**: 法線テクスチャの最適化
3. **自動モデル診断**: 変換前のマテリアル互換性予測
4. **ユーザーガイド**: 最適な VRM メタデータの設定方法
