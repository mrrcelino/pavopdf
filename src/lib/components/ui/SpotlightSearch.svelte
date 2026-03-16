<script lang="ts">
  import { searchTools } from '../../tools-registry';
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import type { ToolMeta } from '../../types';

  let { onClose }: { onClose: () => void } = $props();

  let query = $state('');
  let selected = $state(0);

  const results = $derived(query.length > 0 ? searchTools(query) : []);

  function pick(tool: ToolMeta) {
    activeToolStore.selectTool(tool.id);
    onClose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') { selected = Math.min(selected + 1, results.length - 1); }
    if (e.key === 'ArrowUp') { selected = Math.max(selected - 1, 0); }
    if (e.key === 'Enter' && results[selected]) { pick(results[selected]); }
  }
</script>

<!-- Backdrop -->
<div
  class="fixed inset-0 bg-black/30 z-50 flex items-start justify-center pt-24"
  onclick={onClose}
  role="dialog"
  aria-modal="true"
>
  <div
    class="w-full max-w-md bg-white rounded-xl shadow-2xl overflow-hidden"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="flex items-center gap-3 px-4 py-3 border-b border-stone-100">
      <span class="text-stone-400">🔍</span>
      <input
        type="text"
        placeholder="Search tools..."
        class="flex-1 outline-none text-sm text-stone-800 placeholder-stone-400"
        bind:value={query}
        onkeydown={handleKeydown}
        autofocus
      />
      <kbd class="text-xs text-stone-300 border border-stone-200 rounded px-1">Esc</kbd>
    </div>

    {#if results.length > 0}
      <ul class="max-h-64 overflow-y-auto py-1">
        {#each results as tool, i}
          <li>
            <button
              class="w-full flex items-center gap-3 px-4 py-2 text-left hover:bg-stone-50 transition-colors"
              class:bg-stone-50={i === selected}
              onclick={() => pick(tool)}
            >
              <span class="text-lg">{tool.icon}</span>
              <div>
                <div class="text-sm font-medium text-stone-800">{tool.label}</div>
                <div class="text-xs text-stone-400">{tool.description}</div>
              </div>
            </button>
          </li>
        {/each}
      </ul>
    {:else if query.length > 0}
      <div class="px-4 py-6 text-center text-sm text-stone-400">No tools found</div>
    {:else}
      <div class="px-4 py-6 text-center text-sm text-stone-400">Start typing to search tools...</div>
    {/if}
  </div>
</div>
