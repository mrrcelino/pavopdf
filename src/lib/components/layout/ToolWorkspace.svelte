<script lang="ts">
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import { TOOLS } from '../../tools-registry';

  const tool = $derived(TOOLS.find(t => t.id === activeToolStore.tool));
</script>

<div class="flex-1 overflow-y-auto p-6 bg-cream">
  {#if tool}
    <div class="mb-1 text-xs text-stone-400">
      All Tools › <span class="text-teal font-semibold">{tool.label}</span>
    </div>
    <h1 class="text-xl font-bold text-stone-800 mb-4">{tool.label}</h1>

    <!-- Tool workspace content will be injected by Plans 2-6 -->
    <div class="bg-white rounded-xl border border-stone-200 p-8 text-center text-stone-400">
      <div class="text-4xl mb-2">{tool.icon}</div>
      <p class="text-sm">{tool.description}</p>
      <p class="text-xs mt-2 text-stone-300">Implementation coming in Plans 2–6</p>
    </div>
  {/if}
</div>
