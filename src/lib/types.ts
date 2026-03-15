export type Tool =
  | 'merge' | 'split' | 'compress' | 'rotate' | 'reorder' | 'remove'
  | 'pdf_to_word' | 'pdf_to_excel' | 'pdf_to_ppt' | 'pdf_to_image' | 'pdf_to_pdfa'
  | 'word_to_pdf' | 'excel_to_pdf' | 'ppt_to_pdf' | 'image_to_pdf' | 'html_to_pdf'
  | 'edit' | 'watermark' | 'page_numbers' | 'redact'
  | 'protect' | 'unlock' | 'sign'
  | 'ocr' | 'repair';

export interface ToolMeta {
  id: Tool;
  label: string;
  icon: string;
  category: Category;
  description: string;
}

export type Category =
  | 'organise'
  | 'pdf_to_other'
  | 'other_to_pdf'
  | 'edit'
  | 'security'
  | 'repair';

export interface CategoryMeta {
  id: Category;
  label: string;
  icon: string;
}

export interface RecentEntry {
  path: string;
  tool: Tool;
  timestamp: number;
  exists: boolean;
}

export interface Settings {
  sidebar_collapsed: boolean;
  default_output_folder: string | null;
  ocr_language: string;
  auto_updater_enabled: boolean;
}

export interface ProgressEvent {
  operation_id: string;
  percent: number;
  message: string;
}

export interface ProcessRequest {
  operation_id: string;
  tool: Tool;
  input_paths: string[];
  output_stem: string;
  options: Record<string, unknown>;
}
