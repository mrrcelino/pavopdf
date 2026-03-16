import { listen } from '@tauri-apps/api/event';
import type { ProgressEvent, Tool } from '../types';

interface Operation {
  id: string;
  tool: Tool;
  percent: number;
  message: string;
  status: 'running' | 'done' | 'error';
  errorMessage?: string;
}

let operations = $state<Map<string, Operation>>(new Map());

export const operationStore = {
  get all() { return [...operations.values()]; },
  get(id: string) { return operations.get(id); },

  start(id: string, tool: Tool) {
    const updated = new Map(operations);
    updated.set(id, { id, tool, percent: 0, message: 'Starting...', status: 'running' });
    operations = updated;
  },

  complete(id: string) {
    const op = operations.get(id);
    if (op) {
      const updated = new Map(operations);
      updated.set(id, { ...op, percent: 100, status: 'done' });
      operations = updated;
    }
  },

  fail(id: string, message: string) {
    const op = operations.get(id);
    if (op) {
      const updated = new Map(operations);
      updated.set(id, { ...op, status: 'error', errorMessage: message });
      operations = updated;
    }
  },

  clear(id: string) {
    const updated = new Map(operations);
    updated.delete(id);
    operations = updated;
  },
};

// Wire up Tauri event listeners
listen<ProgressEvent>('pdf-progress', ({ payload }) => {
  const op = operations.get(payload.operation_id);
  if (op) {
    const updated = new Map(operations);
    updated.set(payload.operation_id, { ...op, percent: payload.percent, message: payload.message });
    operations = updated;
  }
}).catch(console.error);

listen<{ operation_id: string }>('pdf-complete', ({ payload }) => {
  operationStore.complete(payload.operation_id);
}).catch(console.error);

listen<{ operation_id: string; message: string }>('pdf-error', ({ payload }) => {
  operationStore.fail(payload.operation_id, payload.message);
}).catch(console.error);
