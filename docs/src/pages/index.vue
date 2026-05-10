<script setup lang="ts">
import logo from '@/assets/logo.png';
const { locale, rt, t, tm } = useI18n();
const localePath = useLocalePath();

// Composables
const { version, downloads, primaryDownload, detectPlatform } = useDownloads();
const { leadDescriptions } = useContentData();
const { languages, setupSeoMeta } = useSeoMetadata();

// import.meta.env はテンプレートで直接使えないので変数に取り出す
const appName = import.meta.env.VITE_APP_NAME || 'VRM2SL';

const notices = computed(() => {
  try {
    const content = tm('notice.content') as unknown;
    return Array.isArray(content) ? (content as string[]) : [];
  } catch {
    return [];
  }
});

// OS検出
onMounted(() => {
  detectPlatform();
});

// SEOメタデータの設定
setupSeoMeta();
</script>

<template>
  <v-card class="mb-6 bg-transparent mx-auto" flat tag="section" max-width="960">
    <v-img :src="logo" alt="App Logo" class="mx-auto my-4" height="256px" />
    <v-card-title class="text-h4 text-center pa-3" tag="h2">
      {{ appName }}
    </v-card-title>
    <v-card-subtitle class="text-center pb-4">{{ t('lead.subtitle') }}</v-card-subtitle>
    <v-card-text class="text-center">
      <!-- Language Links -->
      <v-chip-group class="flex justify-center mb-6" tag="nav">
        <v-chip
          v-for="lang in languages"
          :key="lang.code"
          :hreflang="lang.code"
          :to="lang.code === 'en' ? '/' : `/${lang.code}`"
          :variant="locale === lang.code ? 'elevated' : 'outlined'"
          :color="locale === lang.code ? 'primary' : 'default'"
          :text="lang.name"
          class="block mx-auto"
          rel="alternate"
          size="small"
        />
      </v-chip-group>
      <p v-for="(description, index) in leadDescriptions" :key="`lead-${index}`">
        {{ rt(description) }}
      </p>
    </v-card-text>
    <v-card-actions class="justify-center">
      <v-btn
        :to="localePath('/user-guide')"
        class="ma-4 px-8"
        color="primary"
        prepend-icon="mdi-rocket"
        size="large"
        variant="elevated"
      >
        {{ t('lead.start_button') }}
      </v-btn>
    </v-card-actions>
  </v-card>

  <v-card class="mb-6 bg-transparent" flat tag="section">
    <v-card-title class="text-h5 text-center" tag="h2">
      {{ t('download.download') }}
    </v-card-title>
    <v-card-subtitle class="text-center">
      <v-code>v.{{ version }}</v-code>
    </v-card-subtitle>

    <!-- Primary Download Button (Auto-detected) -->
    <v-card-actions class="justify-center mb-4 d-flex flex-column">
      <v-btn
        :href="primaryDownload.url"
        :prepend-icon-color="primaryDownload.iconColor"
        :prepend-icon="primaryDownload.icon"
        class="px-8 py-4"
        download
        height="100"
        size="x-large"
        spaced="both"
        stacked
        variant="elevated"
      >
        <span class="text-center">
          <div class="text-h6 mb-1 text-primary">{{ primaryDownload.label }}</div>
          <small class="text-medium-emphasis">{{ primaryDownload.subtitle }}</small>
        </span>
      </v-btn>
      <br />
      <!-- Alternative Downloads Expansion Panel -->
      <v-expansion-panels elevation="2">
        <v-expansion-panel>
          <v-expansion-panel-title>
            <v-icon start>mdi-package-variant</v-icon>
            {{ t('download.other_platforms') }}
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <v-row>
              <!-- Windows -->
              <v-col cols="12" md="6">
                <v-list density="compact">
                  <v-list-subheader>
                    <v-icon start color="blue">mdi-microsoft-windows</v-icon>
                    {{ t('download.windows') }}
                  </v-list-subheader>
                  <v-list-item
                    :href="downloads.windows.x64"
                    download
                    prepend-icon="mdi-download"
                    subtitle=".msi installer"
                    title="Windows 10/11 (x64)"
                  />
                </v-list>
              </v-col>
              <!-- macOS -->
              <v-col cols="12" md="6">
                <v-list density="compact">
                  <v-list-subheader>
                    <v-icon start color="red">mdi-apple</v-icon>
                    {{ t('download.macos') }}
                  </v-list-subheader>
                  <v-list-item
                    :href="downloads.macos.universal"
                    :subtitle="t('download.recommended')"
                    :title="t('download.macos_universal')"
                    download
                    prepend-icon="mdi-download"
                  />
                </v-list>
              </v-col>
              <!-- Linux x86_64 -->
              <v-col cols="12" md="6">
                <v-list density="compact">
                  <v-list-subheader>
                    <v-icon start color="orange">mdi-linux</v-icon>
                    {{ t('download.linux') }} - x86_64
                  </v-list-subheader>
                  <!--v-list-item
                    :href="downloads.linux.x64.appimage"
                    :subtitle="t('download.linux_appimage_desc')"
                    download
                    prepend-icon="mdi-download"
                    title="AppImage (x86_64)"
                  /-->
                  <v-list-item
                    :href="downloads.linux.x64.deb"
                    download
                    prepend-icon="mdi-download"
                    subtitle="Debian / Ubuntu"
                    title=".deb (x86_64)"
                  />
                  <v-list-item
                    :href="downloads.linux.x64.rpm"
                    prepend-icon="mdi-download"
                    download
                    title=".rpm (x86_64)"
                    subtitle="Fedora / RHEL / openSUSE"
                  />
                </v-list>
              </v-col>
              <!-- Linux ARM64 -->
              <v-col cols="12" md="6">
                <v-list density="compact">
                  <v-list-subheader class="px-0">
                    <v-icon start color="orange">mdi-linux</v-icon>
                    {{ t('download.linux') }} - ARM64
                  </v-list-subheader>
                  <!--v-list-item
                    :href="downloads.linux.arm64.appimage"
                    :subtitle="t('download.linux_appimage_desc')"
                    download
                    prepend-icon="mdi-download"
                    title="AppImage (ARM64)"
                  /-->
                  <v-list-item
                    :href="downloads.linux.arm64.deb"
                    prepend-icon="mdi-download"
                    download
                    title=".deb (ARM64)"
                    subtitle="Debian / Ubuntu"
                  />
                  <v-list-item
                    :href="downloads.linux.arm64.rpm"
                    prepend-icon="mdi-download"
                    download
                    title=".rpm (ARM64)"
                    subtitle="Fedora / RHEL / openSUSE"
                  />
                </v-list>
              </v-col>
            </v-row>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>
    </v-card-actions>
  </v-card>

  <v-card class="mb-6 bg-transparent" flat tag="section">
    <v-card-title class="text-h5 text-center" tag="h2">
      {{ t('notice.title') }}
    </v-card-title>
    <v-card-text>
      <ul>
        <li v-for="(item, index) in notices" :key="index">
          {{ rt(item) }}
        </li>
      </ul>
    </v-card-text>
  </v-card>
</template>

<i18n lang="yaml">
en:
  lead:
    subtitle: Vrm2sl is vrm avatar to second life avatar converter.
    description:
      - This software exports VRM avatars created with VRoid or similar software into glTF (.gdb) format, which is compatible with Second Life.
      - It is designed to be user-friendly and efficient, allowing users to easily convert their VRM avatars for use in Second Life.
      - The application is built using Tauri v2 and Vue 3, combining the power of Rust with the flexibility of web technologies to provide a seamless experience for users on Windows, macOS, and Linux.
    start_button: Get Started
  download:
    download: Download
    windows: Download for Windows
    window_requirement: Windows 11 or later
    macos: Download for macOS
    macos_universal: Universal Binary (Recommended)
    linux: Download for Linux
    linux_x64: x86_64
    linux_arm64: ARM64
    other_platforms: Other Platforms & Formats
    recommended: Recommended
    select_platform: Select your platform
    linux_appimage_desc: Portable, distribution-independent
  notice:
    title: Notice
    content:
      - If you plan to sell your converted avatar on marketplaces, please check the license of the assets. Many assets sold on platforms like Booth do not allow commercial use or prohibit redistribution, which may prevent you from selling your converted avatar.
      - When uploading, it is recommended to set the LoD (Level of Detail) below Medium to 0.
      - Since the resolution of images will be automatically converted to 1024x1024 or lower, the upload cost will be around L$ 165 including textures.
fr:
  lead:
    subtitle: Application de bureau multiplateforme moderne
    start_button: Commencer
    description:
      - Cette application exporte les avatars VRM créés avec VRoid ou des logiciels similaires au format glTF (.gdb), compatible avec Second Life.
      - Elle est conçue pour être conviviale et efficace, permettant aux utilisateurs de convertir facilement leurs avatars VRM pour une utilisation dans Second Life.
      - L'application est construite avec Tauri v2 et Vue 3, combinant la puissance de Rust avec la flexibilité des technologies web pour offrir une expérience fluide aux utilisateurs sur Windows, macOS et Linux.
  download:
    download: Télécharger
    windows: Télécharger pour Windows
    window_requirement: Windows 11 ou ultérieur
    macos: Télécharger pour macOS
    macos_universal: Binaire Universel
    linux: Télécharger pour Linux
    linux_x64: x86_64
    linux_arm64: ARM64
    other_platforms: Autres Plateformes et Formats
    recommended: Recommandé
    select_platform: Sélectionnez votre plateforme
    linux_appimage_desc: Portable, indépendant de la distribution
  notice:
    title: Avis
    content:
      - Si vous prévoyez de vendre votre avatar converti sur des marchés, veuillez vérifier la licence des actifs. De nombreux actifs vendus sur des plateformes comme Booth n'autorisent pas l'utilisation commerciale ou interdisent la redistribution, ce qui peut vous empêcher de vendre votre avatar converti.
      - Lors du téléchargement, il est recommandé de définir le LoD (Level of Detail) en dessous de Medium à 0.
      - Étant donné que la résolution des images sera automatiquement convertie à 1024x1024 ou moins, le coût de téléchargement sera d'environ L$ 165, y compris les textures.
ja:
  lead:
    subtitle: モダンなクロスプラットフォームデスクトップアプリケーション
    start_button: はじめに
    description:
      - Vroidなどで作成したVRM形式のアバターをSecondLifeで読み込み可能なglTF（.gdb）形式にして出力します。
      - ユーザーフレンドリーで効率的な設計で、ユーザーが簡単にVRMアバターをSecondLife用に変換できるようにします。
      - Tauri v2とVue 3を使用して構築されており、RustのパワーとWeb技術の柔軟性を組み合わせて、Windows、macOS、Linuxのユーザーにシームレスな体験を提供します。
  download:
    download: ダウンロード
    windows: Windows版をダウンロード
    window_requirement: Windows 11以降
    macos: macOS版をダウンロード
    macos_universal: ユニバーサルバイナリ
    linux: Linux版をダウンロード
    linux_x64: x86_64
    linux_arm64: ARM64
    other_platforms: その他のプラットフォームと形式
    recommended: 推奨
    select_platform: プラットフォームを選択
    linux_appimage_desc: ポータブル、ディストリビューション非依存
  notice:
    title: 注意事項
    content:
      - 変換したアバターをマーケットプレイスで販売したい場合は、アセットのライセンスを確認してください。
      - Boothなどで販売されているアセットの多くは、商用利用を許可していないか、二次配布を禁止しているため、変換したアバターの販売ができない可能性があります。
      - アップロード時は、中以下のLoD（詳細度）は0にすることを推奨します。
      - 画像の解像度は1024x1024以下に自動変換されるため、アップロード費用はテクスチャ込みで大体、L$ 165ぐらいになります。
ko:
  lead:
    subtitle: 현대적인 크로스 플랫폼 데스크톱 애플리케이션
    start_button: 시작하기
    description:
      - Vroid 등으로 만든 VRM 아바타를 Second Life에서 읽을 수 있는 glTF(.gdb) 형식으로 내보냅니다.
      - 사용자 친화적이고 효율적인 설계로 사용자가 VRM 아바타를 쉽게 Second Life용으로 변환할 수 있도록 합니다.
      - Tauri v2와 Vue 3을 사용하여 구축되었으며, Rust의 강력함과 웹 기술의 유연성을 결합하여 Windows, macOS, Linux 사용자에게 원활한 경험을 제공합니다.
  download:
    download: 다운로드
    windows: Windows용 다운로드
    window_requirement: Windows 11 이상
    macos: macOS용 다운로드
    macos_universal: 유니버설 바이너리
    linux: Linux용 다운로드
    linux_x64: x86_64
    linux_arm64: ARM64
    other_platforms: 기타 플랫폼 및 형식
    recommended: 권장
    select_platform: 플랫폼 선택
    linux_appimage_desc: 휴대 가능, 배포판 독립적
  notice:
    title: 주의 사항
    content:
      - 변환한 아바타를 마켓플레이스에서 판매하려는 경우, 자산의 라이선스를 확인하십시오. Booth와 같은 플랫폼에서 판매되는 많은 자산은 상업적 사용을 허용하지 않거나 재배포를 금지하므로 변환한 아바타를 판매할 수 없을 수 있습니다.
      - 업로드 시, LoD(Level of Detail)를 Medium 이하로 설정하는 것이 권장됩니다.
      - 이미지 해상도는 자동으로 1024x1024 이하로 변환되므로, 업로드 비용은 텍스처를 포함하여 약 L$ 165 정도입니다。
zhHant:
  lead:
    subtitle: 現代跨平台桌面應用程式
    start_button: 入門
    description:
      - 使用 VRoid 等軟體創建的 VRM 角色將被導出為與 Second Life 兼容的 glTF (.gdb) 格式。
      - 設計為用戶友好且高效，使用戶能夠輕鬆地將 VRM 角色轉換為 Second Life 使用。
      - 該應用程式使用 Tauri v2 和 Vue 3 構建，結合了 Rust 的強大功能和 Web 技術的靈活性，為 Windows、macOS 和 Linux 用戶提供無縫的體驗。
  download:
    download: 下載
    windows: 下載 Windows 版
    window_requirement: Windows 11 或更新版本
    macos: 下載 MacOS 版
    macos_universal: 通用二進位檔
    linux: 下載 Linux 版
    linux_x64: x64
    linux_arm64: ARM64
  notice:
    title: 注意事項
    content:
      - 如果您打算在市場上銷售轉換後的角色，請檢查資產的許可證。許多在 Booth 等平台上銷售的資產不允許商業使用或禁止再分發，這可能會阻止您銷售轉換後的角色。
      - 上傳時，建議將 LoD（細節級別）設置為 Medium 以下為 0。
      - 由於圖像的解析度將自動轉換為 1024x1024 或更低，因此上傳成本將約為 L$ 165，包括紋理。
zhHans:
  lead:
    subtitle: 现代跨平台桌面应用程序
    start_button: 入门
    description:
      - 使用 VRoid 等软件创建的 VRM 角色将被导出为与 Second Life 兼容的 glTF (.gdb) 格式。
      - 设计为用户友好且高效，使用户能够轻松地将 VRM 角色转换为 Second Life 使用。
      - 该应用程序使用 Tauri v2 和 Vue 3 构建，结合了 Rust 的强大功能和 Web 技术的灵活性，为 Windows、macOS 和 Linux 用户提供无缝的体验。
  download:
    download: 下载安装
    windows: 下载安装 Windows 版
    window_requirement: Windows 11 或更新版本
    macos: 下载安装 MacOS 版
    macos_universal: 通用二进制文件
    linux: 下载安装 Linux 版
    linux_x64: x64
    linux_arm64: ARM64
  notice:
    title: 注意事项
    content:
      - 如果您打算在市场上销售转换后的角色，请检查资产的许可证。许多在 Booth 等平台上销售的资产不允许商业使用或禁止再分发，这可能会阻止您销售转换后的角色。
      - 上传时，建议将 LoD（细节级别）设置为 Medium 以下为 0。
      - 由于图像的分辨率将自动转换为 1024x1024 或更低，因此上传成本将约为 L$ 165，包括纹理。
</i18n>
