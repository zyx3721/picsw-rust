import { reactive } from 'vue';
import type { TaskKind, TaskProgressState, TaskProgressStatus } from './types';

type StartTaskInput = {
  title: string;
  message: string;
  total: number;
  detail?: string;
  taskId?: number;
  taskKind?: TaskKind;
};

type UpdateTaskInput = Partial<Pick<TaskProgressState, 'message' | 'detail' | 'current' | 'success' | 'partial' | 'failed' | 'documents' | 'total' | 'taskId' | 'taskKind' | 'resumable'>> & {
  status?: TaskProgressStatus;
};

export function useTaskProgress() {
  const taskProgress = reactive<TaskProgressState>({
    open: false,
    title: '',
    message: '',
    detail: '',
    current: 0,
    total: 0,
    success: 0,
    partial: 0,
    failed: 0,
    status: 'idle',
    closable: true,
    documents: [],
    taskId: 0,
    taskKind: '',
    resumable: false,
  });

  function startTaskProgress(input: StartTaskInput) {
    taskProgress.open = true;
    taskProgress.title = input.title;
    taskProgress.message = input.message;
    taskProgress.detail = input.detail || '';
    taskProgress.current = 0;
    taskProgress.total = input.total;
    taskProgress.success = 0;
    taskProgress.partial = 0;
    taskProgress.failed = 0;
    taskProgress.status = 'running';
    taskProgress.closable = false;
    taskProgress.documents = [];
    taskProgress.taskId = input.taskId || 0;
    taskProgress.taskKind = input.taskKind || '';
    taskProgress.resumable = false;
  }

  function updateTaskProgress(input: UpdateTaskInput) {
    if (input.message !== undefined) taskProgress.message = input.message;
    if (input.detail !== undefined) taskProgress.detail = input.detail;
    if (input.current !== undefined) taskProgress.current = input.current;
    if (input.total !== undefined) taskProgress.total = input.total;
    if (input.success !== undefined) taskProgress.success = input.success;
    if (input.partial !== undefined) taskProgress.partial = input.partial;
    if (input.failed !== undefined) taskProgress.failed = input.failed;
    if (input.documents !== undefined) taskProgress.documents = input.documents;
    if (input.taskId !== undefined) taskProgress.taskId = input.taskId;
    if (input.taskKind !== undefined) taskProgress.taskKind = input.taskKind;
    if (input.resumable !== undefined) taskProgress.resumable = input.resumable;
    if (input.status !== undefined) taskProgress.status = input.status;
  }

  function finishTaskProgress(input: {
    status: Exclude<TaskProgressStatus, 'idle' | 'running'>;
    message: string;
    detail?: string;
    current?: number;
    resumable?: boolean;
  }) {
    taskProgress.status = input.status;
    taskProgress.message = input.message;
    taskProgress.detail = input.detail || '';
    taskProgress.current = input.current ?? taskProgress.total;
    taskProgress.closable = true;
    taskProgress.resumable = input.resumable === true;
  }

  function closeTaskProgress() {
    if (!taskProgress.closable) return;
    taskProgress.open = false;
    taskProgress.status = 'idle';
    taskProgress.documents = [];
    taskProgress.taskId = 0;
    taskProgress.taskKind = '';
    taskProgress.resumable = false;
  }

  return { taskProgress, startTaskProgress, updateTaskProgress, finishTaskProgress, closeTaskProgress };
}
