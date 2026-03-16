<script lang="ts">
  import { CATEGORIES, TOOLS } from '../../tools-registry';
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import { recentFilesStore } from '../../stores/recent-files.svelte';
  import type { Category } from '../../types';

  let { onOpenSpotlight }: { onOpenSpotlight: () => void } = $props();

  let selectedCategory = $state<Category | 'all'>('all');

  const visibleTools = $derived(
    selectedCategory === 'all'
      ? TOOLS
      : TOOLS.filter(t => t.category === selectedCategory)
  );
</script>

<div class="flex flex-col h-full overflow-hidden">
  <!-- Top bar -->
  <header style="background: #1B7A8A;" class="flex items-center gap-3 px-4 py-2 flex-shrink-0">
    <span class="text-white font-bold text-sm">🦚 PavoPDF</span>

    <!-- Category tabs -->
    <div class="flex gap-1 ml-2">
      <button
        onclick={() => selectedCategory = 'all'}
        class={[
          'px-2 py-1 rounded text-xs transition-colors',
          selectedCategory === 'all' ? 'text-white' : 'text-white/60',
        ].join(' ')}
        style={selectedCategory === 'all' ? 'background: rgba(255,255,255,0.20);' : ''}
      >All</button>
      {#each CATEGORIES as cat}
        <button
          onclick={() => selectedCategory = cat.id}
          class={[
            'px-2 py-1 rounded text-xs transition-colors',
            selectedCategory === cat.id ? 'text-white' : 'text-white/60',
          ].join(' ')}
          style={selectedCategory === cat.id ? 'background: rgba(255,255,255,0.20);' : ''}
        >{cat.label}</button>
      {/each}
    </div>

    <!-- Spotlight trigger -->
    <button
      onclick={onOpenSpotlight}
      class="ml-auto text-xs text-white/70 bg-white/15 border border-white/20 rounded px-3 py-1 hover:bg-white/20 transition-colors"
    >
      ⌘K Search tools...
    </button>
  </header>

  <div class="flex-1 overflow-y-auto p-4">
    <!-- Tool grid -->
    <div class="grid grid-cols-6 gap-3 mb-6">
      {#each visibleTools as tool}
        <button
          onclick={() => activeToolStore.selectTool(tool.id)}
          class="bg-white rounded-lg p-3 text-center border border-stone-200 hover:border-peach hover:shadow-sm transition-all group"
          title={tool.description}
        >
          <span class="text-xl block mb-1">{tool.icon}</span>
          <span class="text-xs text-stone-700 group-hover:text-peach transition-colors">{tool.label}</span>
        </button>
      {/each}
    </div>

    <!-- Recent files -->
    {#if recentFilesStore.entries.length > 0}
      <div>
        <h3 class="text-xs uppercase tracking-wider text-stone-400 mb-2">Recent</h3>
        <div class="flex flex-col gap-1">
          {#each recentFilesStore.entries as entry}
            <div
              class="flex items-center gap-3 bg-white rounded-lg px-3 py-2 border border-stone-200"
              class:opacity-50={!entry.exists}
            >
              <div class="w-2 h-2 rounded-sm flex-shrink-0" style="background: #E8956A;"></div>
              <span class="text-xs text-stone-700 flex-1 truncate">{entry.path.split(/[/\\]/).pop()}</span>
              <span class="text-xs text-stone-400">{entry.tool}</span>
              {#if !entry.exists}
                <span class="text-xs text-red-400">not found</span>
                <button
                  onclick={() => recentFilesStore.remove(entry.path)}
                  class="text-xs text-stone-400 hover:text-red-500"
                >✕</button>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>
