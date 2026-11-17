export type User = { id: number; username: string; email: string; email_verified?: boolean; email_verified_at?: string | null };

export type PicbedConfig = {
  id: number;
  picbed_type: string;
  config_name: string;
  is_default: boolean;
  created_at: string;
  updated_at: string;
  config?: Record<string, string | boolean>;
};

export type ConversionRecord = {
  id: number;
  original_filename: string;
  source_picbed: string;
  target_picbed: string;
  status: string;
  error_message?: string;
  image_count: number;
  converted_content?: string;
  details?: ConversionRecordDetail[];
  created_at: string;
};

export type ConversionRecordDetail = {
  id: number;
  original_url: string;
  target_url: string;
  status: string;
  error?: string;
  created_at: string;
};

export type MarkdownImage = { raw: string; url: string; alt: string; picbed: string };

export type ConfigField = {
  key: string;
  label: string;
  placeholder: string;
  required: boolean;
  secret: boolean;
};

export type PicbedTypeDef = { value: string; label: string; description: string; fields: ConfigField[] };

export type BatchFile = {
  id: string;
  filename: string;
  content: string;
  images: MarkdownImage[];
  convertedContent: string;
  changed: number;
  status: 'ready' | 'analyzed' | 'success' | 'partial' | 'failed';
  error: string;
  details?: ConversionRecordDetail[];
  previewOpen?: boolean;
};

export type ConversionTask = {
  id: number;
  task_type: string;
  status: string;
  total: number;
  success: number;
  partial: number;
  failed: number;
  message: string;
  error?: string;
  created_at: string;
  updated_at: string;
};

export type LocalDocument = {
  id: string;
  filename: string;
  content: string;
  references: MarkdownImage[];
  matched: number;
  missing: string[];
  convertedContent: string;
  changed: number;
  status: 'ready' | 'analyzed' | 'success' | 'partial' | 'failed';
  error: string;
  details?: ConversionRecordDetail[];
};

export type LocalImageFile = {
  key: string;
  name: string;
  path: string;
  file?: File;
  sourcePath?: string;
};

export type ScannedLocalImage = {
  name: string;
  path: string;
  relativePath: string;
};

export type LocalImageFilter = 'all' | 'matched' | 'missing';

export type DownloadDocument = {
  id: string;
  filename: string;
  content: string;
  images: MarkdownImage[];
  status: 'ready' | 'analyzed' | 'success' | 'partial' | 'failed';
  error: string;
};

export type DownloadImageItem = {
  key: string;
  docId: string;
  name: string;
  url: string;
  picbed: string;
  status: 'pending' | 'downloading' | 'success' | 'failed' | 'cancelled';
  error: string;
  savedPath?: string;
};

export type DownloadImageFilter = 'all' | 'success' | 'failed';

export type DownloadTaskImage = {
  id: number;
  document_index: number;
  url: string;
  file_name: string;
  saved_path: string;
  status: string;
  error: string;
};

export type DownloadImageResult = {
  url: string;
  file_name: string;
  saved_path: string;
  status: string;
  error: string;
};

export type DownloadProgressEvent = {
  document_index: number;
  filename: string;
  processed: number;
  total: number;
  status: 'running' | 'success' | 'failed';
  error: string;
};

export type RemoteConfigPreview = {
  picbed_type: string;
  config_name: string;
  is_default: boolean;
  config: Record<string, string>;
};

export type SyncRemoteState = {
  version: number;
  updatedAt: number;
  configCount: number;
} | null;

export type TaskKind = 'convert' | 'localUpload' | 'download' | '';

export type TaskProgressStatus = 'idle' | 'running' | 'success' | 'failed' | 'interrupted' | 'cancelled';

export type TaskDocumentProgress = {
  id: number;
  filename: string;
  total_images: number;
  processed_images: number;
  status: 'queued' | 'running' | 'success' | 'partial' | 'failed' | 'interrupted' | 'cancelled';
  error: string;
};

export type TaskProgressState = {
  open: boolean;
  title: string;
  message: string;
  detail: string;
  current: number;
  total: number;
  success: number;
  partial: number;
  failed: number;
  status: TaskProgressStatus;
  closable: boolean;
  documents: TaskDocumentProgress[];
  taskId: number;
  taskKind: TaskKind;
  resumable: boolean;
};

export type RequestError = Error & { status?: number };

export type WorkspaceTab = 'convert' | 'localUpload' | 'download' | 'configs' | 'records' | 'cloudSync' | 'about';
