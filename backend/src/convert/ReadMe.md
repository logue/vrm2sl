| ファイル              | 内容                                                                                  |
| --------------------- | ------------------------------------------------------------------------------------- |
| convert/types.rs      | 定数・公開型定義                                                                      |
| convert/gltf_utils.rs | glTF プリミティブ・バイナリ I/O                                                       |
| convert/skeleton.rs   | ボーン操作・階層再構築・IBM 再生成                                                    |
| convert/skinning.rs   | スキニング重み最適化                                                                  |
| convert/geometry.rs   | スケールベイク・メッシュ統計                                                          |
| convert/validation.rs | バリデーション・テクスチャ料金推定・メタデータ抽出                                    |
| convert/diagnostic.rs | 診断ログ構造体・書き出し                                                              |
| convert/mod.rs        | 公開 API (analyze_vrm, convert_vrm_to_gdb, write_final_validation_checklist) + テスト |
