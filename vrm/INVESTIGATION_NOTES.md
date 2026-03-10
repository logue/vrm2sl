# VRM→SL変換 調査メモ

## 調査日：2026-03-11

---

## 1. 問題の概要

**症状：** SecondLifeにアップロードしたアバターを歩かせると「蟹歩き」になる。
XとYが入れ替わっているように見える。

**現状：** 静的データ（GLB構造・IBM・スキンウェイト・BVHアニメーション）はすべて数学的に正しいことを確認済み。
根本原因の特定には至っていない。

---

## 2. 検証済み項目

### 2.1 IBMの正確性（全47ボーン）

```
検証スクリプト: vrm/check_all_ibm.py
結果: 全47ボーンのIBM位置 = -(ワールド座標) で一致 ✅
```

- IBM accessor count = 47、joints count = 47 で一致 ✅
- 全ジョイントが "OK" ステータス ✅

主要ボーンのワールド座標：

| ボーン     | ワールド位置 (m)          | IBM位置（検証） |
| ---------- | ------------------------- | --------------- |
| mPelvis    | (0.0000, 1.1493, 0.0056)  | 正符号逆転 ✅   |
| mHipLeft   | (0.0892, 1.1031, 0.0014)  | 正符号逆転 ✅   |
| mKneeLeft  | (0.0892, 0.6348, -0.0084) | 正符号逆転 ✅   |
| mAnkleLeft | (0.0890, 0.1105, -0.0402) | 正符号逆転 ✅   |

### 2.2 シーン構造

```python
# vrm/check_scene.py による確認
Scene nodes: [0, 91, 92, 93]

Node 0 (Root):      t=[0,0,0], r=[0,0,0,1]  (identity)
  Node 1 (mPelvis): t=[0, 1.1493, 0.0056], r=[0,0,0,1]
    ... (全47ボーン, 全て回転identity)

Node 91 (Face): mesh=0, skin=1
Node 92 (Body): mesh=1, skin=1
Node 93 (Hair): mesh=2, skin=1

Skin 0,1,2: skeleton=1 (mPelvis)
```

**注目点：** メッシュノード（Face/Body/Hair）は Root の子ではなくシーンルートの兄弟。
ただし glTF の標準的なパターンとして正しい（node.skin で参照されている）。

### 2.3 スキンデータ（Body mesh）

```
検証スクリプト: vrm/check_skin_data2.py
```

- mHipLeft のジョイントインデックス = 41
- mHipLeft に30%以上影響される頂点数 = 394個
  - 全394頂点が X > 0（左側 = 正方向、正しい ✅）
  - Y範囲: 0.616〜1.165m（太もも領域、正しい ✅）
- mKneeLeft 影響頂点: Y範囲 0.067〜0.667m（膝から足首、正しい ✅）

### 2.4 BVHアニメーションデータ

```
検証スクリプト: vrm/analyze_walk_bvh.py
対象: avatar_walk.bvh
```

- lThigh X回転: +31.8°（前方）〜 -39.7°（後方）で交互 ✅
- rThigh X回転: lThigh と逆位相（正常な歩行パターン）✅
- lShin X回転: 0° 〜 +73.9°（膝が前方に屈曲、正しい ✅）
- **BVHアニメーション自体は蟹歩きではない ✅**

BVH lThigh オフセット: (4.500, -6.400, -1.833) cm
BVH lShin オフセット: (-1.359, -18.919, +1.180) cm

### 2.5 BVHローダーの動作確認

```
ソース: three/examples/jsm/loaders/BVHLoader.js（Three.js v0.183.2）
```

- ワールド固定軸 `vx=(1,0,0), vy=(0,1,0), vz=(0,0,1)` を使用
- トラック名形式: `boneName.quaternion`
- BVHLoader は各チャンネル（Xrotation/Zrotation/Yrotation）を個別に `setFromAxisAngle + multiply` で処理
- **SL BVH のチャンネル順が `XZY` でも、BVHLoader が正しいクォータニオンに変換する** ✅

### 2.6 Three.js PropertyBinding の解決

```
ソース: PropertyBinding.js
該当コード:
  const bone = root.skeleton.getBoneByName(nodeName);
```

- `.bones[mHipLeft].quaternion` は `skinnedMesh.skeleton.getBoneByName('mHipLeft')` で解決される ✅
- `buildRetargetedClip` のトラック名形式 `.bones[${targetBoneName}].${parsed.property}` は正しい ✅

### 2.7 Node.js シミュレーション（クォータニオン直接コピーのテスト）

```
スクリプト: vrm/test_bvh_retarget.mjs
```

BVH lThigh X回転を直接GLBへ適用した場合の lShin Z座標変化：

| フレーム | lThigh X回転 | BVH空間 shin.Z | GLB空間 shin.Z | 判定    |
| -------- | ------------ | -------------- | -------------- | ------- |
| 0        | +19.7°       | -0.279         | -0.357         | 前方 ✅ |
| 1        | +31.8°       | -0.473         | -0.545         | 前方 ✅ |
| 3        | -25.5°       | +0.487         | +0.411         | 後方 ✅ |
| 4        | -38.0°       | +0.666         | +0.599         | 後方 ✅ |

**BVH空間とGLB空間の両方で方向が一致している ✅**

---

## 3. GLBファイル構造（検証済み）

**ファイル:** `vrm/avatar_new.glb`（最新変換出力）

```
全ボーン回転: [0,0,0,1]（identity）
スケール: 1.2836倍（VRM 155.81cm → SL 200cm目標）
座標系: glTF準拠（Y-up 右手座標、-Z前方）
```

---

## 4. 座標系に関する重要な知見

### SL BVH の座標系について

**重要: SL BVH は Z-up ではなく Y-up（glTF と同じ右手座標系）**

- `avatar_walk.bvh` の hip 位置が `(x, 40.59, z)` → Y が高さ方向で確認済み
- BVH チャンネル順: `Xrotation Zrotation Yrotation`（SL 独自慣例）
- Three.js BVHLoader がこの順序を自動処理するため、追加の座標変換は不要

### SL ビューア内部座標系について（仮説）

- SL ビューア内部は Z-up（X=東、Y=北/前方、Z=上）の可能性がある
- Y-up glTF をアップロードすると SL が内部で座標変換する可能性がある
- もし Y-up→Z-up 変換が適用される場合、X軸とZ軸が入れ替わり蟹歩きが発生しうる

---

## 5. テスト用GLBファイル

SLの座標系テスト用に以下のファイルを作成済み：

- `vrm/avatar_rot_plus90x.glb` — Root に +90°X 回転（Y-up→Z-up 補正）
- `vrm/avatar_rot_minus90x.glb` — Root に -90°X 回転

これらを SL にアップロードして挙動を確認することで、SL 側の座標変換の有無を確認できる。

---

## 6. 変換パイプライン（確認済み）

```
rename_bones
→ normalize_sl_bone_rotations
→ correct_mesh_vertices_for_bind_pose_change
→ remap_unmapped_bone_weights
→ optimize_skinning_weights_and_joints
→ soften_face_eye_influences
→ collapse_secondary_head_skins_to_primary
→ merge_head_only_skins_into_primary
→ promote_pelvis_to_scene_root
→ set_skin_skeleton_root
→ bake_scale_into_geometry
→ regenerate_inverse_bind_matrices
→ remove_vrm_extensions_and_extras
```

各関数は数学的に正しいことを確認済み。

---

## 7. `buildRetargetedClip` の実装（VrmPreview.vue）

```typescript
const buildRetargetedClip = (
  targetSkeleton: THREE.Skeleton,
): THREE.AnimationClip | null => {
  if (!bvhMotionClip) return null;
  const tracks: THREE.KeyframeTrack[] = [];
  for (const track of bvhMotionClip.tracks) {
    const parsed = parseBvhTrack(track.name);
    if (!parsed) continue;
    const targetBoneName = BVH_TO_SL_BONE[parsed.bone];
    if (!targetBoneName) continue;
    if (!targetSkeleton.getBoneByName(targetBoneName)) continue;
    if (
      parsed.property === "quaternion" &&
      HAND_PROBLEM_BONES.has(targetBoneName)
    )
      continue;
    if (parsed.property === "position") continue; // 位置トラックをスキップ
    const nextTrack = track.clone();
    nextTrack.name = `.bones[${targetBoneName}].${parsed.property}`;
    tracks.push(nextTrack);
  }
  if (tracks.length === 0) return null;
  return new THREE.AnimationClip(
    "avatar_motion_retargeted",
    bvhMotionClip.duration,
    tracks,
  );
};
```

**クォータニオン直接コピー方式の妥当性：**

- GLB バインドポーズ: 全ボーン回転ゼロ（完全Tポーズ）
- SL BVH リファレンスポーズ: Tポーズ前提
- 両者のリファレンスフレームが一致 → 追加変換不要 ✅

---

## 8. ボーンマッピング（BVH_TO_SL_BONE）

| BVH ボーン名 | SL（GLB）ボーン名 |
| ------------ | ----------------- |
| hip          | mPelvis           |
| abdomen      | mTorso            |
| chest        | mChest            |
| neck         | mNeck             |
| head         | mHead             |
| lShldr       | mShoulderLeft     |
| lForeArm     | mElbowLeft        |
| lHand        | mWristLeft        |
| rShldr       | mShoulderRight    |
| rForeArm     | mElbowRight       |
| rHand        | mWristRight       |
| lThigh       | mHipLeft          |
| lShin        | mKneeLeft         |
| lFoot        | mAnkleLeft        |
| rThigh       | mHipRight         |
| rShin        | mKneeRight        |
| rFoot        | mAnkleRight       |

**注:** `neckDummy`（SL の chest→neck 中継ボーン）はマッピングなしでスキップ

---

## 9. 未解決の問題

### 蟹歩きの根本原因

- 全静的データは数学的に正しい
- Three.js シミュレーションでも正しい方向に動く
- **ただし実際の Three.js ランタイムまたは SL での挙動は未検証**

### 次のアクション案

1. **実際のアプリを起動して Three.js プレビューで確認する**
   - 蟹歩きが Three.js プレビューで発生するか？
   - それとも SL にアップロードしてから発生するか？

2. **SL テスト（SL側の問題の場合）**
   - `vrm/avatar_rot_plus90x.glb` を SL にアップロード
   - +90°X 回転でアバターが正常に立つなら SL は Y-up→Z-up 変換を行っている
   - その場合、Rust パイプラインにルート回転を追加する必要がある

3. **Three.js テスト（プレビューの問題の場合）**
   - バインドポーズ補正（レスト回転考慮）を追加する
   - BVH ボーンと GLB ボーンのリファレンスポーズ差を補正する

---

## 10. 検証スクリプト一覧

すべてのスクリプトは `vrm/` ディレクトリに格納されています。

### 既存スクリプト

| スクリプト                        | 用途                                              |
| --------------------------------- | ------------------------------------------------- |
| `check_bind_pose.py`              | GLB のバインドポーズ（IBM）確認                   |
| `check_bvh.py`                    | BVH ファイルの構造確認                            |
| `check_glb_bones.py`              | GLB のボーン階層・名前確認                        |
| `check_glb_hierarchy.py`          | GLB のノード階層確認                              |
| `check_walk_bvh.py`               | BVH 歩行データの回転値確認                        |
| `compare_ibm.py`                  | IBM の比較・検証（元バージョン）                  |
| `compare_vrm_ibm.py`              | VRM と変換後 GLB の IBM 比較                      |
| `inspect_detailed.py`             | GLB の詳細インスペクション                        |
| `inspect_eye_vertex_positions.py` | 目頂点位置のインスペクション                      |
| `inspect_eyes.py`                 | 目ボーン・メッシュのインスペクション              |
| `inspect_output.py`               | 変換後 GLB の総合インスペクション（元バージョン） |
| `inspect_skin0_weights.py`        | スキンウェイトの確認                              |
| `inspect_vrm.py`                  | VRM 元データのインスペクション                    |

### 調査セッションで追加したスクリプト

| スクリプト                     | 用途                                                    |
| ------------------------------ | ------------------------------------------------------- |
| `analyze_axes.py`              | 軸方向の分析                                            |
| `analyze_walk_bvh.py`          | BVH 歩行データのフレーム別回転値分析                    |
| `bvh_retarget_analysis.py`     | BVH リターゲット処理の解析                              |
| `check_all_ibm.py`             | 全47ジョイントの IBM 一括検証                           |
| `check_coordinate.py`          | Y-up / Z-up 座標系の比較分析                            |
| `check_ibm.py`                 | IBM 個別チェック                                        |
| `check_merge.py`               | スキンマージ処理の確認                                  |
| `check_mesh_skin.py`           | メッシュとスキンの参照関係確認                          |
| `check_mhead_ibm.py`           | mHead の IBM 確認                                       |
| `check_scene.py`               | GLB シーン構造（ノード階層）の確認                      |
| `check_skin_data.py`           | スキン JOINTS_0/WEIGHTS_0 データ確認                    |
| `check_skin_data2.py`          | スキン JOINTS_0/WEIGHTS_0 詳細確認（mHipLeft検証）      |
| `check_skin_detail.py`         | スキン詳細インスペクション                              |
| `check_vrm_face.py`            | VRM 顔メッシュの確認                                    |
| `check_vrm_legs.py`            | VRM 脚ボーン・メッシュの確認                            |
| `compare_bone_axes.py`         | VRM と GLB のボーン軸方向比較                           |
| `compare_ibm_session.py`       | IBM 比較（調査セッション版、`compare_ibm.py` と別内容） |
| `create_rotation_variants.py`  | ルート回転バリアント GLB の生成                         |
| `inspect2.py` 〜 `inspect5.py` | 段階的インスペクションスクリプト群                      |
| `inspect_output_session.py`    | 変換後 GLB インスペクション（調査セッション版）         |
| `investigate_crab_walk.py`     | 蟹歩き原因調査の統合スクリプト                          |
| `sim_deform.py`                | スキンデフォームのシミュレーション                      |
| `test_bvh_retarget.mjs`        | BVH→GLB クォータニオン直接コピーの Node.js 検証         |

### テスト用GLBファイル

| ファイル                                | 内容                                   |
| --------------------------------------- | -------------------------------------- |
| `avatar_new.glb`                        | 最新の変換出力（全検証完了済み）       |
| `avatar_rot_plus90x.glb`                | Root に +90°X 回転（SL Z-up テスト用） |
| `avatar_rot_minus90x.glb`               | Root に -90°X 回転（SL Z-up テスト用） |
| `avatar_test.glb` 〜 `avatar_test5.glb` | 各段階の中間変換出力                   |
| `debug_output.glb`                      | デバッグ用変換出力                     |
| `fixed_output.glb`                      | 修正適用後の変換出力                   |
| `output_test.glb`                       | 出力テスト用 GLB                       |

---

## 11. 既知の修正済みバグ

### Face/Hair スキンの joints 圧縮バグ（修正済み）

**症状:** SL アップロード後、頭部が胴体から浮いて見える  
**原因:** `soften_face_eye_influences` が `&Value`（不変参照）だったため joints 書き換えがデッドコード化  
**修正:** `&mut Value` に変更し、joints を `[mHead]` のみに圧縮

修正後の期待値：

```
Skin 0 (Face): joints=['mHead']   ✅
Skin 1 (Body): joints=47ボーン
Skin 2 (Hair): joints=['mHead']   ✅
```
