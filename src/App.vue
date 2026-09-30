<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { provideWorkspace } from './composables/useWorkspaceContext';
import { CircleCheck, CircleX, X } from 'lucide-vue-next';
import { usePicbedWorkspace } from './composables/usePicbedWorkspace';
import CloseConfirmDialog from './components/dialogs/CloseConfirmDialog.vue';
import DeleteConfigDialog from './components/dialogs/DeleteConfigDialog.vue';
import GithubProxyDialog from './components/dialogs/GithubProxyDialog.vue';
import TaskProgressDialog from './components/dialogs/TaskProgressDialog.vue';
import OverviewMetrics from './components/workspace/OverviewMetrics.vue';
import ConvertPanel from './components/workspace/ConvertPanel.vue';
import LocalUploadPanel from './components/workspace/LocalUploadPanel.vue';
import DownloadPanel from './components/workspace/DownloadPanel.vue';
import ConfigsPanel from './components/workspace/ConfigsPanel.vue';
import RecordsPanel from './components/workspace/RecordsPanel.vue';
import CloudSyncPanel from './components/workspace/CloudSyncPanel.vue';
import AboutPanel from './components/workspace/AboutPanel.vue';
import WorkspaceHeader from './components/workspace/WorkspaceHeader.vue';
import WorkspaceTabs from './components/workspace/WorkspaceTabs.vue';

const closeConfirmOpen = ref(false);
let unlistenCloseRequested: (() => void) | undefined;

/// 视口较小时按比例整体缩放工作区，保证 13~16 英寸屏幕都有合适的布局密度
function applyWorkspaceScale() {
  const app = document.getElementById('app');
  if (!app) return;
  const scale = Math.min(1, Math.max(0.72, window.innerWidth / 1500));
  const body = app.style as CSSStyleDeclaration & { zoom?: string };
  body.zoom = scale >= 0.999 ? '' : scale.toFixed(3);
}

onMounted(() => {
  applyWorkspaceScale();
  window.addEventListener('resize', applyWorkspaceScale);
});

onMounted(async () => {
  unlistenCloseRequested = await getCurrentWindow().onCloseRequested(event => {
    event.preventDefault();
    closeConfirmOpen.value = true;
  });
});
onBeforeUnmount(() => {
  unlistenCloseRequested?.();
  window.removeEventListener('resize', applyWorkspaceScale);
});

function exitApp() {
  void invoke('exit_app');
}
function hideToTray() {
  closeConfirmOpen.value = false;
  void invoke('hide_main_window');
}

const {
  activeTab,
  toast,
  toastKind,
  toastNoticeKey,
  loading,
  showMessage,
  showError,
  taskProgress,
  closeTaskProgress,
  configForm,
  configErrors,
  convertForm,
  pasteForm,
  configs,
  records,
  recordDetail,
  recordDetailOpen,
  batchFiles,
  deleteTarget,
  targetDropdownOpen,
  configTypeDropdownOpen,
  uploadDragActive,
  githubProxyDialogOpen,
  githubProxyEnabled,
  githubProxyURL,
  localTargetConfigId,
  localTargetDropdownOpen,
  localDocumentDragActive,
  localImageDragActive,
  localDocuments,
  localImages,
  secretVisibility,
  isAuthed,
  supportedTypes,
  selectedFields,
  targetConfigs,
  selectedTargetConfig,
  totalImages,
  convertedCount,
  canConvertBatch,
  hasGithubImages,
  localTargetConfigs,
  selectedLocalTargetConfig,
  localMatchedCount,
  localMissingCount,
  localConvertedCount,
  canUploadLocalBatch,
  successRecords,
  typeLabel,
  fieldLabel,
  fieldPlaceholder,
  secretFieldVisible,
  toggleSecretField,
  statusLabel,
  targetConfigLabel,
  localTargetConfigLabel,
  selectTargetConfig,
  selectLocalTargetConfig,
  selectConfigType,
  handleConfigTypeChange,
  resetConfigForm,
  resetConvertForm,
  setActiveTab,
  editConfig,
  saveConfig,
  testConfig,
  requestDeleteConfig,
  cancelDeleteConfig,
  confirmDeleteConfig,
  setDefault,
  handleFiles,
  handleFileDrop,
  addPastedDocument,
  removeBatchFile,
  localStatusLabel,
  handleLocalDocumentFiles,
  handleLocalDocumentDrop,
  handleLocalImageFiles,
  handleLocalImageDrop,
  removeLocalDocument,
  removeLocalImage,
analyzeBatch,
  convertBatch,
  analyzeLocalBatch,
  uploadLocalBatch,
  downloadDocuments,
  downloadDragActive,
  downloadItems,
  downloadTargetDir,
  downloadActive,
  downloadTotalCount,
  downloadSuccessCount,
  downloadFailedCount,
  canDownloadBatch,
  handleDownloadDocumentFiles,
  handleDownloadDocumentDrop,
  removeDownloadDocument,
  selectDownloadDir,
  analyzeDownloadDocuments,
  downloadAllImages,
  syncAccount,
  secretBackend,
  syncPassword,
  syncPatInput,
  syncRemoteState,
  syncBusy,
  syncConnected,
  localConfigCount,
  syncStatusBadge,
  remoteVersionBadge,
  lastSyncText,
  canSyncNow,
  syncNow,
  syncBanner,
  overrideCloudWithLocal,
  pushEmptyVaultLocal,
  resolveConflictWithCloudPassword,
  restoreRemoteVersion,
  forcePushLocal,
  passwordStatus,
  passwordBackend,
  lockSyncPassword,
  autoSyncDisplayChecked,
  autoSyncToggleEnabled,
  autoSyncToggleTitle,
  setAutoSyncEnabled,
  passwordModalOpen,
  passwordModalMode,
  openPasswordModal,
  closePasswordModal,
  cloudPwdModalOpen,
  openCloudPwdModal,
  closeCloudPwdModal,
  submitPasswordModal,
  unlockWithPassword,
  githubConnectOpen,
  githubConnectPreparing,
  deviceFlowUserCode,
  deviceFlowStatus,
  deviceFlowError,
  deviceFlowAvailable,
  patMode,
  autoSyncEnabled,
  openGithubConnect,
  cancelGithubConnect,
  openVerificationPage,
  copyUserCode,
  connectGithub,
  disconnectGithub,
  syncHistoryOpen,
  syncHistoryLoading,
  syncHistory,
  selectedRevisionSha,
  revisionPreview,
  revisionPreviewLoading,
  revisionError,
  restoringRevision,
  toggleSyncHistory,
  selectRevision,
  restoreRevision,
  updateStatus,
  updateVersion,
  checkUpdate,
  startUpdate,
  closeGithubProxyDialog,
  confirmGithubProxyConvert,
  togglePreview,
  changedLines,
  downloadFile,
  downloadAll,
  downloadLocalFile,
  downloadAllLocalFiles,
  loadRecords,
  openRecordDetail,
  closeRecordDetail,
  deleteRecords,
  resetLocalUploadForm,
  clearWorkspaceDrafts,
} = usePicbedWorkspace();

provideWorkspace({
  activeTab, toast, toastKind, toastNoticeKey, loading, showMessage, showError,
  taskProgress,
  closeTaskProgress,
  configForm, configErrors, convertForm, pasteForm, configs, records, recordDetail, recordDetailOpen, batchFiles, deleteTarget,
  targetDropdownOpen, configTypeDropdownOpen, uploadDragActive, githubProxyDialogOpen, githubProxyEnabled, githubProxyURL,
  localTargetConfigId, localTargetDropdownOpen, localDocumentDragActive, localImageDragActive, localDocuments, localImages,
  downloadDocuments, downloadDragActive, downloadItems, downloadTargetDir, downloadActive, downloadTotalCount, downloadSuccessCount,
  downloadFailedCount, canDownloadBatch,
  secretVisibility,
  isAuthed, supportedTypes, selectedFields, targetConfigs, selectedTargetConfig, totalImages, convertedCount,
  canConvertBatch, hasGithubImages, localTargetConfigs, selectedLocalTargetConfig, localMatchedCount, localMissingCount,
  localConvertedCount, canUploadLocalBatch, successRecords, typeLabel, fieldLabel, fieldPlaceholder, secretFieldVisible, toggleSecretField,
  statusLabel, targetConfigLabel, localTargetConfigLabel, selectTargetConfig, selectLocalTargetConfig, selectConfigType, handleConfigTypeChange,
  resetConfigForm, resetConvertForm, setActiveTab, editConfig, saveConfig, testConfig, requestDeleteConfig, cancelDeleteConfig,
  confirmDeleteConfig, setDefault, handleFiles, handleFileDrop, addPastedDocument, removeBatchFile, localStatusLabel,
  handleLocalDocumentFiles, handleLocalDocumentDrop, handleLocalImageFiles, handleLocalImageDrop, removeLocalDocument, removeLocalImage,
  handleDownloadDocumentFiles, handleDownloadDocumentDrop, removeDownloadDocument, selectDownloadDir, analyzeDownloadDocuments, downloadAllImages,
  syncAccount, secretBackend, syncPassword, syncPatInput, syncRemoteState, syncBusy, syncConnected, localConfigCount,
  syncStatusBadge, remoteVersionBadge, lastSyncText, canSyncNow, syncNow, syncBanner, overrideCloudWithLocal, pushEmptyVaultLocal, resolveConflictWithCloudPassword, restoreRemoteVersion, forcePushLocal,
  passwordStatus, passwordBackend, lockSyncPassword, passwordModalOpen, passwordModalMode, openPasswordModal, closePasswordModal, cloudPwdModalOpen, openCloudPwdModal, closeCloudPwdModal,
  submitPasswordModal, unlockWithPassword, autoSyncDisplayChecked, autoSyncToggleEnabled, autoSyncToggleTitle, setAutoSyncEnabled, githubConnectOpen, githubConnectPreparing, deviceFlowUserCode, deviceFlowStatus, deviceFlowError, deviceFlowAvailable, patMode,
  autoSyncEnabled, openGithubConnect, cancelGithubConnect, openVerificationPage, copyUserCode, connectGithub, disconnectGithub,
  syncHistoryOpen, syncHistoryLoading, syncHistory, selectedRevisionSha, revisionPreview, revisionPreviewLoading, revisionError, restoringRevision,
  toggleSyncHistory, selectRevision, restoreRevision, updateStatus, updateVersion, checkUpdate, startUpdate,
analyzeBatch, convertBatch, analyzeLocalBatch, uploadLocalBatch, closeGithubProxyDialog, confirmGithubProxyConvert,
  togglePreview, changedLines, downloadFile, downloadAll, downloadLocalFile, downloadAllLocalFiles, loadRecords,
  openRecordDetail, closeRecordDetail, deleteRecords, resetLocalUploadForm, clearWorkspaceDrafts,
});

</script>
<template>
  <main class="app-shell">
    <div v-if="toast" :key="toastNoticeKey" class="workspace-toast" :data-kind="toastKind" role="alert">
      <CircleCheck v-if="toastKind === 'success'" :size="18" />
      <CircleX v-else :size="18" />
      <span>{{ toast }}</span>
      <button class="toast-close" type="button" aria-label="关闭提示" @click="toast = ''">
        <X :size="16" />
      </button>
    </div>

    <section v-if="isAuthed" class="workspace">
      <WorkspaceHeader />
      <OverviewMetrics />
      <WorkspaceTabs />

      <ConvertPanel v-if="activeTab === 'convert'" />
      <LocalUploadPanel v-if="activeTab === 'localUpload'" />
      <DownloadPanel v-if="activeTab === 'download'" />
      <ConfigsPanel v-if="activeTab === 'configs'" />
      <RecordsPanel v-if="activeTab === 'records'" />
      <CloudSyncPanel v-if="activeTab === 'cloudSync'" />
      <AboutPanel v-if="activeTab === 'about'" />
    </section>
    <GithubProxyDialog />
    <TaskProgressDialog />
    <CloseConfirmDialog
      :open="closeConfirmOpen"
      @confirm-exit="exitApp"
      @hide-to-tray="hideToTray"
      @cancel="closeConfirmOpen = false"
    />
    <DeleteConfigDialog />
  </main>
</template>
