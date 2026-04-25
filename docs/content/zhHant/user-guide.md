# 使用者指南

## 基本使用方式

1. 首先，在 VRoid Studio 匯出你的 Avatar。此時請將格式選為 **VRM1.0**。建議降低多邊形數量；但貼圖尺寸不需要特別處理，因為本工具會自動縮小為 1024。
   ![vroid_export](../images/vroid_export.avif)
2. 啟動本工具，點擊資料夾圖示，然後選擇剛匯出的 \*.vrm 檔案。
3. 稍候片刻後，Avatar 會顯示在預覽區。點擊匯出按鈕，將 \*.glb 檔案輸出到任意位置。
   ![vrm2sl](../images/vrm2sl.avif)
4. 啟動 Second Life Viewer，選擇 世界 > 上傳 > Mesh。出現檔案選擇對話框後，選擇先前匯出的 \*.glb 檔案。
   ![upload_mesh_model](../images/upload_mesh_model.avif)
5. 會顯示模型上傳浮動視窗。在 細節等級（LoD） 分頁中，將中、低、最低 LoD 值都設為 0。同時勾選 產生法線。
   ::alert{type="info"}
   這不只適用於 Avatar。在 Second Life 中，建議將中、低、最低 LoD 設為 0。這不僅可降低土地影響與複雜度，也能降低上傳費用。
   ::
6. 接著切換到 上傳選項 分頁，並勾選 包含貼圖。
   ![upload_model-upload-options](../images/upload_model-upload-options.avif)
7. 切換到 綁定 分頁，將顯示的所有核取方塊全部勾選。
   ![upload_model-rigging](../images/upload_model-rigging.avif)
   ::alert{type="warning"}
   若未勾選此處，模型將無法作為 Avatar 使用。
   ::
8. 完成以上設定後，點擊 計算費用 按鈕計算上傳費用，然後進行上傳。
9. 右鍵點擊自己，選擇 脫下 > 全部移除，接著在物品欄中雙擊剛上傳的 Mesh 進行穿戴。
   ::alert{type="info"}
   需要事先準備全身 Alpha。這樣可讓系統 Avatar 身體不顯示。
   ::
10. 穿上上傳後的模型時，若維持原樣，可能會出現圖中所示的 Alpha 問題。請在 Avatar 已穿戴狀態下，於物品欄中右鍵已穿戴物件並點擊 編輯。
    ![edit_mesh](../images/edit_mesh.avif)
11. 在建置工具中開啟 材質 分頁。（Firestorm 可能還需要另外開啟 Blinn-Phong 分頁。）將 Alpha 模式設為 Alpha Mask，並將 Mask Cutoff 設為約 127，即可解決 Alpha 問題。
    ![build_tool_panel](../images/build_tool_panel.avif)

## 命令列使用方式

```bash
vrm2sl input.vrm output.glb --analyze-only --report report.json
```

## 注意事項

若要面向不特定大眾散布，或在市集中轉售，請務必**確認授權條款**。在 [Booth](https://booth.pm/zh-tw/browse/VRoid)、[Vroid Hub](https://hub.vroid.com/)、[Niconico Commons](https://commons.nicovideo.jp/search?keywords=VROID&sort=created&order=desc)、[Etsy](https://www.etsy.com/search?q=Vroid&ref=search_bar) 上販售或散布的許多 Avatar，通常**禁止二次利用、再散布或轉售**。
