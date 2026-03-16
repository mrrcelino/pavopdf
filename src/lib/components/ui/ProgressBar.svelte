<script lang="ts">
  interface Props { value: number; label?: string; }
  const { value, label = '' }: Props = $props();
</script>
<div class="flex flex-col gap-1">
  {#if label}
    <span class="text-xs text-gray-500">{label}</span>
  {/if}
  <div class="h-2 bg-gray-200 rounded-full overflow-hidden">
    <div class="h-full bg-teal-500 rounded-full transition-all" style="width: {Math.min(100, value)}%"></div>
  </div>
</div>
