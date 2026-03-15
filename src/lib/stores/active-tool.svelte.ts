import type { Tool, Category } from '../types';

let activeTool = $state<Tool | null>(null);
let activeCategory = $state<Category>('organise');
let view = $state<'dashboard' | 'workspace'>('dashboard');

export const activeToolStore = {
  get tool() { return activeTool; },
  get category() { return activeCategory; },
  get view() { return view; },

  selectTool(tool: Tool) {
    activeTool = tool;
    view = 'workspace';
  },

  setCategory(category: Category) {
    activeCategory = category;
  },

  goHome() {
    activeTool = null;
    view = 'dashboard';
  },
};
