# 사용자 가이드

## 기본 사용 방법

1. 먼저 VRoid Studio에서 아바타를 내보냅니다. 이때 형식은 **VRM1.0**을 선택하세요. 폴리곤 수를 줄이는 것을 권장하지만, 텍스처 크기는 이 도구에서 자동으로 1024로 축소하므로 별도로 신경 쓰지 않아도 됩니다.
   ![vroid_export](../images/vroid_export.avif)
2. 이 도구를 실행하고 폴더 아이콘을 클릭한 뒤, 방금 내보낸 \*.vrm 파일을 선택합니다.
3. 잠시 후 미리보기 영역에 아바타가 표시되면 내보내기 버튼을 눌러 원하는 위치에 \*.glb 파일을 저장하세요.
   ![vrm2sl](../images/vrm2sl.avif)
4. Second Life 뷰어를 실행하고 월드 > 업로드 > 메시를 선택합니다. 파일 선택 대화상자가 열리면 앞에서 내보낸 \*.glb 파일을 선택하세요.
   ![upload_mesh_model](../images/upload_mesh_model.avif)
5. 모델 업로드 창이 표시됩니다. 상세도(LoD) 탭에서 중간, 낮음, 최저 LoD 값을 각각 0으로 설정합니다. 또한 노멀 생성 옵션을 체크하세요.
   ::alert{type="info"}
   아바타에만 해당되는 내용은 아니지만, Second Life에서는 중간, 낮음, 최저 LoD를 0으로 설정하는 것을 권장합니다. 이렇게 하면 랜드 임팩트와 복잡도를 줄일 수 있고 업로드 비용도 절감할 수 있습니다.
   ::
6. 다음으로 업로드 옵션 탭으로 이동해 텍스처 포함을 체크하세요.
   ![upload_model-upload-options](../images/upload_model-upload-options.avif)
7. 리깅 탭으로 이동해 표시된 체크박스를 모두 체크 상태로 만드세요.
   ![upload_model-rigging](../images/upload_model-rigging.avif)
   ::alert{type="warning"}
   여기에 체크하지 않으면 아바타로 사용할 수 없습니다.
   ::
8. 여기까지 완료되면 비용 계산 버튼을 눌러 업로드 비용을 확인한 뒤 업로드하세요.
9. 자신의 아바타를 우클릭하고 벗기 > 모두 제거를 선택한 다음, 인벤토리에서 방금 업로드한 메시를 더블클릭하여 착용하세요.
   ::alert{type="info"}
   전신 알파를 사전에 준비해야 합니다. 이를 통해 시스템 아바타 바디를 보이지 않게 할 수 있습니다.
   ::
10. 업로드한 모델을 착용하면 그대로는 그림과 같은 알파 문제가 발생할 수 있습니다. 아바타를 착용한 상태에서 인벤토리의 착용 중인 오브젝트를 우클릭하고 편집을 클릭하세요.
    ![edit_mesh](../images/edit_mesh.avif)
11. 빌드 도구에서 머티리얼 탭을 선택합니다. (Firestorm의 경우 Blinn-Phong 탭을 추가로 선택해야 할 수 있습니다.) 알파 모드를 알파 마스크로 설정하고 마스크 컷오프를 127 정도로 맞추면 알파 문제가 해결됩니다.
    ![build_tool_panel](../images/build_tool_panel.avif)

## 명령줄 사용 방법

```bash
vrm2sl input.vrm output.glb --analyze-only --report report.json
```

## 주의 사항

불특정 다수에게 배포하거나 마켓플레이스에서 재판매하려면 반드시 **라이선스를 확인**하세요. [Booth](https://booth.pm/ko/browse/VRoid), [Vroid Hub](https://hub.vroid.com/), [Niconico Commons](https://commons.nicovideo.jp/search?keywords=VROID&sort=created&order=desc), [Etsy](https://www.etsy.com/search?q=Vroid&ref=search_bar)에서 판매 또는 배포되는 많은 아바타는 **2차 이용, 재배포, 재판매를 금지**하고 있습니다.
