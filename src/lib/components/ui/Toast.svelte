<script lang="ts">
  import { operationStore } from '../../stores/operation.svelte';

  const doneOps = $derived(operationStore.all.filter(o => o.status === 'done'));
  const errorOps = $derived(operationStore.all.filter(o => o.status === 'error'));
</script>

<div class="fixed bottom-4 right-4 flex flex-col gap-2 z-50 pointer-events-none">
  {#each doneOps as op}
    <div class="bg-white border border-stone-200 rounded-lg px-4 py-2 shadow-lg pointer-events-auto
                flex items-center gap-2 text-sm text-stone-700">
      <span class="text-green-500">✓</span>
      <span>{op.tool} complete</span>
      <button
        onclick={() => operationStore.clear(op.id)}
        class="ml-2 text-stone-400 hover:text-stone-600"
      >✕</button>
    </div>
  {/each}

  {#each errorOps as op}
    <div class="bg-red-50 border border-red-200 rounded-lg px-4 py-2 shadow-lg pointer-events-auto
                flex items-center gap-2 text-sm text-red-700">
      <span>⚠</span>
      <span>{op.errorMessage ?? 'Operation failed'}</span>
      <button
        onclick={() => operationStore.clear(op.id)}
        class="ml-2 text-red-400 hover:text-red-600"
      >✕</button>
    </div>
  {/each}
</div>
