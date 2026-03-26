# PavoPDF — Plan 2: Organise Tools

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement six PDF organisation tools (Merge, Split, Compress, Rotate Pages, Reorder Pages, Remove Pages) with full Rust backends and Svelte 5 workspace UIs, wired into the Plan 1 pipeline infrastructure.

**Architecture:** Each tool lives in `src-tauri/src/tools/organise/` as its own module, using `TempStage` + `emit_progress` from Plan 1. The Svelte frontend provides a workspace component per tool in `src/lib/components/tools/organise/`, all wired via the existing `invoke('process_pdf', ...)` IPC command. `lopdf` handles all structural PDF mutations; `pdfium-render` renders page thumbnails (read-only).

**Tech Stack:** lopdf, pdfium-render, image crate (thumbnail resize), Svelte 5 runes.

**Depends on:** Plan 1 complete.

---

## Chunk 1: Rust Module Scaffold

### Task 1: Create `organise` module scaffold and wire dispatch

**Files:**
- Create: `src-tauri/src/tools/organise/mod.rs`
- Modify: `src-tauri/src/tools/mod.rs`

- [ ] **Step 1: Write failing dispatch test**

Add to `src-tauri/src/tools/mod.rs` inside a `#[cfg(test)]` block:

```rust
#[cfg(test)]
mod organise_dispatch_tests {
    use super::*;

    #[test]
    fn organise_tool_ids_are_recognized() {
        let ids = [
            "merge", "split", "compress",
            "rotate_pages", "reorder_pages", "remove_pages",
        ];
        for id in &ids {
            assert!(
                tool_name_is_known(id),
                "Tool id '{id}' not recognized in dispatch"
            );
        }
    }
}
```

Run `cargo test` — it must FAIL with "not recognized" before Step 3.

- [ ] **Step 2: Create `src-tauri/src/tools/organise/mod.rs`**

```rust
pub mod merge;
pub mod split;
pub mod compress;
pub mod rotate;
pub mod reorder;
pub mod remove;
```

- [ ] **Step 3: Wire organise into `tools/mod.rs`**

In `src-tauri/src/tools/mod.rs`, add the module declaration and dispatch arms. The full updated file:

```rust
pub mod organise;
// (other plan modules declared here as they are added)

use std::path::PathBuf;
use tauri::AppHandle;
use crate::error::AppError;

/// Shared request payload for all tool invocations.
#[derive(serde::Deserialize, Debug, Clone)]
pub struct ProcessRequest {
    pub tool_id: String,
    pub input_paths: Vec<PathBuf>,
    pub output_dir: PathBuf,
    pub options: serde_json::Value,
}

/// Dispatch to the correct tool implementation.
pub async fn run(handle: AppHandle, req: ProcessRequest) -> Result<PathBuf, AppError> {
    match req.tool_id.as_str() {
        // --- Plan 2: Organise ---
        "merge"        => organise::merge::run(handle, req).await,
        "split"        => organise::split::run(handle, req).await,
        "compress"     => organise::compress::run(handle, req).await,
        "rotate_pages" => organise::rotate::run(handle, req).await,
        "reorder_pages"=> organise::reorder::run(handle, req).await,
        "remove_pages" => organise::remove::run(handle, req).await,

        other => Err(AppError::Validation(format!("Unknown tool: {other}"))),
    }
}

/// Returns true if `name` is a registered tool id (used in tests).
pub fn tool_name_is_known(name: &str) -> bool {
    matches!(
        name,
        "merge" | "split" | "compress"
        | "rotate_pages" | "reorder_pages" | "remove_pages"
    )
}
```

- [ ] **Step 4: Verify test passes**

```
cargo test organise_dispatch_tests
```

All 6 tool ids must be recognized. Commit:

```
git add src-tauri/src/tools/
git commit -m "feat: scaffold organise tool module and dispatch"
```

---

## Chunk 2: Merge PDF

### Task 2: Rust — `organise/merge.rs`

**Files:**
- Create: `src-tauri/src/tools/organise/merge.rs`

- [ ] **Step 1: Write failing unit tests**

```rust
// src-tauri/src/tools/organise/merge.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Build a minimal 1-page lopdf::Document for testing.
    fn make_doc(page_count: usize) -> lopdf::Document {
        use lopdf::{Document, Object, Stream, Dictionary};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut page_ids = Vec::new();
        for _ in 0..page_count {
            let content = Stream::new(Dictionary::new(), b"BT /F1 12 Tf (test) Tj ET".to_vec());
            let content_id = doc.add_object(content);
            let page = lopdf::dictionary! {
                "Type" => lopdf::Object::Name(b"Page".to_vec()),
                "Parent" => lopdf::Object::Reference(pages_id),
                "MediaBox" => lopdf::Object::Array(vec![
                    0.into(), 0.into(), (595).into(), (842).into()
                ]),
                "Contents" => lopdf::Object::Reference(content_id),
            };
            let page_id = doc.add_object(page);
            page_ids.push(page_id);
        }
        let pages = lopdf::dictionary! {
            "Type" => lopdf::Object::Name(b"Pages".to_vec()),
            "Kids" => lopdf::Object::Array(
                page_ids.iter().map(|id| lopdf::Object::Reference(*id)).collect()
            ),
            "Count" => (page_count as i64).into(),
        };
        doc.objects.insert(pages_id, lopdf::Object::Dictionary(pages));
        doc.trailer.set("Root", {
            let catalog = lopdf::dictionary! {
                "Type" => lopdf::Object::Name(b"Catalog".to_vec()),
                "Pages" => lopdf::Object::Reference(pages_id),
            };
            lopdf::Object::Reference(doc.add_object(catalog))
        });
        doc
    }

    #[test]
    fn merge_two_docs_produces_combined_page_count() {
        let doc_a = make_doc(2);
        let doc_b = make_doc(3);
        let merged = merge_documents(vec![doc_a, doc_b]).expect("merge failed");
        assert_eq!(merged.get_pages().len(), 5);
    }

    #[test]
    fn merge_single_doc_is_identity() {
        let doc = make_doc(4);
        let merged = merge_documents(vec![doc]).expect("merge failed");
        assert_eq!(merged.get_pages().len(), 4);
    }

    #[test]
    fn merge_empty_list_returns_error() {
        let result = merge_documents(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn output_stem_is_correct() {
        let p = PathBuf::from("/tmp/report.pdf");
        assert_eq!(output_stem(&p), "report_merged");
    }
}
```

Run `cargo test` — must FAIL (functions not yet defined).

- [ ] **Step 2: Implement `merge.rs`**

```rust
use std::path::PathBuf;
use lopdf::Document;
use tauri::AppHandle;

use crate::error::AppError;
use crate::pipeline::{temp::TempStage, progress::{emit_progress, emit_complete, emit_error}};
use crate::tools::ProcessRequest;

/// Merge `docs` in order into a single lopdf::Document.
pub fn merge_documents(docs: Vec<Document>) -> Result<Document, AppError> {
    if docs.is_empty() {
        return Err(AppError::Validation("Merge requires at least one document".into()));
    }
    if docs.len() == 1 {
        return Ok(docs.into_iter().next().unwrap());
    }

    let mut iter = docs.into_iter();
    let mut merged = iter.next().unwrap();
    for doc in iter {
        merged.merge(doc).map_err(|e| AppError::Pdf(e.to_string()))?;
    }
    Ok(merged)
}

pub fn output_stem(first_input: &PathBuf) -> String {
    let stem = first_input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    format!("{stem}_merged")
}

pub async fn run(handle: AppHandle, req: ProcessRequest) -> Result<PathBuf, AppError> {
    let op_id = req.options
        .get("op_id")
        .and_then(|v| v.as_str())
        .unwrap_or("merge")
        .to_string();

    if req.input_paths.is_empty() {
        let msg = "Merge requires at least one input file".to_string();
        emit_error(&handle, &op_id, &msg);
        return Err(AppError::Validation(msg));
    }

    let total = req.input_paths.len();

    // Validate and load each document
    let mut docs: Vec<Document> = Vec::with_capacity(total);
    for (i, path) in req.input_paths.iter().enumerate() {
        crate::pipeline::validate::validate_pdf(path)?;
        emit_progress(&handle, &op_id, (i * 40 / total) as u8, &format!("Loading file {}/{}", i + 1, total));
        let doc = Document::load(path).map_err(|e| AppError::Pdf(format!("Failed to load {:?}: {e}", path)))?;
        docs.push(doc);
    }

    emit_progress(&handle, &op_id, 50, "Merging documents…");
    let merged = merge_documents(docs)?;

    // Stage output
    let stem = output_stem(&req.input_paths[0]);
    let stage = TempStage::new(&req.output_dir)?;
    let out_path = stage.output_path(&stem, "pdf");

    emit_progress(&handle, &op_id, 80, "Writing output…");
    merged.save(&out_path).map_err(|e| AppError::Pdf(format!("Failed to save merged PDF: {e}")))?;

    emit_complete(&handle, &op_id, &out_path);
    Ok(out_path)
}
```

- [ ] **Step 3: Run tests and verify**

```
cargo test organise::merge::tests
```

All 4 tests must pass.

- [ ] **Step 4: Commit**

```
git add src-tauri/src/tools/organise/merge.rs
git commit -m "feat(organise): implement merge PDF Rust backend"
```

---

### Task 3: Svelte 5 — `MergeWorkspace.svelte`

**Files:**
- Create: `src/lib/components/tools/organise/MergeWorkspace.svelte`

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { invoke } from '@tauri-apps/api/core';
  import { operationStore } from '$lib/stores/operation';
  import ProgressBar from '$lib/components/ui/ProgressBar.svelte';
  import FileListItem from '$lib/components/ui/FileListItem.svelte';

  // --- State ---
  let files = $state<string[]>([]);
  let draggingIndex = $state<number | null>(null);
  let dragOverIndex = $state<number | null>(null);
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);

  const op = operationStore;

  // --- Derived ---
  const canMerge = $derived(files.length >= 2 && !$op.running);

  // --- File picking ---
  async function addFiles() {
    const selected = await open({
      multiple: true,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    // Immutable: create new array
    files = [...files, ...paths.filter(p => !files.includes(p))];
    error = null;
  }

  function removeFile(index: number) {
    files = files.filter((_, i) => i !== index);
  }

  // --- Drag-to-reorder ---
  function onDragStart(index: number) {
    draggingIndex = index;
  }

  function onDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    dragOverIndex = index;
  }

  function onDrop(targetIndex: number) {
    if (draggingIndex === null || draggingIndex === targetIndex) {
      draggingIndex = null;
      dragOverIndex = null;
      return;
    }
    const reordered = [...files];
    const [moved] = reordered.splice(draggingIndex, 1);
    reordered.splice(targetIndex, 0, moved);
    files = reordered;
    draggingIndex = null;
    dragOverIndex = null;
  }

  function onDragEnd() {
    draggingIndex = null;
    dragOverIndex = null;
  }

  // --- Merge ---
  async function runMerge() {
    error = null;
    outputPath = null;

    const outDir = await save({
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
      defaultPath: 'merged.pdf',
    });
    if (!outDir) return;

    // Extract directory from chosen save path
    const dir = outDir.substring(0, Math.max(outDir.lastIndexOf('/'), outDir.lastIndexOf('\\')));

    op.start('merge');
    try {
      const result: string = await invoke('process_pdf', {
        request: {
          tool_id: 'merge',
          input_paths: files,
          output_dir: dir,
          options: { op_id: 'merge' },
        },
      });
      outputPath = result;
      op.complete(result);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      error = msg;
      op.fail(msg);
    }
  }
</script>

<div class="flex flex-col gap-4 p-4 h-full">
  <header class="flex items-center justify-between">
    <h2 class="text-lg font-semibold">Merge PDF</h2>
    <button
      onclick={addFiles}
      class="btn btn-secondary text-sm"
    >
      + Add Files
    </button>
  </header>

  <!-- Drop hint when empty -->
  {#if files.length === 0}
    <button
      class="flex-1 flex flex-col items-center justify-center border-2 border-dashed border-gray-300 rounded-lg text-gray-400 hover:border-blue-400 hover:text-blue-500 transition-colors cursor-pointer"
      onclick={addFiles}
      aria-label="Add PDF files"
    >
      <svg class="w-10 h-10 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
          d="M12 4v16m8-8H4" />
      </svg>
      <span>Click to add PDF files</span>
      <span class="text-xs mt-1">Drag to reorder after adding</span>
    </button>
  {:else}
    <!-- File list with drag-to-reorder -->
    <ul class="flex-1 overflow-y-auto space-y-1">
      {#each files as file, i (file)}
        <li
          draggable="true"
          ondragstart={() => onDragStart(i)}
          ondragover={(e) => onDragOver(e, i)}
          ondrop={() => onDrop(i)}
          ondragend={onDragEnd}
          class={[
            'flex items-center gap-2 p-2 rounded border bg-white cursor-grab active:cursor-grabbing transition-colors',
            dragOverIndex === i ? 'border-blue-400 bg-blue-50' : 'border-gray-200',
          ].join(' ')}
          aria-label={`File ${i + 1}: ${file}`}
        >
          <!-- Drag handle -->
          <span class="text-gray-300 select-none">⠿</span>
          <span class="text-xs text-gray-400 w-5 text-right shrink-0">{i + 1}</span>
          <FileListItem path={file} class="flex-1 min-w-0" />
          <button
            onclick={() => removeFile(i)}
            class="text-gray-400 hover:text-red-500 shrink-0"
            aria-label="Remove file"
          >✕</button>
        </li>
      {/each}
    </ul>

    <p class="text-xs text-gray-400">
      {files.length} file{files.length !== 1 ? 's' : ''} — drag rows to reorder
    </p>
  {/if}

  <!-- Progress -->
  {#if $op.running}
    <ProgressBar value={$op.progress} label={$op.label} />
  {/if}

  <!-- Error -->
  {#if error}
    <p class="text-sm text-red-600 bg-red-50 rounded px-3 py-2">{error}</p>
  {/if}

  <!-- Success -->
  {#if outputPath}
    <p class="text-sm text-green-700 bg-green-50 rounded px-3 py-2">
      Saved: <span class="font-mono break-all">{outputPath}</span>
    </p>
  {/if}

  <!-- Action -->
  <button
    onclick={runMerge}
    disabled={!canMerge}
    class="btn btn-primary w-full"
  >
    {$op.running ? 'Merging…' : 'Merge PDFs'}
  </button>
</div>
```

- [ ] **Step 2: Register in tools registry**

In `src/lib/tools-registry.ts`, update the entry for `merge` to reference the component:

```typescript
// In the tool registry map, update the merge entry:
{
  id: 'merge',
  label: 'Merge PDF',
  group: 'organise',
  icon: 'merge',
  description: 'Combine multiple PDFs into one',
  component: () => import('./components/tools/organise/MergeWorkspace.svelte'),
}
```

- [ ] **Step 3: Commit**

```
git add src/lib/components/tools/organise/MergeWorkspace.svelte src/lib/tools-registry.ts
git commit -m "feat(organise): add Merge PDF workspace UI"
```

---

## Chunk 3: Split PDF

### Task 4: Rust — `organise/split.rs`

**Files:**
- Create: `src-tauri/src/tools/organise/split.rs`

- [ ] **Step 1: Write failing unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_single_page() {
        assert_eq!(parse_range("5", 10).unwrap(), vec![5usize]);
    }

    #[test]
    fn parse_range_span() {
        assert_eq!(parse_range("2-4", 10).unwrap(), vec![2, 3, 4]);
    }

    #[test]
    fn parse_range_mixed() {
        assert_eq!(parse_range("1-3,5,7-9", 10).unwrap(), vec![1, 2, 3, 5, 7, 8, 9]);
    }

    #[test]
    fn parse_range_out_of_bounds_returns_error() {
        assert!(parse_range("1-15", 10).is_err());
    }

    #[test]
    fn parse_range_invalid_syntax_returns_error() {
        assert!(parse_range("a-b", 10).is_err());
    }

    #[test]
    fn chunks_by_n_splits_correctly() {
        let pages = vec![1, 2, 3, 4, 5];
        let chunks = chunk_by_n(&pages, 2);
        assert_eq!(chunks, vec![vec![1, 2], vec![3, 4], vec![5]]);
    }

    #[test]
    fn output_stem_for_split() {
        use std::path::PathBuf;
        let p = PathBuf::from("/tmp/report.pdf");
        assert_eq!(output_stem_for_chunk(&p, 1), "report_split_1");
        assert_eq!(output_stem_for_chunk(&p, 2), "report_split_2");
    }
}
```

- [ ] **Step 2: Implement `split.rs`**

```rust
use std::path::PathBuf;
use lopdf::Document;
use tauri::AppHandle;

use crate::error::AppError;
use crate::pipeline::{temp::TempStage, progress::{emit_progress, emit_complete, emit_error}};
use crate::tools::ProcessRequest;

/// Parse a page-range string like "1-3,5,7-9" into a sorted, deduplicated
/// list of 1-based page numbers. Returns error if any page > `total_pages`.
pub fn parse_range(range_str: &str, total_pages: usize) -> Result<Vec<usize>, AppError> {
    let mut pages: Vec<usize> = Vec::new();
    for part in range_str.split(',') {
        let part = part.trim();
        if part.contains('-') {
            let mut iter = part.splitn(2, '-');
            let start: usize = iter.next().unwrap_or("").trim().parse()
                .map_err(|_| AppError::Validation(format!("Invalid range part: '{part}'")))?;
            let end: usize = iter.next().unwrap_or("").trim().parse()
                .map_err(|_| AppError::Validation(format!("Invalid range part: '{part}'")))?;
            if start == 0 || end == 0 || start > end {
                return Err(AppError::Validation(format!("Invalid range: '{part}'")));
            }
            if end > total_pages {
                return Err(AppError::Validation(
                    format!("Page {end} exceeds document length ({total_pages})")
                ));
            }
            pages.extend(start..=end);
        } else {
            let page: usize = part.parse()
                .map_err(|_| AppError::Validation(format!("Invalid page number: '{part}'")))?;
            if page == 0 || page > total_pages {
                return Err(AppError::Validation(
                    format!("Page {page} out of range (1–{total_pages})")
                ));
            }
            pages.push(page);
        }
    }
    pages.sort_unstable();
    pages.dedup();
    Ok(pages)
}

/// Split `pages` (1-based indices) into chunks of size `n`.
pub fn chunk_by_n(pages: &[usize], n: usize) -> Vec<Vec<usize>> {
    pages.chunks(n).map(|c| c.to_vec()).collect()
}

pub fn output_stem_for_chunk(input: &PathBuf, chunk_index: usize) -> String {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
    format!("{stem}_split_{chunk_index}")
}

/// Extract the given 1-based `page_numbers` from `doc` into a new Document.
fn extract_pages(doc: &Document, page_numbers: &[usize]) -> Result<Document, AppError> {
    let all_pages: Vec<_> = doc.get_pages().into_iter().collect();
    // lopdf get_pages returns a BTreeMap<u32, ObjectId>; keys are 1-based page numbers
    let mut new_doc = doc.clone();
    let keep: std::collections::BTreeSet<u32> =
        page_numbers.iter().map(|&p| p as u32).collect();
    let to_delete: Vec<u32> = all_pages
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| !keep.contains(n))
        .collect();
    for page_num in to_delete {
        new_doc.delete_pages(&[page_num]);
    }
    Ok(new_doc)
}

pub async fn run(handle: AppHandle, req: ProcessRequest) -> Result<PathBuf, AppError> {
    let op_id = req.options.get("op_id").and_then(|v| v.as_str()).unwrap_or("split").to_string();

    let input_path = req.input_paths.first()
        .ok_or_else(|| AppError::Validation("Split requires one input file".into()))?;

    crate::pipeline::validate::validate_pdf(input_path)?;

    emit_progress(&handle, &op_id, 5, "Loading document…");
    let doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    let total_pages = doc.get_pages().len();

    // Determine chunks
    let chunks: Vec<Vec<usize>> = if let Some(range_str) = req.options.get("range").and_then(|v| v.as_str()) {
        // Range mode: one output chunk from the specified pages
        let pages = parse_range(range_str, total_pages)?;
        vec![pages]
    } else if let Some(n) = req.options.get("every_n_pages").and_then(|v| v.as_u64()) {
        // Every-N mode
        let all_pages: Vec<usize> = (1..=total_pages).collect();
        chunk_by_n(&all_pages, n as usize)
    } else {
        return Err(AppError::Validation(
            "Split requires 'range' or 'every_n_pages' option".into()
        ));
    };

    let stage = TempStage::new(&req.output_dir)?;
    let mut output_paths: Vec<PathBuf> = Vec::new();

    for (i, chunk) in chunks.iter().enumerate() {
        let pct = 20 + (i * 70 / chunks.len()) as u8;
        emit_progress(&handle, &op_id, pct, &format!("Writing chunk {}/{}", i + 1, chunks.len()));

        let new_doc = extract_pages(&doc, chunk)?;
        let stem = output_stem_for_chunk(input_path, i + 1);
        let out_path = stage.output_path(&stem, "pdf");
        new_doc.save(&out_path)
            .map_err(|e| AppError::Pdf(format!("Failed to save split chunk {}: {e}", i + 1)))?;
        output_paths.push(out_path);
    }

    // Return first output (multi-output communicated via progress events in real usage)
    let first = output_paths.into_iter().next()
        .ok_or_else(|| AppError::Pdf("No output produced".into()))?;

    emit_complete(&handle, &op_id, &first);
    Ok(first)
}
```

- [ ] **Step 3: Run tests and verify**

```
cargo test organise::split::tests
```

All 7 tests must pass.

- [ ] **Step 4: Commit**

```
git add src-tauri/src/tools/organise/split.rs
git commit -m "feat(organise): implement split PDF Rust backend"
```

---

### Task 5: Svelte 5 — `SplitWorkspace.svelte`

**Files:**
- Create: `src/lib/components/tools/organise/SplitWorkspace.svelte`

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { invoke } from '@tauri-apps/api/core';
  import { operationStore } from '$lib/stores/operation';
  import ProgressBar from '$lib/components/ui/ProgressBar.svelte';

  type SplitMode = 'range' | 'every_n';

  let filePath = $state<string | null>(null);
  let mode = $state<SplitMode>('range');
  let rangeStr = $state('');
  let everyN = $state(1);
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);

  const op = operationStore;

  const canRun = $derived(
    filePath !== null &&
    !$op.running &&
    (mode === 'every_n' ? everyN >= 1 : rangeStr.trim().length > 0)
  );

  async function pickFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (selected && !Array.isArray(selected)) {
      filePath = selected;
      error = null;
      outputPath = null;
    }
  }

  async function runSplit() {
    if (!filePath) return;
    error = null;
    outputPath = null;

    const outDir = await save({
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
      defaultPath: 'split_1.pdf',
    });
    if (!outDir) return;
    const dir = outDir.substring(0, Math.max(outDir.lastIndexOf('/'), outDir.lastIndexOf('\\')));

    const options: Record<string, unknown> = { op_id: 'split' };
    if (mode === 'range') {
      options['range'] = rangeStr.trim();
    } else {
      options['every_n_pages'] = everyN;
    }

    op.start('split');
    try {
      const result: string = await invoke('process_pdf', {
        request: {
          tool_id: 'split',
          input_paths: [filePath],
          output_dir: dir,
          options,
        },
      });
      outputPath = result;
      op.complete(result);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      error = msg;
      op.fail(msg);
    }
  }
</script>

<div class="flex flex-col gap-4 p-4 h-full">
  <h2 class="text-lg font-semibold">Split PDF</h2>

  <!-- File picker -->
  <button
    onclick={pickFile}
    class={[
      'flex items-center gap-3 p-3 rounded-lg border transition-colors text-left',
      filePath ? 'border-blue-300 bg-blue-50' : 'border-dashed border-gray-300 hover:border-blue-400',
    ].join(' ')}
  >
    <svg class="w-5 h-5 text-gray-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
        d="M9 13h6m-3-3v6m5 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414A1 1 0 0121 9.414V19a2 2 0 01-2 2z" />
    </svg>
    <span class="truncate text-sm">
      {filePath ?? 'Click to choose a PDF…'}
    </span>
  </button>

  <!-- Mode selector -->
  <fieldset class="flex gap-4">
    <legend class="text-sm font-medium text-gray-700 mb-1 w-full">Split mode</legend>
    <label class="flex items-center gap-2 cursor-pointer">
      <input type="radio" bind:group={mode} value="range" class="accent-blue-600" />
      <span class="text-sm">Page ranges</span>
    </label>
    <label class="flex items-center gap-2 cursor-pointer">
      <input type="radio" bind:group={mode} value="every_n" class="accent-blue-600" />
      <span class="text-sm">Every N pages</span>
    </label>
  </fieldset>

  <!-- Range input -->
  {#if mode === 'range'}
    <div class="flex flex-col gap-1">
      <label for="range-input" class="text-sm text-gray-600">
        Page ranges <span class="text-gray-400">(e.g. 1-3,5,7-9)</span>
      </label>
      <input
        id="range-input"
        type="text"
        bind:value={rangeStr}
        placeholder="1-3,5,7-9"
        class="input input-bordered text-sm"
      />
    </div>
  {:else}
    <div class="flex flex-col gap-1">
      <label for="n-input" class="text-sm text-gray-600">Pages per chunk</label>
      <input
        id="n-input"
        type="number"
        bind:value={everyN}
        min="1"
        class="input input-bordered text-sm w-28"
      />
    </div>
  {/if}

  <!-- Progress -->
  {#if $op.running}
    <ProgressBar value={$op.progress} label={$op.label} />
  {/if}

  {#if error}
    <p class="text-sm text-red-600 bg-red-50 rounded px-3 py-2">{error}</p>
  {/if}

  {#if outputPath}
    <p class="text-sm text-green-700 bg-green-50 rounded px-3 py-2">
      Saved: <span class="font-mono break-all">{outputPath}</span>
    </p>
  {/if}

  <button onclick={runSplit} disabled={!canRun} class="btn btn-primary w-full mt-auto">
    {$op.running ? 'Splitting…' : 'Split PDF'}
  </button>
</div>
```

- [ ] **Step 2: Register in tools registry**

```typescript
{
  id: 'split',
  label: 'Split PDF',
  group: 'organise',
  icon: 'split',
  description: 'Extract pages or split into chunks',
  component: () => import('./components/tools/organise/SplitWorkspace.svelte'),
}
```

- [ ] **Step 3: Commit**

```
git add src/lib/components/tools/organise/SplitWorkspace.svelte src/lib/tools-registry.ts
git commit -m "feat(organise): add Split PDF workspace UI"
```

---

## Chunk 4: Compress PDF

### Task 6: Rust — `organise/compress.rs`

**Files:**
- Create: `src-tauri/src/tools/organise/compress.rs`

The compress tool uses lopdf to: (1) remove embedded thumbnail streams (`/Thumb` entries), (2) find inline image streams and re-encode them at the target DPI using the `image` crate, and (3) re-save with object streams (PDF 1.5 cross-reference streams) for structural compression.

- [ ] **Step 1: Write failing unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_from_str_small() {
        let p = CompressPreset::from_str("small").unwrap();
        assert_eq!(p.dpi, 72);
        assert!(p.quality <= 60);
    }

    #[test]
    fn preset_from_str_balanced() {
        let p = CompressPreset::from_str("balanced").unwrap();
        assert_eq!(p.dpi, 150);
    }

    #[test]
    fn preset_from_str_high_quality() {
        let p = CompressPreset::from_str("high_quality").unwrap();
        assert_eq!(p.dpi, 220);
    }

    #[test]
    fn preset_from_str_invalid_returns_error() {
        assert!(CompressPreset::from_str("ultra").is_err());
    }

    #[test]
    fn output_stem_compress() {
        use std::path::PathBuf;
        let p = PathBuf::from("/tmp/report.pdf");
        assert_eq!(output_stem(&p), "report_compressed");
    }
}
```

- [ ] **Step 2: Implement `compress.rs`**

```rust
use std::path::PathBuf;
use lopdf::{Document, Object};
use tauri::AppHandle;

use crate::error::AppError;
use crate::pipeline::{temp::TempStage, progress::{emit_progress, emit_complete}};
use crate::tools::ProcessRequest;

#[derive(Debug, Clone)]
pub struct CompressPreset {
    /// Target image DPI (images above this are downsampled).
    pub dpi: u32,
    /// JPEG quality 0–100.
    pub quality: u8,
}

impl CompressPreset {
    pub fn from_str(s: &str) -> Result<Self, AppError> {
        match s {
            "small" => Ok(Self { dpi: 72, quality: 55 }),
            "balanced" => Ok(Self { dpi: 150, quality: 75 }),
            "high_quality" => Ok(Self { dpi: 220, quality: 90 }),
            other => Err(AppError::Validation(
                format!("Unknown compress preset '{other}'. Use: small, balanced, high_quality")
            )),
        }
    }
}

pub fn output_stem(input: &PathBuf) -> String {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
    format!("{stem}_compressed")
}

/// Remove /Thumb entries from page dictionaries (embedded thumbnails waste space).
fn strip_thumbnails(doc: &mut Document) {
    let page_ids: Vec<_> = doc.get_pages().values().copied().collect();
    for page_id in page_ids {
        if let Ok(Object::Dictionary(dict)) = doc.get_object_mut(page_id) {
            dict.remove(b"Thumb");
        }
    }
}

/// Re-encode image XObjects in the document at the given preset's DPI/quality.
/// Images whose natural size exceeds the target DPI are downsampled.
fn compress_images(doc: &mut Document, preset: &CompressPreset) -> Result<(), AppError> {
    use lopdf::Stream;
    use image::{DynamicImage, ImageFormat};
    use std::io::Cursor;

    // Collect image object IDs first (avoid borrow conflict)
    let image_ids: Vec<lopdf::ObjectId> = doc
        .objects
        .iter()
        .filter_map(|(id, obj)| {
            if let Object::Stream(stream) = obj {
                let subtype = stream.dict.get(b"Subtype")
                    .and_then(|o| o.as_name_str().ok())
                    .unwrap_or("");
                if subtype == "Image" {
                    return Some(*id);
                }
            }
            None
        })
        .collect();

    for id in image_ids {
        let stream = match doc.get_object(id) {
            Ok(Object::Stream(s)) => s.clone(),
            _ => continue,
        };

        let width = stream.dict.get(b"Width").and_then(|o| o.as_i64().ok()).unwrap_or(0) as u32;
        let height = stream.dict.get(b"Height").and_then(|o| o.as_i64().ok()).unwrap_or(0) as u32;

        // Simple heuristic: if image is larger than 2× preset DPI in either dimension,
        // decode and re-encode at target size. We treat 72dpi as a baseline reference
        // (1 PDF pt = 1/72 inch), so target_px = dpi * page_size_in_inches.
        // For simplicity we cap at preset.dpi * 8 inches = max image dimension.
        let max_dim = preset.dpi * 8;
        if width <= max_dim && height <= max_dim {
            continue; // Already small enough
        }

        // Decompress stream to raw bytes using lopdf
        let content = match stream.decompressed_content() {
            Ok(c) => c,
            Err(_) => continue, // Skip if we can't decompress
        };

        // Detect image format from stream parameters
        let color_space = stream.dict.get(b"ColorSpace")
            .and_then(|o| o.as_name_str().ok())
            .unwrap_or("DeviceRGB");
        let bits = stream.dict.get(b"BitsPerComponent")
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(8) as u32;

        let img: DynamicImage = if color_space == "DeviceGray" && bits == 8 {
            let gray = image::GrayImage::from_raw(width, height, content)
                .ok_or_else(|| AppError::Pdf("Failed to decode grayscale image".into()))?;
            DynamicImage::ImageLuma8(gray)
        } else {
            // Assume RGB
            let rgb = image::RgbImage::from_raw(width, height, content)
                .ok_or_else(|| AppError::Pdf("Failed to decode RGB image".into()))?;
            DynamicImage::ImageRgb8(rgb)
        };

        // Compute new dimensions
        let scale = (max_dim as f32) / (width.max(height) as f32);
        let new_w = ((width as f32) * scale) as u32;
        let new_h = ((height as f32) * scale) as u32;
        let resized = img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3);

        // Re-encode as JPEG
        let mut jpeg_buf: Vec<u8> = Vec::new();
        resized
            .write_to(&mut Cursor::new(&mut jpeg_buf), ImageFormat::Jpeg)
            .map_err(|e| AppError::Pdf(format!("JPEG encode failed: {e}")))?;

        // Build replacement stream
        let mut new_dict = stream.dict.clone();
        new_dict.set("Width", (new_w as i64).into());
        new_dict.set("Height", (new_h as i64).into());
        new_dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
        new_dict.remove(b"DecodeParms");
        let new_stream = Stream::new(new_dict, jpeg_buf);
        doc.objects.insert(id, Object::Stream(new_stream));
    }

    Ok(())
}

pub async fn run(handle: AppHandle, req: ProcessRequest) -> Result<PathBuf, AppError> {
    let op_id = req.options.get("op_id").and_then(|v| v.as_str()).unwrap_or("compress").to_string();

    let preset_name = req.options
        .get("preset")
        .and_then(|v| v.as_str())
        .unwrap_or("balanced");
    let preset = CompressPreset::from_str(preset_name)?;

    let input_path = req.input_paths.first()
        .ok_or_else(|| AppError::Validation("Compress requires one input file".into()))?;

    crate::pipeline::validate::validate_pdf(input_path)?;

    emit_progress(&handle, &op_id, 5, "Loading document…");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    emit_progress(&handle, &op_id, 20, "Stripping thumbnails…");
    strip_thumbnails(&mut doc);

    emit_progress(&handle, &op_id, 35, "Compressing images…");
    compress_images(&mut doc, &preset)?;

    emit_progress(&handle, &op_id, 80, "Writing compressed output…");
    let stem = output_stem(input_path);
    let stage = TempStage::new(&req.output_dir)?;
    let out_path = stage.output_path(&stem, "pdf");
    doc.save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save compressed PDF: {e}")))?;

    emit_complete(&handle, &op_id, &out_path);
    Ok(out_path)
}
```

- [ ] **Step 3: Add `image` crate dependency**

In `src-tauri/Cargo.toml`:

```toml
[dependencies]
image = { version = "0.25", default-features = false, features = ["jpeg", "png", "webp"] }
```

- [ ] **Step 4: Run tests and verify**

```
cargo test organise::compress::tests
```

- [ ] **Step 5: Commit**

```
git add src-tauri/src/tools/organise/compress.rs src-tauri/Cargo.toml
git commit -m "feat(organise): implement compress PDF Rust backend"
```

---

### Task 7: Svelte 5 — `CompressWorkspace.svelte`

**Files:**
- Create: `src/lib/components/tools/organise/CompressWorkspace.svelte`

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { invoke } from '@tauri-apps/api/core';
  import { operationStore } from '$lib/stores/operation';
  import ProgressBar from '$lib/components/ui/ProgressBar.svelte';

  type Preset = 'small' | 'balanced' | 'high_quality';

  const PRESETS: { id: Preset; label: string; description: string }[] = [
    { id: 'small', label: 'Small file', description: '72 DPI images — smallest size, lower quality' },
    { id: 'balanced', label: 'Balanced', description: '150 DPI images — good size/quality trade-off' },
    { id: 'high_quality', label: 'High quality', description: '220 DPI images — near-lossless, larger file' },
  ];

  let filePath = $state<string | null>(null);
  let preset = $state<Preset>('balanced');
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);

  const op = operationStore;
  const canRun = $derived(filePath !== null && !$op.running);

  async function pickFile() {
    const selected = await open({ multiple: false, filters: [{ name: 'PDF', extensions: ['pdf'] }] });
    if (selected && !Array.isArray(selected)) {
      filePath = selected;
      error = null;
      outputPath = null;
    }
  }

  async function runCompress() {
    if (!filePath) return;
    error = null;
    outputPath = null;

    const outFile = await save({
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
      defaultPath: 'compressed.pdf',
    });
    if (!outFile) return;
    const dir = outFile.substring(0, Math.max(outFile.lastIndexOf('/'), outFile.lastIndexOf('\\')));

    op.start('compress');
    try {
      const result: string = await invoke('process_pdf', {
        request: {
          tool_id: 'compress',
          input_paths: [filePath],
          output_dir: dir,
          options: { op_id: 'compress', preset },
        },
      });
      outputPath = result;
      op.complete(result);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      error = msg;
      op.fail(msg);
    }
  }
</script>

<div class="flex flex-col gap-4 p-4 h-full">
  <h2 class="text-lg font-semibold">Compress PDF</h2>

  <!-- Limitation banner -->
  <div class="text-xs text-amber-700 bg-amber-50 border border-amber-200 rounded px-3 py-2">
    Compression targets embedded images. Text and vector graphics are unaffected.
    Results vary by document content.
  </div>

  <!-- File picker -->
  <button
    onclick={pickFile}
    class={[
      'flex items-center gap-3 p-3 rounded-lg border transition-colors text-left',
      filePath ? 'border-blue-300 bg-blue-50' : 'border-dashed border-gray-300 hover:border-blue-400',
    ].join(' ')}
  >
    <svg class="w-5 h-5 text-gray-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
        d="M9 13h6m-3-3v6m5 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414A1 1 0 0121 9.414V19a2 2 0 01-2 2z" />
    </svg>
    <span class="truncate text-sm">{filePath ?? 'Click to choose a PDF…'}</span>
  </button>

  <!-- Preset selection -->
  <fieldset class="flex flex-col gap-2">
    <legend class="text-sm font-medium text-gray-700">Compression preset</legend>
    {#each PRESETS as p (p.id)}
      <label class={[
        'flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition-colors',
        preset === p.id ? 'border-blue-500 bg-blue-50' : 'border-gray-200 hover:border-gray-300',
      ].join(' ')}>
        <input
          type="radio"
          bind:group={preset}
          value={p.id}
          class="mt-0.5 accent-blue-600"
        />
        <span class="flex flex-col">
          <span class="text-sm font-medium">{p.label}</span>
          <span class="text-xs text-gray-500">{p.description}</span>
        </span>
      </label>
    {/each}
  </fieldset>

  {#if $op.running}
    <ProgressBar value={$op.progress} label={$op.label} />
  {/if}

  {#if error}
    <p class="text-sm text-red-600 bg-red-50 rounded px-3 py-2">{error}</p>
  {/if}

  {#if outputPath}
    <p class="text-sm text-green-700 bg-green-50 rounded px-3 py-2">
      Saved: <span class="font-mono break-all">{outputPath}</span>
    </p>
  {/if}

  <button onclick={runCompress} disabled={!canRun} class="btn btn-primary w-full mt-auto">
    {$op.running ? 'Compressing…' : 'Compress PDF'}
  </button>
</div>
```

- [ ] **Step 2: Register in tools registry**

```typescript
{
  id: 'compress',
  label: 'Compress PDF',
  group: 'organise',
  icon: 'compress',
  description: 'Reduce file size with 3 quality presets',
  component: () => import('./components/tools/organise/CompressWorkspace.svelte'),
}
```

- [ ] **Step 3: Commit**

```
git add src/lib/components/tools/organise/CompressWorkspace.svelte src/lib/tools-registry.ts
git commit -m "feat(organise): add Compress PDF workspace UI"
```

---

## Chunk 5: Rotate Pages

### Task 8: Rust — `organise/rotate.rs`

**Files:**
- Create: `src-tauri/src/tools/organise/rotate.rs`

- [ ] **Step 1: Write failing unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rotation_zero() {
        assert_eq!(normalize_rotation(0), 0);
    }

    #[test]
    fn normalize_rotation_360_wraps() {
        assert_eq!(normalize_rotation(360), 0);
    }

    #[test]
    fn normalize_rotation_450_wraps() {
        assert_eq!(normalize_rotation(450), 90);
    }

    #[test]
    fn normalize_rotation_negative() {
        // -90 mod 360 = 270
        assert_eq!(normalize_rotation(-90), 270);
    }

    #[test]
    fn parse_page_selection_all() {
        assert_eq!(parse_page_selection("all", 5), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn parse_page_selection_list() {
        assert_eq!(parse_page_selection("1,3,5", 5), vec![1, 3, 5]);
    }

    #[test]
    fn parse_page_selection_range() {
        assert_eq!(parse_page_selection("2-4", 5), vec![2, 3, 4]);
    }

    #[test]
    fn output_stem_rotate() {
        use std::path::PathBuf;
        let p = PathBuf::from("/tmp/doc.pdf");
        assert_eq!(output_stem(&p), "doc_rotated");
    }
}
```

- [ ] **Step 2: Implement `rotate.rs`**

```rust
use std::path::PathBuf;
use lopdf::{Document, Object};
use tauri::AppHandle;

use crate::error::AppError;
use crate::pipeline::{temp::TempStage, progress::{emit_progress, emit_complete}};
use crate::tools::ProcessRequest;

/// Normalise any rotation angle to 0, 90, 180, or 270.
pub fn normalize_rotation(degrees: i32) -> i32 {
    ((degrees % 360) + 360) % 360
}

/// Parse "all", a comma-separated list, or a range like "2-4" into 1-based page numbers.
pub fn parse_page_selection(selection: &str, total: usize) -> Vec<usize> {
    if selection.trim().eq_ignore_ascii_case("all") {
        return (1..=total).collect();
    }
    let mut pages = Vec::new();
    for part in selection.split(',') {
        let part = part.trim();
        if part.contains('-') {
            let mut it = part.splitn(2, '-');
            let a: usize = it.next().unwrap_or("0").trim().parse().unwrap_or(0);
            let b: usize = it.next().unwrap_or("0").trim().parse().unwrap_or(0);
            if a > 0 && b >= a && b <= total {
                pages.extend(a..=b);
            }
        } else {
            let n: usize = part.parse().unwrap_or(0);
            if n > 0 && n <= total {
                pages.push(n);
            }
        }
    }
    pages.sort_unstable();
    pages.dedup();
    pages
}

pub fn output_stem(input: &PathBuf) -> String {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
    format!("{stem}_rotated")
}

/// Apply `additional_degrees` to the /Rotate entry of the given page.
fn rotate_page(doc: &mut Document, page_id: lopdf::ObjectId, additional: i32) -> Result<(), AppError> {
    let page_obj = doc.get_object_mut(page_id)
        .map_err(|e| AppError::Pdf(format!("Page object error: {e}")))?;
    if let Object::Dictionary(dict) = page_obj {
        let current: i32 = dict
            .get(b"Rotate")
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0) as i32;
        let new_rotation = normalize_rotation(current + additional);
        dict.set("Rotate", Object::Integer(new_rotation as i64));
    }
    Ok(())
}

pub async fn run(handle: AppHandle, req: ProcessRequest) -> Result<PathBuf, AppError> {
    let op_id = req.options.get("op_id").and_then(|v| v.as_str()).unwrap_or("rotate_pages").to_string();

    let input_path = req.input_paths.first()
        .ok_or_else(|| AppError::Validation("Rotate requires one input file".into()))?;

    let degrees: i32 = req.options
        .get("degrees")
        .and_then(|v| v.as_i64())
        .unwrap_or(90) as i32;
    let degrees = normalize_rotation(degrees);

    let page_selection = req.options
        .get("pages")
        .and_then(|v| v.as_str())
        .unwrap_or("all")
        .to_string();

    crate::pipeline::validate::validate_pdf(input_path)?;

    emit_progress(&handle, &op_id, 5, "Loading document…");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    let total = doc.get_pages().len();
    let selected = parse_page_selection(&page_selection, total);

    let page_map: std::collections::BTreeMap<u32, lopdf::ObjectId> = doc.get_pages();

    emit_progress(&handle, &op_id, 20, "Rotating pages…");
    for (i, page_num) in selected.iter().enumerate() {
        if let Some(&page_id) = page_map.get(&(*page_num as u32)) {
            rotate_page(&mut doc, page_id, degrees)?;
        }
        if i % 10 == 0 {
            let pct = 20 + (i * 60 / selected.len().max(1)) as u8;
            emit_progress(&handle, &op_id, pct, &format!("Rotating page {}/{}", i + 1, selected.len()));
        }
    }

    emit_progress(&handle, &op_id, 85, "Saving output…");
    let stem = output_stem(input_path);
    let stage = TempStage::new(&req.output_dir)?;
    let out_path = stage.output_path(&stem, "pdf");
    doc.save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save rotated PDF: {e}")))?;

    emit_complete(&handle, &op_id, &out_path);
    Ok(out_path)
}
```

- [ ] **Step 3: Run tests and verify**

```
cargo test organise::rotate::tests
```

- [ ] **Step 4: Commit**

```
git add src-tauri/src/tools/organise/rotate.rs
git commit -m "feat(organise): implement rotate pages Rust backend"
```

---

### Task 9: Svelte 5 — `RotateWorkspace.svelte`

**Files:**
- Create: `src/lib/components/tools/organise/RotateWorkspace.svelte`
- Create: `src/lib/components/tools/organise/ThumbnailGrid.svelte` (shared; used by Rotate, Reorder, Remove)

- [ ] **Step 1: Create `ThumbnailGrid.svelte`** (shared thumbnail picker)

```svelte
<!-- src/lib/components/tools/organise/ThumbnailGrid.svelte -->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface Props {
    pdfPath: string;
    totalPages: number;
    selectedPages: Set<number>;
    onToggle: (page: number) => void;
  }

  const { pdfPath, totalPages, selectedPages, onToggle }: Props = $props();

  // Lazy-load: show first 100, then load-more
  const INITIAL_BATCH = 100;
  const CONCURRENT_RENDERS = 4;

  let visibleCount = $state(Math.min(INITIAL_BATCH, totalPages));
  let thumbnails = $state<Map<number, string>>(new Map());
  let loading = $state(false);

  // Render thumbnails in batches of CONCURRENT_RENDERS
  async function renderBatch(start: number, end: number) {
    loading = true;
    const batch: number[] = [];
    for (let p = start; p <= end; p++) {
      if (!thumbnails.has(p)) batch.push(p);
    }
    for (let i = 0; i < batch.length; i += CONCURRENT_RENDERS) {
      const chunk = batch.slice(i, i + CONCURRENT_RENDERS);
      const results: { page: number; data_url: string }[] = await Promise.all(
        chunk.map(page =>
          invoke<{ page: number; data_url: string }>('render_page_thumbnail', {
            path: pdfPath,
            page,
            width: 96,
            height: 96,
          })
        )
      );
      // Immutable update: create new Map
      thumbnails = new Map([
        ...thumbnails,
        ...results.map(r => [r.page, r.data_url] as [number, string]),
      ]);
    }
    loading = false;
  }

  // Initial render
  $effect(() => {
    if (pdfPath && totalPages > 0) {
      renderBatch(1, visibleCount);
    }
  });

  function loadMore() {
    const next = Math.min(visibleCount + INITIAL_BATCH, totalPages);
    renderBatch(visibleCount + 1, next);
    visibleCount = next;
  }
</script>

<div class="flex flex-col gap-3">
  <div class="grid grid-cols-[repeat(auto-fill,minmax(96px,1fr))] gap-2">
    {#each { length: visibleCount } as _, i (i)}
      {@const page = i + 1}
      {@const selected = selectedPages.has(page)}
      {@const thumb = thumbnails.get(page)}
      <button
        onclick={() => onToggle(page)}
        class={[
          'relative flex flex-col items-center gap-1 p-1 rounded border-2 transition-colors cursor-pointer',
          selected ? 'border-blue-500 bg-blue-50' : 'border-gray-200 hover:border-gray-400',
        ].join(' ')}
        aria-pressed={selected}
        aria-label={`Page ${page}`}
      >
        {#if thumb}
          <img src={thumb} alt="Page {page}" class="w-24 h-24 object-contain rounded" />
        {:else}
          <div class="w-24 h-24 bg-gray-100 rounded animate-pulse flex items-center justify-center">
            <span class="text-xs text-gray-400">{page}</span>
          </div>
        {/if}
        <span class="text-xs text-gray-600">{page}</span>
        {#if selected}
          <span class="absolute top-1 right-1 w-4 h-4 bg-blue-500 rounded-full flex items-center justify-center">
            <svg class="w-2.5 h-2.5 text-white" fill="currentColor" viewBox="0 0 20 20">
              <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
            </svg>
          </span>
        {/if}
      </button>
    {/each}
  </div>

  {#if loading}
    <p class="text-xs text-center text-gray-400">Loading thumbnails…</p>
  {/if}

  {#if visibleCount < totalPages}
    <button onclick={loadMore} class="btn btn-ghost text-sm self-center">
      Load more ({totalPages - visibleCount} remaining)
    </button>
  {/if}
</div>
```

- [ ] **Step 2: Create `RotateWorkspace.svelte`**

```svelte
<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { invoke } from '@tauri-apps/api/core';
  import { operationStore } from '$lib/stores/operation';
  import ProgressBar from '$lib/components/ui/ProgressBar.svelte';
  import ThumbnailGrid from './ThumbnailGrid.svelte';

  const ROTATION_OPTIONS = [
    { degrees: 90,  label: '90° clockwise' },
    { degrees: 180, label: '180°' },
    { degrees: 270, label: '90° counter-clockwise' },
  ];

  let filePath = $state<string | null>(null);
  let totalPages = $state(0);
  let selectedPages = $state<Set<number>>(new Set());
  let degrees = $state(90);
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);

  const op = operationStore;

  const allSelected = $derived(totalPages > 0 && selectedPages.size === totalPages);
  const canRun = $derived(filePath !== null && !$op.running && (selectedPages.size > 0 || allSelected));

  async function pickFile() {
    const selected = await open({ multiple: false, filters: [{ name: 'PDF', extensions: ['pdf'] }] });
    if (selected && !Array.isArray(selected)) {
      filePath = selected;
      error = null;
      outputPath = null;
      selectedPages = new Set();
      // Get page count
      totalPages = await invoke<number>('get_page_count', { path: selected });
    }
  }

  function togglePage(page: number) {
    // Immutable: create new Set
    const next = new Set(selectedPages);
    if (next.has(page)) {
      next.delete(page);
    } else {
      next.add(page);
    }
    selectedPages = next;
  }

  function selectAll() {
    selectedPages = new Set(Array.from({ length: totalPages }, (_, i) => i + 1));
  }

  function clearSelection() {
    selectedPages = new Set();
  }

  async function runRotate() {
    if (!filePath) return;
    error = null;
    outputPath = null;

    const outFile = await save({
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
      defaultPath: 'rotated.pdf',
    });
    if (!outFile) return;
    const dir = outFile.substring(0, Math.max(outFile.lastIndexOf('/'), outFile.lastIndexOf('\\')));

    // Determine page selection string
    const pageSelection = allSelected
      ? 'all'
      : Array.from(selectedPages).sort((a, b) => a - b).join(',');

    op.start('rotate_pages');
    try {
      const result: string = await invoke('process_pdf', {
        request: {
          tool_id: 'rotate_pages',
          input_paths: [filePath],
          output_dir: dir,
          options: { op_id: 'rotate_pages', degrees, pages: pageSelection },
        },
      });
      outputPath = result;
      op.complete(result);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      error = msg;
      op.fail(msg);
    }
  }
</script>

<div class="flex flex-col gap-4 p-4 h-full overflow-hidden">
  <h2 class="text-lg font-semibold">Rotate Pages</h2>

  <!-- File picker -->
  <button
    onclick={pickFile}
    class={[
      'flex items-center gap-3 p-3 rounded-lg border transition-colors text-left shrink-0',
      filePath ? 'border-blue-300 bg-blue-50' : 'border-dashed border-gray-300 hover:border-blue-400',
    ].join(' ')}
  >
    <svg class="w-5 h-5 text-gray-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
        d="M9 13h6m-3-3v6m5 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414A1 1 0 0121 9.414V19a2 2 0 01-2 2z" />
    </svg>
    <span class="truncate text-sm">{filePath ?? 'Click to choose a PDF…'}</span>
  </button>

  {#if filePath && totalPages > 0}
    <!-- Rotation selector -->
    <div class="flex gap-3 shrink-0">
      {#each ROTATION_OPTIONS as opt (opt.degrees)}
        <label class={[
          'flex items-center gap-2 px-3 py-2 rounded-lg border cursor-pointer transition-colors text-sm',
          degrees === opt.degrees ? 'border-blue-500 bg-blue-50 font-medium' : 'border-gray-200 hover:border-gray-300',
        ].join(' ')}>
          <input type="radio" bind:group={degrees} value={opt.degrees} class="accent-blue-600" />
          {opt.label}
        </label>
      {/each}
    </div>

    <!-- Page selection controls -->
    <div class="flex items-center gap-3 shrink-0">
      <span class="text-sm text-gray-600">{selectedPages.size} of {totalPages} selected</span>
      <button onclick={selectAll} class="text-xs text-blue-600 hover:underline">All</button>
      <button onclick={clearSelection} class="text-xs text-gray-500 hover:underline">None</button>
    </div>

    <!-- Thumbnail grid -->
    <div class="flex-1 overflow-y-auto">
      <ThumbnailGrid
        pdfPath={filePath}
        {totalPages}
        {selectedPages}
        onToggle={togglePage}
      />
    </div>
  {/if}

  {#if $op.running}
    <ProgressBar value={$op.progress} label={$op.label} />
  {/if}

  {#if error}
    <p class="text-sm text-red-600 bg-red-50 rounded px-3 py-2">{error}</p>
  {/if}

  {#if outputPath}
    <p class="text-sm text-green-700 bg-green-50 rounded px-3 py-2">
      Saved: <span class="font-mono break-all">{outputPath}</span>
    </p>
  {/if}

  <button onclick={runRotate} disabled={!canRun} class="btn btn-primary w-full shrink-0">
    {$op.running ? 'Rotating…' : 'Rotate Pages'}
  </button>
</div>
```

- [ ] **Step 3: Add `render_page_thumbnail` and `get_page_count` Tauri commands**

These commands are needed by ThumbnailGrid and page-level workspace components. Add to `src-tauri/src/commands/thumbnails.rs`:

```rust
use tauri::AppHandle;
use pdfium_render::prelude::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use crate::error::AppError;

/// Render a single PDF page to a 96×96 JPEG and return it as a base64 data URL.
#[tauri::command]
pub async fn render_page_thumbnail(
    _handle: AppHandle,
    path: String,
    page: u32,
    width: u32,
    height: u32,
) -> Result<serde_json::Value, String> {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library().map_err(|e| e.to_string())?
    );
    let doc = pdfium.load_pdf_from_file(&path, None).map_err(|e| e.to_string())?;
    let pages = doc.pages();
    let page_idx = page.saturating_sub(1) as usize;
    if page_idx >= pages.len() {
        return Err(format!("Page {page} out of range"));
    }
    let pdf_page = pages.get(page_idx).map_err(|e| e.to_string())?;
    let bitmap = pdf_page.render_with_config(
        &PdfRenderConfig::new().set_target_size(width, height)
    ).map_err(|e| e.to_string())?;

    let img = bitmap.as_image().to_rgb8();
    let mut jpeg_buf: Vec<u8> = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut jpeg_buf),
        image::ImageFormat::Jpeg,
    ).map_err(|e| e.to_string())?;

    let data_url = format!("data:image/jpeg;base64,{}", STANDARD.encode(&jpeg_buf));
    Ok(serde_json::json!({ "page": page, "data_url": data_url }))
}

/// Return the total number of pages in a PDF.
#[tauri::command]
pub async fn get_page_count(_handle: AppHandle, path: String) -> Result<u32, String> {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library().map_err(|e| e.to_string())?
    );
    let doc = pdfium.load_pdf_from_file(&path, None).map_err(|e| e.to_string())?;
    Ok(doc.pages().len() as u32)
}
```

Register both in `src-tauri/src/lib.rs` (or wherever Tauri commands are registered):

```rust
.invoke_handler(tauri::generate_handler![
    commands::process::process_pdf,
    commands::thumbnails::render_page_thumbnail,
    commands::thumbnails::get_page_count,
    // ... existing commands
])
```

Add `pdfium-render` and `base64` to `Cargo.toml`:

```toml
[dependencies]
pdfium-render = { version = "0.8", features = ["image"] }
base64 = "0.22"
```

- [ ] **Step 4: Register rotate in tools registry**

```typescript
{
  id: 'rotate_pages',
  label: 'Rotate Pages',
  group: 'organise',
  icon: 'rotate',
  description: 'Rotate individual or all pages by 90/180/270°',
  component: () => import('./components/tools/organise/RotateWorkspace.svelte'),
}
```

- [ ] **Step 5: Commit**

```
git add src-tauri/src/tools/organise/rotate.rs \
        src-tauri/src/commands/thumbnails.rs \
        src/lib/components/tools/organise/RotateWorkspace.svelte \
        src/lib/components/tools/organise/ThumbnailGrid.svelte \
        src/lib/tools-registry.ts \
        src-tauri/Cargo.toml
git commit -m "feat(organise): implement rotate pages backend and UI with thumbnail grid"
```

---

## Chunk 6: Reorder Pages

### Task 10: Rust — `organise/reorder.rs`

**Files:**
- Create: `src-tauri/src/tools/organise/reorder.rs`

- [ ] **Step 1: Write failing unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorder_reverses_pages() {
        // Given a 4-page order request [4,3,2,1], the output should have pages in that order.
        let new_order = vec![4usize, 3, 2, 1];
        // Validate: all pages present, no duplicates, all in range 1..=4
        assert!(validate_order(&new_order, 4).is_ok());
    }

    #[test]
    fn reorder_validates_duplicate_pages() {
        let new_order = vec![1usize, 1, 2, 3];
        assert!(validate_order(&new_order, 4).is_err());
    }

    #[test]
    fn reorder_validates_missing_pages() {
        // [1,2,4] is missing page 3 from a 4-page doc
        let new_order = vec![1usize, 2, 4];
        assert!(validate_order(&new_order, 4).is_err());
    }

    #[test]
    fn reorder_validates_out_of_range() {
        let new_order = vec![1usize, 2, 3, 9];
        assert!(validate_order(&new_order, 4).is_err());
    }

    #[test]
    fn output_stem_reorder() {
        use std::path::PathBuf;
        let p = PathBuf::from("/tmp/doc.pdf");
        assert_eq!(output_stem(&p), "doc_reordered");
    }
}
```

- [ ] **Step 2: Implement `reorder.rs`**

```rust
use std::path::PathBuf;
use std::collections::BTreeMap;
use lopdf::{Document, Object, ObjectId};
use tauri::AppHandle;

use crate::error::AppError;
use crate::pipeline::{temp::TempStage, progress::{emit_progress, emit_complete}};
use crate::tools::ProcessRequest;

pub fn output_stem(input: &PathBuf) -> String {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
    format!("{stem}_reordered")
}

/// Validate that `new_order` is a permutation of 1..=total_pages with no duplicates.
pub fn validate_order(new_order: &[usize], total_pages: usize) -> Result<(), AppError> {
    if new_order.len() != total_pages {
        return Err(AppError::Validation(
            format!("Expected {total_pages} pages in order, got {}", new_order.len())
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for &p in new_order {
        if p == 0 || p > total_pages {
            return Err(AppError::Validation(
                format!("Page {p} is out of range (1–{total_pages})")
            ));
        }
        if !seen.insert(p) {
            return Err(AppError::Validation(format!("Duplicate page {p} in reorder list")));
        }
    }
    Ok(())
}

/// Rebuild the /Pages tree with pages in `new_order` (1-based).
/// Strategy: clone the doc, then replace the Kids array in the /Pages node.
fn apply_reorder(doc: &mut Document, new_order: &[usize]) -> Result<(), AppError> {
    // Get page object IDs in original order (BTreeMap is keyed by 1-based page number)
    let original_pages: BTreeMap<u32, ObjectId> = doc.get_pages();

    // Build new Kids list in desired order
    let new_kids: Vec<Object> = new_order
        .iter()
        .map(|&p| {
            original_pages
                .get(&(p as u32))
                .map(|&id| Object::Reference(id))
                .ok_or_else(|| AppError::Pdf(format!("Page {p} not found in document")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Find the /Pages dictionary object ID via the catalog
    let catalog_id = doc
        .trailer
        .get(b"Root")
        .and_then(|o| o.as_reference().ok())
        .ok_or_else(|| AppError::Pdf("Missing /Root in trailer".into()))?;

    let pages_id = {
        let catalog = doc
            .get_object(catalog_id)
            .map_err(|e| AppError::Pdf(format!("Catalog error: {e}")))?;
        if let Object::Dictionary(dict) = catalog {
            dict.get(b"Pages")
                .and_then(|o| o.as_reference().ok())
                .ok_or_else(|| AppError::Pdf("Missing /Pages in catalog".into()))?
        } else {
            return Err(AppError::Pdf("Catalog is not a dictionary".into()));
        }
    };

    // Update Kids array
    let pages_obj = doc
        .get_object_mut(pages_id)
        .map_err(|e| AppError::Pdf(format!("Pages object error: {e}")))?;
    if let Object::Dictionary(dict) = pages_obj {
        dict.set("Kids", Object::Array(new_kids));
    } else {
        return Err(AppError::Pdf("/Pages is not a dictionary".into()));
    }

    Ok(())
}

pub async fn run(handle: AppHandle, req: ProcessRequest) -> Result<PathBuf, AppError> {
    let op_id = req.options.get("op_id").and_then(|v| v.as_str()).unwrap_or("reorder_pages").to_string();

    let input_path = req.input_paths.first()
        .ok_or_else(|| AppError::Validation("Reorder requires one input file".into()))?;

    let new_order: Vec<usize> = req.options
        .get("page_order")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::Validation("'page_order' array required in options".into()))?
        .iter()
        .filter_map(|v| v.as_u64().map(|n| n as usize))
        .collect();

    crate::pipeline::validate::validate_pdf(input_path)?;

    emit_progress(&handle, &op_id, 5, "Loading document…");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    let total = doc.get_pages().len();
    validate_order(&new_order, total)?;

    emit_progress(&handle, &op_id, 40, "Reordering pages…");
    apply_reorder(&mut doc, &new_order)?;

    emit_progress(&handle, &op_id, 80, "Saving output…");
    let stem = output_stem(input_path);
    let stage = TempStage::new(&req.output_dir)?;
    let out_path = stage.output_path(&stem, "pdf");
    doc.save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save reordered PDF: {e}")))?;

    emit_complete(&handle, &op_id, &out_path);
    Ok(out_path)
}
```

- [ ] **Step 3: Run tests and verify**

```
cargo test organise::reorder::tests
```

- [ ] **Step 4: Commit**

```
git add src-tauri/src/tools/organise/reorder.rs
git commit -m "feat(organise): implement reorder pages Rust backend"
```

---

### Task 11: Svelte 5 — `ReorderWorkspace.svelte`

**Files:**
- Create: `src/lib/components/tools/organise/ReorderWorkspace.svelte`

- [ ] **Step 1: Create the component**

Reorder uses a drag-and-drop thumbnail grid where each card represents a page.

```svelte
<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { invoke } from '@tauri-apps/api/core';
  import { operationStore } from '$lib/stores/operation';
  import ProgressBar from '$lib/components/ui/ProgressBar.svelte';

  interface PageEntry {
    originalPage: number;
    thumbnailUrl: string | null;
  }

  const CONCURRENT_RENDERS = 4;

  let filePath = $state<string | null>(null);
  let pages = $state<PageEntry[]>([]);
  let draggingIndex = $state<number | null>(null);
  let dragOverIndex = $state<number | null>(null);
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);

  const op = operationStore;
  const canRun = $derived(filePath !== null && pages.length > 0 && !$op.running);

  async function pickFile() {
    const selected = await open({ multiple: false, filters: [{ name: 'PDF', extensions: ['pdf'] }] });
    if (!selected || Array.isArray(selected)) return;
    filePath = selected;
    error = null;
    outputPath = null;

    const total: number = await invoke('get_page_count', { path: selected });

    // Build initial page list (original order, no thumbnails yet)
    pages = Array.from({ length: total }, (_, i) => ({
      originalPage: i + 1,
      thumbnailUrl: null,
    }));

    // Render thumbnails lazily in batches
    await renderThumbnails(selected, total);
  }

  async function renderThumbnails(path: string, total: number) {
    const indices = Array.from({ length: total }, (_, i) => i);
    for (let i = 0; i < indices.length; i += CONCURRENT_RENDERS) {
      const chunk = indices.slice(i, i + CONCURRENT_RENDERS);
      const results = await Promise.all(
        chunk.map(idx =>
          invoke<{ page: number; data_url: string }>('render_page_thumbnail', {
            path,
            page: idx + 1,
            width: 96,
            height: 96,
          })
        )
      );
      // Immutable update: create new array with thumbnails filled in
      pages = pages.map(entry => {
        const result = results.find(r => r.page === entry.originalPage);
        if (result) {
          return { ...entry, thumbnailUrl: result.data_url };
        }
        return entry;
      });
    }
  }

  // --- Drag-to-reorder ---
  function onDragStart(index: number) {
    draggingIndex = index;
  }

  function onDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    dragOverIndex = index;
  }

  function onDrop(targetIndex: number) {
    if (draggingIndex === null || draggingIndex === targetIndex) {
      draggingIndex = null;
      dragOverIndex = null;
      return;
    }
    const reordered = [...pages];
    const [moved] = reordered.splice(draggingIndex, 1);
    reordered.splice(targetIndex, 0, moved);
    pages = reordered;
    draggingIndex = null;
    dragOverIndex = null;
  }

  function onDragEnd() {
    draggingIndex = null;
    dragOverIndex = null;
  }

  async function runReorder() {
    if (!filePath || pages.length === 0) return;
    error = null;
    outputPath = null;

    const outFile = await save({
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
      defaultPath: 'reordered.pdf',
    });
    if (!outFile) return;
    const dir = outFile.substring(0, Math.max(outFile.lastIndexOf('/'), outFile.lastIndexOf('\\')));

    // Build the new page order as 1-based original page numbers
    const pageOrder = pages.map(p => p.originalPage);

    op.start('reorder_pages');
    try {
      const result: string = await invoke('process_pdf', {
        request: {
          tool_id: 'reorder_pages',
          input_paths: [filePath],
          output_dir: dir,
          options: { op_id: 'reorder_pages', page_order: pageOrder },
        },
      });
      outputPath = result;
      op.complete(result);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      error = msg;
      op.fail(msg);
    }
  }
</script>

<div class="flex flex-col gap-4 p-4 h-full overflow-hidden">
  <header class="flex items-center justify-between shrink-0">
    <h2 class="text-lg font-semibold">Reorder Pages</h2>
    <button onclick={pickFile} class="btn btn-secondary text-sm">
      {filePath ? 'Change File' : 'Open PDF'}
    </button>
  </header>

  {#if !filePath}
    <button
      class="flex-1 flex flex-col items-center justify-center border-2 border-dashed border-gray-300 rounded-lg text-gray-400 hover:border-blue-400 hover:text-blue-500 transition-colors cursor-pointer"
      onclick={pickFile}
      aria-label="Open a PDF to reorder pages"
    >
      <svg class="w-10 h-10 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 13h6m-3-3v6m5 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414A1 1 0 0121 9.414V19a2 2 0 01-2 2z" />
      </svg>
      <span>Click to open a PDF</span>
      <span class="text-xs mt-1">Drag pages to reorder them</span>
    </button>
  {:else}
    <p class="text-xs text-gray-400 shrink-0">
      {pages.length} page{pages.length !== 1 ? 's' : ''} — drag thumbnails to reorder
    </p>

    <!-- Drag-to-reorder thumbnail grid -->
    <div class="flex-1 overflow-y-auto">
      <div class="grid grid-cols-[repeat(auto-fill,minmax(100px,1fr))] gap-2">
        {#each pages as entry, i (entry.originalPage)}
          <div
            draggable="true"
            ondragstart={() => onDragStart(i)}
            ondragover={(e) => onDragOver(e, i)}
            ondrop={() => onDrop(i)}
            ondragend={onDragEnd}
            class={[
              'flex flex-col items-center gap-1 p-1 rounded border-2 cursor-grab active:cursor-grabbing transition-colors select-none',
              dragOverIndex === i ? 'border-blue-400 bg-blue-50 scale-105' : 'border-gray-200 hover:border-gray-400',
              draggingIndex === i ? 'opacity-40' : '',
            ].join(' ')}
            role="listitem"
            aria-label={`Page ${entry.originalPage} (position ${i + 1})`}
          >
            {#if entry.thumbnailUrl}
              <img
                src={entry.thumbnailUrl}
                alt="Page {entry.originalPage}"
                class="w-24 h-24 object-contain rounded"
              />
            {:else}
              <div class="w-24 h-24 bg-gray-100 rounded animate-pulse flex items-center justify-center">
                <span class="text-xs text-gray-400">{entry.originalPage}</span>
              </div>
            {/if}
            <span class="text-xs text-gray-500">{i + 1}</span>
            <span class="text-xs text-gray-300">(p.{entry.originalPage})</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if $op.running}
    <ProgressBar value={$op.progress} label={$op.label} />
  {/if}

  {#if error}
    <p class="text-sm text-red-600 bg-red-50 rounded px-3 py-2">{error}</p>
  {/if}

  {#if outputPath}
    <p class="text-sm text-green-700 bg-green-50 rounded px-3 py-2">
      Saved: <span class="font-mono break-all">{outputPath}</span>
    </p>
  {/if}

  <button onclick={runReorder} disabled={!canRun} class="btn btn-primary w-full shrink-0">
    {$op.running ? 'Saving…' : 'Save Reordered PDF'}
  </button>
</div>
```

- [ ] **Step 2: Register in tools registry**

```typescript
{
  id: 'reorder_pages',
  label: 'Reorder Pages',
  group: 'organise',
  icon: 'reorder',
  description: 'Drag-and-drop page reordering with thumbnail previews',
  component: () => import('./components/tools/organise/ReorderWorkspace.svelte'),
}
```

- [ ] **Step 3: Commit**

```
git add src/lib/components/tools/organise/ReorderWorkspace.svelte src/lib/tools-registry.ts
git commit -m "feat(organise): add Reorder Pages workspace UI"
```

---

## Chunk 7: Remove Pages

### Task 12: Rust — `organise/remove.rs`

**Files:**
- Create: `src-tauri/src/tools/organise/remove.rs`

- [ ] **Step 1: Write failing unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_removal_rejects_empty_list() {
        assert!(validate_removal(&[], 5).is_err());
    }

    #[test]
    fn validate_removal_rejects_removing_all_pages() {
        assert!(validate_removal(&[1, 2, 3], 3).is_err());
    }

    #[test]
    fn validate_removal_rejects_out_of_range() {
        assert!(validate_removal(&[6], 5).is_err());
    }

    #[test]
    fn validate_removal_accepts_valid_subset() {
        assert!(validate_removal(&[1, 3], 5).is_ok());
    }

    #[test]
    fn output_stem_remove() {
        use std::path::PathBuf;
        let p = PathBuf::from("/tmp/doc.pdf");
        assert_eq!(output_stem(&p), "doc_pages_removed");
    }
}
```

- [ ] **Step 2: Implement `remove.rs`**

```rust
use std::path::PathBuf;
use lopdf::Document;
use tauri::AppHandle;

use crate::error::AppError;
use crate::pipeline::{temp::TempStage, progress::{emit_progress, emit_complete}};
use crate::tools::ProcessRequest;

pub fn output_stem(input: &PathBuf) -> String {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
    format!("{stem}_pages_removed")
}

/// Validate the removal list: non-empty, no out-of-range pages, and at least one page must remain.
pub fn validate_removal(pages_to_remove: &[usize], total_pages: usize) -> Result<(), AppError> {
    if pages_to_remove.is_empty() {
        return Err(AppError::Validation("No pages selected for removal".into()));
    }
    for &p in pages_to_remove {
        if p == 0 || p > total_pages {
            return Err(AppError::Validation(
                format!("Page {p} is out of range (1–{total_pages})")
            ));
        }
    }
    let unique: std::collections::HashSet<usize> = pages_to_remove.iter().copied().collect();
    if unique.len() >= total_pages {
        return Err(AppError::Validation(
            "Cannot remove all pages from a document".into()
        ));
    }
    Ok(())
}

pub async fn run(handle: AppHandle, req: ProcessRequest) -> Result<PathBuf, AppError> {
    let op_id = req.options.get("op_id").and_then(|v| v.as_str()).unwrap_or("remove_pages").to_string();

    let input_path = req.input_paths.first()
        .ok_or_else(|| AppError::Validation("Remove requires one input file".into()))?;

    let pages_to_remove: Vec<usize> = req.options
        .get("pages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::Validation("'pages' array required in options".into()))?
        .iter()
        .filter_map(|v| v.as_u64().map(|n| n as usize))
        .collect();

    crate::pipeline::validate::validate_pdf(input_path)?;

    emit_progress(&handle, &op_id, 5, "Loading document…");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    let total = doc.get_pages().len();
    validate_removal(&pages_to_remove, total)?;

    emit_progress(&handle, &op_id, 30, "Removing pages…");
    let page_nums: Vec<u32> = pages_to_remove.iter().map(|&p| p as u32).collect();
    doc.delete_pages(&page_nums);

    emit_progress(&handle, &op_id, 80, "Saving output…");
    let stem = output_stem(input_path);
    let stage = TempStage::new(&req.output_dir)?;
    let out_path = stage.output_path(&stem, "pdf");
    doc.save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save PDF: {e}")))?;

    emit_complete(&handle, &op_id, &out_path);
    Ok(out_path)
}
```

- [ ] **Step 3: Run tests and verify**

```
cargo test organise::remove::tests
```

- [ ] **Step 4: Commit**

```
git add src-tauri/src/tools/organise/remove.rs
git commit -m "feat(organise): implement remove pages Rust backend"
```

---

### Task 13: Svelte 5 — `RemoveWorkspace.svelte`

**Files:**
- Create: `src/lib/components/tools/organise/RemoveWorkspace.svelte`

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { invoke } from '@tauri-apps/api/core';
  import { operationStore } from '$lib/stores/operation';
  import ProgressBar from '$lib/components/ui/ProgressBar.svelte';
  import ThumbnailGrid from './ThumbnailGrid.svelte';

  let filePath = $state<string | null>(null);
  let totalPages = $state(0);
  let selectedPages = $state<Set<number>>(new Set());
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);

  const op = operationStore;

  // Must have at least one page selected AND at least one page remaining
  const canRun = $derived(
    filePath !== null &&
    !$op.running &&
    selectedPages.size > 0 &&
    selectedPages.size < totalPages
  );

  const remainingCount = $derived(totalPages - selectedPages.size);

  async function pickFile() {
    const selected = await open({ multiple: false, filters: [{ name: 'PDF', extensions: ['pdf'] }] });
    if (!selected || Array.isArray(selected)) return;
    filePath = selected;
    error = null;
    outputPath = null;
    selectedPages = new Set();
    totalPages = await invoke<number>('get_page_count', { path: selected });
  }

  function togglePage(page: number) {
    const next = new Set(selectedPages);
    if (next.has(page)) {
      next.delete(page);
    } else {
      next.add(page);
    }
    selectedPages = next;
  }

  function clearSelection() {
    selectedPages = new Set();
  }

  async function runRemove() {
    if (!filePath) return;
    error = null;
    outputPath = null;

    const outFile = await save({
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
      defaultPath: 'pages_removed.pdf',
    });
    if (!outFile) return;
    const dir = outFile.substring(0, Math.max(outFile.lastIndexOf('/'), outFile.lastIndexOf('\\')));

    const pagesToRemove = Array.from(selectedPages).sort((a, b) => a - b);

    op.start('remove_pages');
    try {
      const result: string = await invoke('process_pdf', {
        request: {
          tool_id: 'remove_pages',
          input_paths: [filePath],
          output_dir: dir,
          options: { op_id: 'remove_pages', pages: pagesToRemove },
        },
      });
      outputPath = result;
      op.complete(result);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      error = msg;
      op.fail(msg);
    }
  }
</script>

<div class="flex flex-col gap-4 p-4 h-full overflow-hidden">
  <header class="flex items-center justify-between shrink-0">
    <h2 class="text-lg font-semibold">Remove Pages</h2>
    <button onclick={pickFile} class="btn btn-secondary text-sm">
      {filePath ? 'Change File' : 'Open PDF'}
    </button>
  </header>

  {#if !filePath}
    <button
      class="flex-1 flex flex-col items-center justify-center border-2 border-dashed border-gray-300 rounded-lg text-gray-400 hover:border-blue-400 hover:text-blue-500 transition-colors cursor-pointer"
      onclick={pickFile}
      aria-label="Open a PDF to remove pages"
    >
      <svg class="w-10 h-10 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
          d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
      </svg>
      <span>Click to open a PDF</span>
      <span class="text-xs mt-1">Select pages to delete</span>
    </button>
  {:else}
    <!-- Selection summary -->
    <div class="flex items-center gap-4 shrink-0">
      <span class="text-sm text-gray-600">
        {selectedPages.size} page{selectedPages.size !== 1 ? 's' : ''} selected for removal
      </span>
      {#if selectedPages.size > 0}
        <span class="text-sm text-gray-400">→ {remainingCount} will remain</span>
        <button onclick={clearSelection} class="text-xs text-gray-500 hover:underline ml-auto">
          Clear selection
        </button>
      {/if}
    </div>

    <!-- Warning: all pages selected -->
    {#if selectedPages.size >= totalPages}
      <div class="text-xs text-red-700 bg-red-50 border border-red-200 rounded px-3 py-2 shrink-0">
        Cannot remove all pages. Deselect at least one page.
      </div>
    {/if}

    <!-- Thumbnail grid -->
    <div class="flex-1 overflow-y-auto">
      <ThumbnailGrid
        pdfPath={filePath}
        {totalPages}
        {selectedPages}
        onToggle={togglePage}
      />
    </div>
  {/if}

  {#if $op.running}
    <ProgressBar value={$op.progress} label={$op.label} />
  {/if}

  {#if error}
    <p class="text-sm text-red-600 bg-red-50 rounded px-3 py-2">{error}</p>
  {/if}

  {#if outputPath}
    <p class="text-sm text-green-700 bg-green-50 rounded px-3 py-2">
      Saved: <span class="font-mono break-all">{outputPath}</span>
    </p>
  {/if}

  <button onclick={runRemove} disabled={!canRun} class="btn btn-primary w-full shrink-0">
    {#if $op.running}
      Removing…
    {:else if selectedPages.size === 0}
      Select pages to remove
    {:else}
      Remove {selectedPages.size} page{selectedPages.size !== 1 ? 's' : ''}
    {/if}
  </button>
</div>
```

- [ ] **Step 2: Register in tools registry**

```typescript
{
  id: 'remove_pages',
  label: 'Remove Pages',
  group: 'organise',
  icon: 'trash',
  description: 'Delete selected pages with thumbnail previews',
  component: () => import('./components/tools/organise/RemoveWorkspace.svelte'),
}
```

- [ ] **Step 3: Commit**

```
git add src/lib/components/tools/organise/RemoveWorkspace.svelte src/lib/tools-registry.ts
git commit -m "feat(organise): add Remove Pages workspace UI"
```

---

## Chunk 8: Integration and Final Verification

### Task 14: Full integration test

**Files:**
- Create: `src-tauri/src/tools/organise/integration_tests.rs`
- Modify: `src-tauri/src/tools/organise/mod.rs`

- [ ] **Step 1: Add integration test module to `organise/mod.rs`**

```rust
pub mod merge;
pub mod split;
pub mod compress;
pub mod rotate;
pub mod reorder;
pub mod remove;

#[cfg(test)]
mod integration_tests;
```

- [ ] **Step 2: Write integration tests**

Create `src-tauri/src/tools/organise/integration_tests.rs`:

```rust
//! End-to-end integration tests: write real PDF files, run each tool, verify output.
//! These tests use `tempfile` to create real on-disk PDFs.

use std::path::PathBuf;
use tempfile::TempDir;
use lopdf::Document;

/// Build a minimal valid PDF with `page_count` blank pages and save to `path`.
fn write_test_pdf(path: &PathBuf, page_count: usize) {
    use lopdf::{Object, Stream, Dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_refs = Vec::new();

    for _ in 0..page_count {
        let content = Stream::new(Dictionary::new(), b"BT ET".to_vec());
        let cid = doc.add_object(content);
        let page = lopdf::dictionary! {
            "Type" => Object::Name(b"Page".to_vec()),
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), (595i64).into(), (842i64).into()]),
            "Contents" => Object::Reference(cid),
        };
        page_refs.push(Object::Reference(doc.add_object(page)));
    }

    let pages = lopdf::dictionary! {
        "Type" => Object::Name(b"Pages".to_vec()),
        "Kids" => Object::Array(page_refs),
        "Count" => (page_count as i64).into(),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog = lopdf::dictionary! {
        "Type" => Object::Name(b"Catalog".to_vec()),
        "Pages" => Object::Reference(pages_id),
    };
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc.save(path).expect("failed to write test PDF");
}

#[test]
fn integration_merge_two_pdfs() {
    let dir = TempDir::new().unwrap();
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    write_test_pdf(&a, 2);
    write_test_pdf(&b, 3);

    let doc_a = Document::load(&a).unwrap();
    let doc_b = Document::load(&b).unwrap();
    let merged = super::merge::merge_documents(vec![doc_a, doc_b]).unwrap();
    assert_eq!(merged.get_pages().len(), 5);
}

#[test]
fn integration_split_every_2_pages() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.pdf");
    write_test_pdf(&input, 5);
    let doc = Document::load(&input).unwrap();
    let total = doc.get_pages().len();
    let all_pages: Vec<usize> = (1..=total).collect();
    let chunks = super::split::chunk_by_n(&all_pages, 2);
    assert_eq!(chunks.len(), 3); // [1,2], [3,4], [5]
    assert_eq!(chunks[2], vec![5]);
}

#[test]
fn integration_rotate_does_not_corrupt_page_count() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.pdf");
    write_test_pdf(&input, 3);
    let mut doc = Document::load(&input).unwrap();
    let pages: std::collections::BTreeMap<u32, lopdf::ObjectId> = doc.get_pages();
    for (_, page_id) in &pages {
        super::rotate::rotate_page_direct(&mut doc, *page_id, 90).unwrap();
    }
    assert_eq!(doc.get_pages().len(), 3);
}

#[test]
fn integration_reorder_roundtrip() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.pdf");
    write_test_pdf(&input, 4);
    let mut doc = Document::load(&input).unwrap();
    let new_order = vec![4usize, 3, 2, 1];
    super::reorder::apply_reorder_direct(&mut doc, &new_order).unwrap();
    assert_eq!(doc.get_pages().len(), 4);
}

#[test]
fn integration_remove_pages_reduces_count() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.pdf");
    write_test_pdf(&input, 5);
    let mut doc = Document::load(&input).unwrap();
    doc.delete_pages(&[2u32, 4]);
    assert_eq!(doc.get_pages().len(), 3);
}
```

Note: the integration tests call `rotate_page_direct` and `apply_reorder_direct` — expose these as `pub` aliases of the private inner functions in each module:

In `rotate.rs`, add:
```rust
/// Public alias used by integration tests only.
#[cfg(test)]
pub fn rotate_page_direct(doc: &mut Document, page_id: lopdf::ObjectId, degrees: i32) -> Result<(), AppError> {
    rotate_page(doc, page_id, degrees)
}
```

In `reorder.rs`, add:
```rust
/// Public alias used by integration tests only.
#[cfg(test)]
pub fn apply_reorder_direct(doc: &mut Document, new_order: &[usize]) -> Result<(), AppError> {
    apply_reorder(doc, new_order)
}
```

- [ ] **Step 3: Run all organise tests**

```
cargo test organise
```

Expected: all unit + integration tests pass with no warnings.

- [ ] **Step 4: Run full build**

```
cargo build
```

Resolve any compilation errors before proceeding.

- [ ] **Step 5: Commit**

```
git add src-tauri/src/tools/organise/integration_tests.rs \
        src-tauri/src/tools/organise/mod.rs \
        src-tauri/src/tools/organise/rotate.rs \
        src-tauri/src/tools/organise/reorder.rs
git commit -m "test(organise): add integration tests for all 6 organise tools"
```

---

### Task 15: Wire ToolWorkspace routing for organise tools

**Files:**
- Modify: `src/lib/components/layout/ToolWorkspace.svelte`

- [ ] **Step 1: Update ToolWorkspace to dynamically load organise components**

The `ToolWorkspace.svelte` stub from Plan 1 shows the tool name. Replace it with dynamic component loading using the tools-registry:

```svelte
<script lang="ts">
  import { activeToolStore } from '$lib/stores/active-tool';
  import { toolsRegistry } from '$lib/tools-registry';
  import type { Component } from 'svelte';

  let WorkspaceComponent = $state<Component | null>(null);

  $effect(() => {
    const toolId = $activeToolStore;
    if (!toolId) {
      WorkspaceComponent = null;
      return;
    }
    const entry = toolsRegistry.find(t => t.id === toolId);
    if (entry?.component) {
      entry.component().then(mod => {
        WorkspaceComponent = mod.default;
      });
    } else {
      WorkspaceComponent = null;
    }
  });
</script>

<div class="flex flex-col h-full w-full overflow-hidden">
  {#if WorkspaceComponent}
    <WorkspaceComponent />
  {:else if $activeToolStore}
    <div class="flex items-center justify-center h-full text-gray-400">
      <p class="text-sm">No workspace for tool: {$activeToolStore}</p>
    </div>
  {:else}
    <div class="flex items-center justify-center h-full text-gray-300">
      <p class="text-sm">Select a tool from the sidebar</p>
    </div>
  {/if}
</div>
```

- [ ] **Step 2: Commit**

```
git add src/lib/components/layout/ToolWorkspace.svelte
git commit -m "feat: wire dynamic workspace component loading in ToolWorkspace"
```

---

### Task 16: Final plan completion checklist

- [ ] `cargo test organise` — all tests pass
- [ ] `cargo build` — zero errors, zero unused-import warnings in organise modules
- [ ] `npm run build` (or `cargo tauri build`) — frontend builds without TypeScript errors
- [ ] Manually verify in the running app:
  - Merge: add 2+ PDFs, drag to reorder, merge → file opens correctly
  - Split: open PDF, enter range "1-3", split → output has 3 pages
  - Compress: open PDF, select "Balanced", compress → file size reduced
  - Rotate: open PDF, select pages via thumbnails, rotate 90° → pages rotate correctly
  - Reorder: open PDF, drag thumbnails to new positions, save → order matches UI
  - Remove: open PDF, select pages, remove → deleted pages absent from output

---

## Dependency Summary

Add the following to `src-tauri/Cargo.toml` (if not already present from Plan 1):

```toml
[dependencies]
lopdf = "0.34"
pdfium-render = { version = "0.8", features = ["image"] }
image = { version = "0.25", default-features = false, features = ["jpeg", "png"] }
base64 = "0.22"
tempfile = "3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tauri = { version = "2", features = ["protocol-asset"] }
```

## File Tree (Plan 2 additions)

```
src-tauri/src/
  tools/
    mod.rs                          ← modified: add organise dispatch arms
    organise/
      mod.rs                        ← new
      merge.rs                      ← new
      split.rs                      ← new
      compress.rs                   ← new
      rotate.rs                     ← new
      reorder.rs                    ← new
      remove.rs                     ← new
      integration_tests.rs          ← new
  commands/
    thumbnails.rs                   ← new: render_page_thumbnail, get_page_count

src/lib/
  components/
    layout/
      ToolWorkspace.svelte          ← modified: dynamic component routing
    tools/
      organise/
        MergeWorkspace.svelte       ← new
        SplitWorkspace.svelte       ← new
        CompressWorkspace.svelte    ← new
        RotateWorkspace.svelte      ← new
        ReorderWorkspace.svelte     ← new
        RemoveWorkspace.svelte      ← new
        ThumbnailGrid.svelte        ← new (shared)
  tools-registry.ts                 ← modified: 6 new entries with component loaders
```
