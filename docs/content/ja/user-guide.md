# ユーザーガイド

## 基本的な使い方

1. まず、VRoid Studioでアバターを出力します。このときに、フォーマットは**VRM1.0**を選択してください。ポリゴン数は削減することを推奨しますが、テクスチャサイズに関しては、本ツール内で自動的に1024に縮小するので意識する必要はありません。
   ![vroid_export](../images/vroid_export.avif)
2. 本ツールを起動し、フォルダアイコンをクリックして先ほどエクスポートした\*.vrmファイルを選択します。
3. しばらくすると、右にアバターが表示されるので「エクスポートボタン」を押して任意の場所に\*.glbファイルを出力してください。
   ![vrm2sl](../images/vrm2sl.avif)
4. SecondLifeビューアを起動して、「ワールド」＞「アップロード」＞「メッシュ」を選択します。ファイル選択ダイアログが出るので、先程エクスポートした\*.glbファイルを選択してください。
   ![upload_mesh_model](../images/upload_mesh_model.avif)
5. モデルアップロードフローターが表示されます。この「詳細度（LoD）」タブから、まず、中・低・最低のLoD値をそれぞれ0にします。また、「法線を生成」にはチェックを入れてください。
   ::alert{type="info"}
   アバターに限った話ではありませんが、SecondLifeにおいてLoDの中と低と最低は0にすることを推奨します。これは、ランドインパクトや複雑度を削減するだけでなく、アップロード費用を削減することができます。
   ::
6. 次に、「アップロードオプション」タブに移動し、「テクスチャを含める」にチェックを入れてください。
   ![upload_model-upload-options](../images/upload_model-upload-options.avif)
7. 「リグ」タブに移動し表示されているチェックボックス全てをチェック状態にしてください。
   ![upload_model-rigging](../images/upload_model-rigging.avif)
   ::alert{type="warning"}
   ここにチェックを入れないと、アバターとして使用できません。
   ::
8. ここまでできたら、「費用を計算」ボタンを押してアップロード費用を計算し、アップロードしてください。
9. 自分を右クリックし、コンテキストメニューの「脱ぐ」から「全てを取り外す」を選択したら、インベントリに先ほどアップロードしたメッシュをダブルクリックし、装着してください。
   ::alert{type="info"}
   事前に全身アルファを入手しておく必要があります。これにより、システムアバターが表示されない状態にします。
   ::
10. アップロードしたモデルを装着したときに、そのままだと、図のようにアルファバグが発生してしまいます。そこで、アバターを装着した状態で、インベントリから装着しているオブジェクトを右クリックして「編集」をクリックしてください。
    ![edit_mesh](../images/edit_mesh.avif)
11. ビルドツールからマテリアルタブを選択します。（Firestormの場合、更にBlinn-Phongタブを選択する必要があります。）そこで、「アルファモード」を「アルファマスク」にし、「マスクのカットオフ」を127程度にしてください。これでアルファバグは解決します。
    ![build_tool_panel](../images/build_tool_panel.avif)

## コマンドラインでの使用方法

```bash
vrm2sl input.vrm output.glb --analyze-only --report report.json
```

## 注意事項

不特定多数に配布したり、マーケットプレイスで再販したい場合は、必ず**ライセンスを確認**してください。[Booth](https://booth.pm/ja/browse/VRoid)や[Vroid Hub](https://hub.vroid.com/)、[ニコニコモンズ](https://commons.nicovideo.jp/search?keywords=VROID&sort=created&order=desc)、[Etsy](https://www.etsy.com/jp/search?q=Vroid&ref=search_bar)で販売や配布されているアバターの多くは**二次利用や再配布、再販を禁じています**。
