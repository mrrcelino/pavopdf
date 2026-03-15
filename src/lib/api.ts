import { invoke } from '@tauri-apps/api/core';
import type { Settings, RecentEntry, ProcessRequest } from './types';

export const api = {
  getSettings: () => invoke<Settings>('get_settings'),
  setSettings: (settings: Settings) => invoke<void>('set_settings', { settings }),

  getRecentFiles: () => invoke<RecentEntry[]>('get_recent_files'),
  removeRecentFile: (path: string) => invoke<void>('remove_recent_file', { path }),

  openFileDialog: (multiple: boolean) => invoke<string[]>('open_file_dialog', { multiple }),
  saveFileDialog: (suggestedName: string) => invoke<string | null>('save_file_dialog', { suggestedName }),

  processPdf: (request: ProcessRequest) => invoke<string>('process_pdf', { request }),
};
