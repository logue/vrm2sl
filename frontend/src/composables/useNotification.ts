import {
  isPermissionGranted,
  requestPermission,
  sendNotification
} from '@tauri-apps/plugin-notification';
import type { ComposerTranslation } from 'vue-i18n';

/**
 * デスクトップ通知を送信するためのcomposable
 */
export const useNotification = (t: ComposerTranslation) => {
  /**
   * 通知権限を要求し、通知を送信
   */
  const notify = async (title: string, body?: string, icon?: string) => {
    try {
      // 通知権限を確認
      let permissionGranted = await isPermissionGranted();

      // 権限がない場合は要求
      if (!permissionGranted) {
        const permission = await requestPermission();
        permissionGranted = permission === 'granted';
      }

      if (permissionGranted) {
        // 通知を送信
        await sendNotification({ title, body, icon });
      }
    } catch (error) {
      console.error('通知の送信に失敗しました:', error);
    }
  };

  /**
   * 画像変換完了通知
   */
  const success = async (message: string) => {
    // 画像変換処理が完了したことを通知
    await notify(t('notification.success.title'), message);
  };

  /**
   * エラー通知
   */
  const error = async (message: string) => {
    await notify(t('notification.error.title'), message);
  };

  return {
    notify,
    success,
    error
  };
};
