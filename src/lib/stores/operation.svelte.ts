import { listen } from '@tauri-apps/api/event';
import type { ProgressEvent } from '../types';

interface Operation {
  id: string;
  tool: string;
  percent: number;
  message: string;
  status: 'running' | 'done' | 'error';
  errorMessage?: string;
}

let operations = $state<Map<string, Operation>>(new Map());

export const operationStore = {
  get all() { return [...operations.values()]; },
  get(id: string) { return operations.get(id); },

  start(id: string, tool: string) {
    operations.set(id, { id, tool, percent: 0, message: 'Starting...', status: 'running' });
    operations = new Map(operations);
  },

  complete(id: string) {
    const op = operations.get(id);
    if (op) {
      operations.set(id, { ...op, percent: 100, status: 'done' });
      operations = new Map(operations);
    }
  },

  fail(id: string, message: string) {
    const op = operations.get(id);
    if (op) {
      operations.set(id, { ...op, status: 'error', errorMessage: message });
      operations = new Map(operations);
    }
  },

  clear(id: string) {
    operations.delete(id);
    operations = new Map(operations);
  },
};

// Wire up Tauri event listeners
listen<ProgressEvent>('pdf-progress', ({ payload }) => {
  const op = operations.get(payload.operation_id);
  if (op) {
    operations.set(payload.operation_id, { ...op, percent: payload.percent, message: payload.message });
    operations = new Map(operations);
  }
});

listen<{ operation_id: string }>('pdf-complete', ({ payload }) => {
  operationStore.complete(payload.operation_id);
});

listen<{ operation_id: string; message: string }>('pdf-error', ({ payload }) => {
  operationStore.fail(payload.operation_id, payload.message);
});
