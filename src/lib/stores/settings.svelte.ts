import { api } from '../api';
import type { Settings } from '../types';

const defaults: Settings = {
  sidebar_collapsed: false,
  default_output_folder: null,
  ocr_language: 'eng',
  auto_updater_enabled: false,
};

let settings = $state<Settings>(defaults);
let loaded = $state(false);

export const settingsStore = {
  get value() { return settings; },
  get loaded() { return loaded; },

  async load() {
    settings = await api.getSettings();
    loaded = true;
  },

  async update(patch: Partial<Settings>) {
    settings = { ...settings, ...patch };
    await api.setSettings(settings);
  },
};
