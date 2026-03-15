import { api } from '../api';
import type { RecentEntry } from '../types';

let entries = $state<RecentEntry[]>([]);

export const recentFilesStore = {
  get entries() { return entries; },

  async load() {
    entries = await api.getRecentFiles();
  },

  async remove(path: string) {
    await api.removeRecentFile(path);
    entries = entries.filter(e => e.path !== path);
  },
};
