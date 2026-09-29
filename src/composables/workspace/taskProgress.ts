import { reactive } from 'vue';
import type { TaskProgressState, TaskProgressStatus } from './types';

type StartTaskInput = {
  title: string;
  message: string;
  total: number;
  detail?: string;
};

type UpdateTaskInput = Partial<Pick<TaskProgressState, 'message' | 'detail' | 'current' | 'success' | 'failed'>> & {
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
    failed: 0,
    status: 'idle',
    closable: true,
  });

  function startTaskProgress(input: StartTaskInput) {
    taskProgress.open = true;
    taskProgress.title = input.title;
    taskProgress.message = input.message;
    taskProgress.detail = input.detail || '';
    taskProgress.current = 0;
    taskProgress.total = input.total;
    taskProgress.success = 0;
    taskProgress.failed = 0;
    taskProgress.status = 'running';
    taskProgress.closable = false;
  }

  function updateTaskProgress(input: UpdateTaskInput) {
    if (input.message !== undefined) taskProgress.message = input.message;
    if (input.detail !== undefined) taskProgress.detail = input.detail;
    if (input.current !== undefined) taskProgress.current = input.current;
    if (input.success !== undefined) taskProgress.success = input.success;
    if (input.failed !== undefined) taskProgress.failed = input.failed;
    if (input.status !== undefined) taskProgress.status = input.status;
  }

  function finishTaskProgress(input: { status: Exclude<TaskProgressStatus, 'idle' | 'running'>; message: string; detail?: string }) {
    taskProgress.status = input.status;
    taskProgress.message = input.message;
    taskProgress.detail = input.detail || '';
    taskProgress.current = taskProgress.total;
    taskProgress.closable = true;
  }

  function closeTaskProgress() {
    if (!taskProgress.closable) return;
    taskProgress.open = false;
    taskProgress.status = 'idle';
  }

  return { taskProgress, startTaskProgress, updateTaskProgress, finishTaskProgress, closeTaskProgress };
}
