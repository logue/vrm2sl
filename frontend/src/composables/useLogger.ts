import { useGlobalStore } from '@/store';
import { onBeforeUnmount, onMounted, ref } from 'vue';

import { listen } from '@tauri-apps/api/event';

/** A single log entry emitted by the Rust backend. */
export interface LogEntry {
  /** Severity level (e.g. 'info', 'warn', 'error'). */
  level: string;
  /** Human-readable log message. */
  message: string;
  /** ISO 8601 timestamp string. */
  timestamp: string;
}

/** Maximum number of log entries retained in memory. */
const MAX_LOG_ENTRIES = 200;

/**
 * Composable that subscribes to backend `log-message` events and maintains a
 * reactive log history.
 *
 * @returns Reactive `logs` array and `clearLogs` helper.
 */
export function useLogger() {
  const globalStore = useGlobalStore();
  const logs = ref<LogEntry[]>([]);

  let unlisten: (() => void) | null = null;

  onMounted(async () => {
    unlisten = await listen<LogEntry>('log-message', event => {
      const entry = event.payload;
      logs.value.push(entry);
      if (logs.value.length > MAX_LOG_ENTRIES) {
        logs.value.splice(0, logs.value.length - MAX_LOG_ENTRIES);
      }

      if (entry.level === 'error') {
        globalStore.setMessage(entry.message, 'error');
      } else if (entry.level === 'info') {
        globalStore.setMessage(entry.message);
      }
    });
  });

  onBeforeUnmount(() => {
    unlisten?.();
    unlisten = null;
  });

  /**
   * Clear all retained log entries.
   */
  const clearLogs = () => {
    logs.value = [];
  };

  return { logs, clearLogs };
}
