/**
 * Provides download link metadata and platform detection helpers.
 *
 * @returns Download metadata, detected platform refs, and helper functions.
 */
export const useDownloads = () => {
  const { t } = useI18n();
  const config = useRuntimeConfig();
  const version = config.public.appVersion as string;
  const appNameKebab = config.public.appNameKebab;
  const urlPrefix = `${import.meta.env.PROJECT_URL}/releases/download/${version}/${appNameKebab}_${version}_`;

  // OS detection state.
  const detectedOS = ref<'windows' | 'macos' | 'linux' | 'unknown'>('unknown');
  const detectedArch = ref<'x64' | 'arm64' | 'unknown'>('unknown');

  /**
   * Detects OS and CPU architecture on the client side.
   *
   * @returns Void.
   */
  const detectPlatform = () => {
    if (!import.meta.client) return;

    const userAgent = navigator.userAgent.toLowerCase();
    const platform = navigator.platform.toLowerCase();

    // OS detection.
    if (userAgent.includes('win')) {
      detectedOS.value = 'windows';
    } else if (userAgent.includes('mac')) {
      detectedOS.value = 'macos';
    } else if (userAgent.includes('linux')) {
      detectedOS.value = 'linux';
    }

    // Architecture detection.
    if (platform.includes('arm') || platform.includes('aarch64') || userAgent.includes('arm64')) {
      detectedArch.value = 'arm64';
    } else if (platform.includes('x86') || platform.includes('x64') || platform.includes('amd64')) {
      detectedArch.value = 'x64';
    }
  };

  // Download URL map by platform and architecture.
  const downloads = computed(() => ({
    windows: {
      x64: `${urlPrefix}x64_en-US.msi`
    },
    macos: {
      universal: `${urlPrefix}universal.dmg`,
      arm64: `${urlPrefix}aarch64.dmg`,
      x64: `${urlPrefix}x64.dmg`
    },
    linux: {
      x64: {
        appimage: `${urlPrefix}amd64.AppImage`,
        deb: `${urlPrefix}amd64.deb`,
        rpm: `${urlPrefix}x86_64.rpm`
      },
      arm64: {
        appimage: `${urlPrefix}arm64.AppImage`,
        deb: `${urlPrefix}arm64.deb`,
        rpm: `${urlPrefix}aarch64.rpm`
      }
    }
  }));

  // Primary download button payload for current platform.
  const primaryDownload = computed(() => {
    const os = detectedOS.value;
    const arch = detectedArch.value;

    if (os === 'windows') {
      return {
        label: t('download.windows'),
        icon: 'mdi-microsoft-windows',
        iconColor: 'blue',
        url: downloads.value.windows.x64,
        subtitle: t('download.window_requirement')
      };
    } else if (os === 'macos') {
      return {
        label: t('download.macos'),
        icon: 'mdi-apple',
        iconColor: 'red',
        url: downloads.value.macos.universal,
        subtitle: t('download.macos_universal')
      };
    } else if (os === 'linux') {
      return {
        label: t('download.linux'),
        icon: 'mdi-linux',
        iconColor: 'orange',
        url:
          arch === 'arm64'
            ? downloads.value.linux.arm64.appimage
            : downloads.value.linux.x64.appimage,
        subtitle: arch === 'arm64' ? t('download.linux_arm64') : t('download.linux_x64')
      };
    }

    // Default fallback when platform detection is unavailable.
    return {
      label: t('download.download'),
      icon: 'mdi-download',
      iconColor: 'primary',
      url: downloads.value.macos.universal,
      subtitle: t('download.select_platform')
    };
  });

  return {
    version,
    urlPrefix,
    detectedOS,
    detectedArch,
    detectPlatform,
    downloads,
    primaryDownload
  };
};
