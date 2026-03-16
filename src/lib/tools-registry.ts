import type { ToolMeta, CategoryMeta } from './types';

export const CATEGORIES: CategoryMeta[] = [
  { id: 'organise',     label: 'Organise',     icon: '📋' },
  { id: 'pdf_to_other', label: 'PDF → Other',  icon: '📤' },
  { id: 'other_to_pdf', label: 'Other → PDF',  icon: '📥' },
  { id: 'edit',         label: 'Edit',         icon: '✏️' },
  { id: 'security',     label: 'Security',     icon: '🔒' },
  { id: 'repair',       label: 'Repair & OCR', icon: '🔬' },
];

export const TOOLS: ToolMeta[] = [
  // Organise
  { id: 'merge',    label: 'Merge PDF',      icon: '⊕',  category: 'organise',     description: 'Combine multiple PDFs into one' },
  { id: 'split',    label: 'Split PDF',      icon: '✂️', category: 'organise',     description: 'Split by page range or every N pages' },
  { id: 'compress', label: 'Compress PDF',   icon: '🗜', category: 'organise',     description: 'Reduce file size with quality presets' },
  { id: 'rotate',   label: 'Rotate Pages',   icon: '🔄', category: 'organise',     description: 'Rotate individual or all pages' },
  { id: 'reorder',  label: 'Reorder Pages',  icon: '📑', category: 'organise',     description: 'Drag and drop to reorder pages' },
  { id: 'remove',   label: 'Remove Pages',   icon: '🗑', category: 'organise',     description: 'Delete selected pages' },
  // PDF → Other
  { id: 'pdf_to_word',  label: 'PDF → Word',  icon: '📝', category: 'pdf_to_other', description: 'Convert PDF to Word document' },
  { id: 'pdf_to_excel', label: 'PDF → Excel', icon: '📊', category: 'pdf_to_other', description: 'Extract tables to spreadsheet' },
  { id: 'pdf_to_ppt',   label: 'PDF → PPT',   icon: '📽', category: 'pdf_to_other', description: 'Convert pages to PowerPoint slides' },
  { id: 'pdf_to_image', label: 'PDF → Image', icon: '🖼', category: 'pdf_to_other', description: 'Export pages as JPG or PNG' },
  { id: 'pdf_to_pdfa',  label: 'PDF → PDF/A', icon: '🗄', category: 'pdf_to_other', description: 'Convert to archival PDF/A-1b format' },
  // Other → PDF
  { id: 'word_to_pdf',  label: 'Word → PDF',  icon: '📝', category: 'other_to_pdf', description: 'Convert Word document to PDF' },
  { id: 'excel_to_pdf', label: 'Excel → PDF', icon: '📊', category: 'other_to_pdf', description: 'Convert spreadsheet to PDF' },
  { id: 'ppt_to_pdf',   label: 'PPT → PDF',   icon: '📽', category: 'other_to_pdf', description: 'Convert presentation to PDF' },
  { id: 'image_to_pdf', label: 'Image → PDF', icon: '🖼', category: 'other_to_pdf', description: 'Convert JPG/PNG images to PDF' },
  { id: 'html_to_pdf',  label: 'HTML → PDF',  icon: '🌐', category: 'other_to_pdf', description: 'Convert local HTML file to PDF' },
  // Edit
  { id: 'edit',         label: 'Edit PDF',     icon: '🔤', category: 'edit', description: 'Add text boxes, images, and shapes' },
  { id: 'watermark',    label: 'Watermark',    icon: '💧', category: 'edit', description: 'Add text or image watermark' },
  { id: 'page_numbers', label: 'Page Numbers', icon: '#️⃣', category: 'edit', description: 'Add page numbers to your PDF' },
  { id: 'redact',       label: 'Redact PDF',   icon: '⬛', category: 'edit', description: 'Permanently remove sensitive content' },
  // Security
  { id: 'protect', label: 'Protect PDF', icon: '🔐', category: 'security', description: 'Add password protection' },
  { id: 'unlock',  label: 'Unlock PDF',  icon: '🔓', category: 'security', description: 'Remove PDF password' },
  { id: 'sign',    label: 'Sign PDF',    icon: '✍️', category: 'security', description: 'Add your signature to a PDF' },
  // Repair & OCR
  { id: 'ocr',    label: 'OCR PDF',    icon: '🔍', category: 'repair', description: 'Make scanned PDFs searchable' },
  { id: 'repair', label: 'Repair PDF', icon: '🔧', category: 'repair', description: 'Fix broken or corrupted PDFs' },
];

export function toolsByCategory(categoryId: string): ToolMeta[] {
  return TOOLS.filter(t => t.category === categoryId);
}

export function searchTools(query: string): ToolMeta[] {
  const q = query.toLowerCase();
  return TOOLS.filter(t =>
    t.label.toLowerCase().includes(q) ||
    t.description.toLowerCase().includes(q)
  );
}
