import { inject, provide, type InjectionKey } from 'vue';
import { usePicbedWorkspace } from './usePicbedWorkspace';

export type WorkspaceContext = ReturnType<typeof usePicbedWorkspace>;

const workspaceKey: InjectionKey<WorkspaceContext> = Symbol('picbed-workspace');

export function provideWorkspace(workspace: WorkspaceContext) {
  provide(workspaceKey, workspace);
}

export function useWorkspaceContext() {
  const workspace = inject(workspaceKey);
  if (!workspace) throw new Error('Workspace context is not provided');
  return workspace;
}
