# User Guide

## Basic Usage

1. First, export your avatar from VRoid Studio. At this point, select **VRM1.0** as the format. Reducing polygon count is recommended, but you do not need to worry about texture size because this tool automatically resizes textures to 1024.
   ![vroid_export](../images/vroid_export.avif)
2. Launch this tool, click the folder icon, and select the exported \*.vrm file.
3. After a short wait, the avatar appears in the preview area. Click the export button and save the \*.glb file to any location.
   ![vrm2sl](../images/vrm2sl.avif)
4. Open the Second Life viewer, then select World > Upload > Mesh. When the file dialog appears, choose the \*.glb file you exported earlier.
   ![upload_mesh_model](../images/upload_mesh_model.avif)
5. The model upload floater appears. In the Level of Detail (LoD) tab, set Medium, Low, and Lowest LoD values to 0. Also enable Generate Normals.
   ::alert{type="info"}
   This is not limited to avatars, but in Second Life it is recommended to set Medium, Low, and Lowest LoD to 0. This helps reduce land impact and complexity, and can also lower upload cost.
   ::
6. Next, move to the Upload Options tab and enable Include Textures.
   ![upload_model-upload-options](../images/upload_model-upload-options.avif)
7. Move to the Rigging tab and check all displayed checkboxes.
   ![upload_model-rigging](../images/upload_model-rigging.avif)
   ::alert{type="warning"}
   If these are not checked, the model cannot be used as an avatar.
   ::
8. Once done, click Calculate Fee to estimate the upload cost, then upload the model.
9. Right-click your avatar, choose Take Off > Remove All, then double-click the uploaded mesh in your inventory to wear it.
   ::alert{type="info"}
   You need a full-body alpha in advance. This hides the system avatar body.
   ::
10. After wearing the uploaded model, an alpha rendering issue may appear as shown. With the avatar still worn, right-click the attached object in inventory and click Edit.
    ![edit_mesh](../images/edit_mesh.avif)
11. In the build tool, open the Material tab. (In Firestorm, you may need to open the Blinn-Phong tab as well.) Set Alpha Mode to Alpha Mask and Mask Cutoff to around 127. This resolves the alpha issue.
    ![build_tool_panel](../images/build_tool_panel.avif)

## Command Line Usage

```bash
vrm2sl input.vrm output.glb --analyze-only --report report.json
```

## Notes

If you plan to distribute to the public or resell in a marketplace, make sure to **check the license**. Many avatars sold or distributed on [Booth](https://booth.pm/en/browse/VRoid), [Vroid Hub](https://hub.vroid.com/), [Niconico Commons](https://commons.nicovideo.jp/search?keywords=VROID&sort=created&order=desc), and [Etsy](https://www.etsy.com/search?q=Vroid&ref=search_bar) **prohibit derivative use, redistribution, or resale**.
