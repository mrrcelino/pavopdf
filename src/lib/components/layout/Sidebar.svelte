<script lang="ts">
  import { CATEGORIES, toolsByCategory } from '../../tools-registry';
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import { settingsStore } from '../../stores/settings.svelte';

  const collapsed = $derived(settingsStore.value.sidebar_collapsed);

  function toggleCollapse() {
    settingsStore.update({ sidebar_collapsed: !collapsed });
  }
</script>

<aside
  class="h-full flex flex-col flex-shrink-0 transition-all duration-200 overflow-hidden"
  class:w-12={collapsed}
  style="background: #1B7A8A; {!collapsed ? 'width: 120px;' : ''}"
>
  <!-- Logo -->
  <div class="px-3 py-3 border-b border-white/10 flex items-center gap-2">
    <span class="text-lg">🦚</span>
    {#if !collapsed}
      <span class="text-white font-bold text-sm">PavoPDF</span>
    {/if}
  </div>

  <!-- Home link -->
  <button
    onclick={() => activeToolStore.goHome()}
    class="px-3 py-2 text-white/50 text-xs hover:text-white/80 text-left"
  >
    {collapsed ? '⌂' : '← Home'}
  </button>

  <!-- Tool groups -->
  <nav class="flex-1 overflow-y-auto py-1">
    {#each CATEGORIES as cat}
      {@const tools = toolsByCategory(cat.id)}
      {#if !collapsed}
        <div class="px-3 py-1 text-white/40 text-[10px] uppercase tracking-wider mt-2">
          {cat.label}
        </div>
      {/if}
      {#each tools as tool}
        <button
          onclick={() => activeToolStore.selectTool(tool.id)}
          class={[
            'flex items-center gap-2 rounded-md transition-colors',
            collapsed ? 'mx-auto justify-center w-8 h-7' : 'w-full px-2 py-1',
            activeToolStore.tool === tool.id ? 'text-white' : 'text-white/60',
          ].join(' ')}
          style={activeToolStore.tool === tool.id ? 'background: rgba(255,255,255,0.15);' : ''}
          title={collapsed ? tool.label : undefined}
        >
          <span class="text-sm flex-shrink-0">{tool.icon}</span>
          {#if !collapsed}
            <span class="text-xs truncate">{tool.label}</span>
          {/if}
        </button>
      {/each}
    {/each}
  </nav>

  <!-- Collapse toggle -->
  <button
    onclick={toggleCollapse}
    class="px-3 py-2 text-white/40 text-xs hover:text-white/70 border-t border-white/10 text-left"
    title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
  >
    {collapsed ? '›' : '‹ collapse'}
  </button>
</aside>
