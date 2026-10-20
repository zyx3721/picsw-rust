import { ref } from 'vue';

export type CloseAction = 'ask' | 'tray' | 'quit';

const CLOSE_ACTION_STORAGE_KEY = 'picbed_close_action';

const CLOSE_ACTION_VALUES: CloseAction[] = ['ask', 'tray', 'quit'];

function readStoredCloseAction(): CloseAction {
  const stored = localStorage.getItem(CLOSE_ACTION_STORAGE_KEY);
  return CLOSE_ACTION_VALUES.includes(stored as CloseAction) ? (stored as CloseAction) : 'ask';
}

/// 关闭窗口行为：ask 每次关闭弹窗询问，tray 直接隐藏到托盘，quit 直接退出应用
export function createCloseBehavior() {
  const closeAction = ref<CloseAction>(readStoredCloseAction());

  function setCloseAction(action: CloseAction) {
    closeAction.value = action;
    localStorage.setItem(CLOSE_ACTION_STORAGE_KEY, action);
  }

  return { closeAction, setCloseAction };
}
