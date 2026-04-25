# 用户指南

## 基本用法

1. 首先，在 VRoid Studio 中导出你的头像。此时请将格式选择为 **VRM1.0**。建议减少多边形数量；但纹理尺寸无需特别处理，因为本工具会自动缩小到 1024。
   ![vroid_export](../images/vroid_export.avif)
2. 启动本工具，点击文件夹图标，选择刚刚导出的 \*.vrm 文件。
3. 稍等片刻后，头像会显示在预览区域。点击导出按钮，将 \*.glb 文件保存到任意位置。
   ![vrm2sl](../images/vrm2sl.avif)
4. 启动 Second Life 查看器，选择 世界 > 上传 > 网格。出现文件选择对话框后，选择先前导出的 \*.glb 文件。
   ![upload_mesh_model](../images/upload_mesh_model.avif)
5. 会显示模型上传窗口。在 细节等级（LoD） 选项卡中，将中、低、最低 LoD 数值都设为 0。同时勾选 生成法线。
   ::alert{type="info"}
   这不仅适用于头像。在 Second Life 中，建议将中、低、最低 LoD 设为 0。这样不仅能降低地块影响和复杂度，也可以减少上传费用。
   ::
6. 接着切换到 上传选项 选项卡，并勾选 包含纹理。
   ![upload_model-upload-options](../images/upload_model-upload-options.avif)
7. 切换到 绑定 选项卡，将显示的所有复选框全部勾选。
   ![upload_model-rigging](../images/upload_model-rigging.avif)
   ::alert{type="warning"}
   如果这里不勾选，将无法作为头像使用。
   ::
8. 完成以上设置后，点击 计算费用 按钮计算上传费用，然后执行上传。
9. 右键点击自己，选择 脱下 > 全部移除，然后在物品栏中双击刚上传的网格进行穿戴。
   ::alert{type="info"}
   需要提前准备全身 Alpha。这样可以隐藏系统头像身体。
   ::
10. 穿戴上传后的模型时，若保持默认设置，可能会出现图中所示的 Alpha 问题。请在头像已穿戴状态下，在物品栏中右键已穿戴对象并点击 编辑。
    ![edit_mesh](../images/edit_mesh.avif)
11. 在建造工具中打开 材质 选项卡。（Firestorm 还可能需要额外打开 Blinn-Phong 选项卡。）将 Alpha 模式设为 Alpha Mask，并将 Mask Cutoff 设为约 127，即可解决 Alpha 问题。
    ![build_tool_panel](../images/build_tool_panel.avif)

## 命令行用法

```bash
vrm2sl input.vrm output.glb --analyze-only --report report.json
```

## 注意事项

若要面向不特定多数进行分发，或在市场中转售，请务必**确认许可协议**。在 [Booth](https://booth.pm/zh-cn/browse/VRoid)、[Vroid Hub](https://hub.vroid.com/)、[Niconico Commons](https://commons.nicovideo.jp/search?keywords=VROID&sort=created&order=desc)、[Etsy](https://www.etsy.com/search?q=Vroid&ref=search_bar) 上销售或分发的许多头像，通常**禁止二次利用、再分发或转售**。
