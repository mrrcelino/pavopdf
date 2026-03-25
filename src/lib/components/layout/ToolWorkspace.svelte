<script lang="ts">
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import { TOOLS } from '../../tools-registry';
  import CompressWorkspace from '../tools/organise/CompressWorkspace.svelte';
  import MergeWorkspace from '../tools/organise/MergeWorkspace.svelte';
  import RemoveWorkspace from '../tools/organise/RemoveWorkspace.svelte';
  import ReorderWorkspace from '../tools/organise/ReorderWorkspace.svelte';
  import RotateWorkspace from '../tools/organise/RotateWorkspace.svelte';
  import SplitWorkspace from '../tools/organise/SplitWorkspace.svelte';
  import PdfToWordWorkspace from '../tools/convert_from/PdfToWordWorkspace.svelte';
  import PdfToExcelWorkspace from '../tools/convert_from/PdfToExcelWorkspace.svelte';
  import PdfToPptWorkspace from '../tools/convert_from/PdfToPptWorkspace.svelte';
  import PdfToImageWorkspace from '../tools/convert_from/PdfToImageWorkspace.svelte';
  import PdfToPdfaWorkspace from '../tools/convert_from/PdfToPdfaWorkspace.svelte';
  import WordToPdfWorkspace from '../tools/convert_to/WordToPdfWorkspace.svelte';
  import ExcelToPdfWorkspace from '../tools/convert_to/ExcelToPdfWorkspace.svelte';
  import PptToPdfWorkspace from '../tools/convert_to/PptToPdfWorkspace.svelte';
  import ImageToPdfWorkspace from '../tools/convert_to/ImageToPdfWorkspace.svelte';
  import HtmlToPdfWorkspace from '../tools/convert_to/HtmlToPdfWorkspace.svelte';
  import MetadataWorkspace from '../tools/edit/MetadataWorkspace.svelte';
  import WatermarkWorkspace from '../tools/edit/WatermarkWorkspace.svelte';
  import PageNumbersWorkspace from '../tools/edit/PageNumbersWorkspace.svelte';
  import RedactWorkspace from '../tools/edit/RedactWorkspace.svelte';

  const tool = $derived(TOOLS.find(t => t.id === activeToolStore.tool));
</script>

<div class="flex-1 overflow-y-auto p-6 bg-cream">
  {#if tool}
    <div class="mb-1 text-xs text-stone-400">
      All Tools > <span class="text-teal font-semibold">{tool.label}</span>
    </div>
    <h1 class="text-xl font-bold text-stone-800 mb-4">{tool.label}</h1>

    {#if tool.id === 'merge'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <MergeWorkspace />
      </div>
    {:else if tool.id === 'split'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <SplitWorkspace />
      </div>
    {:else if tool.id === 'compress'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <CompressWorkspace />
      </div>
    {:else if tool.id === 'rotate'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <RotateWorkspace />
      </div>
    {:else if tool.id === 'reorder'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <ReorderWorkspace />
      </div>
    {:else if tool.id === 'remove'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <RemoveWorkspace />
      </div>
    {:else if tool.id === 'pdf_to_word'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <PdfToWordWorkspace />
      </div>
    {:else if tool.id === 'pdf_to_excel'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <PdfToExcelWorkspace />
      </div>
    {:else if tool.id === 'pdf_to_ppt'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <PdfToPptWorkspace />
      </div>
    {:else if tool.id === 'pdf_to_image'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <PdfToImageWorkspace />
      </div>
    {:else if tool.id === 'pdf_to_pdfa'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <PdfToPdfaWorkspace />
      </div>
    {:else if tool.id === 'word_to_pdf'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <WordToPdfWorkspace />
      </div>
    {:else if tool.id === 'excel_to_pdf'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <ExcelToPdfWorkspace />
      </div>
    {:else if tool.id === 'ppt_to_pdf'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <PptToPdfWorkspace />
      </div>
    {:else if tool.id === 'image_to_pdf'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <ImageToPdfWorkspace />
      </div>
    {:else if tool.id === 'html_to_pdf'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <HtmlToPdfWorkspace />
      </div>
    {:else if tool.id === 'edit'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <MetadataWorkspace />
      </div>
    {:else if tool.id === 'watermark'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <WatermarkWorkspace />
      </div>
    {:else if tool.id === 'page_numbers'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <PageNumbersWorkspace />
      </div>
    {:else if tool.id === 'redact'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <RedactWorkspace />
      </div>
    {:else}
      <div class="bg-white rounded-xl border border-stone-200 p-8 text-center text-stone-400">
        <div class="text-4xl mb-2">{tool.icon}</div>
        <p class="text-sm">{tool.description}</p>
        <p class="text-xs mt-2 text-stone-300">Implementation coming in Plans 4-6</p>
      </div>
    {/if}
  {/if}
</div>
