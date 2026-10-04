import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useWorkspaceConfigActions } from './workspace/configActions';
import { useWorkspaceConfigForm } from './workspace/configForm';
import { useWorkspaceCloudSync } from './workspace/cloudSyncWorkspace';
import { useWorkspaceConvert } from './workspace/convertWorkspace';
import { useWorkspaceDownload } from './workspace/downloadWorkspace';
import { useWorkspaceLocalUpload } from './workspace/localUploadWorkspace';
import { useWorkspaceNotices } from './workspace/notices';
import { createWorkspaceRequest } from './workspace/request';
import { useTaskProgress } from './workspace/taskProgress';
import { createUpdateChecker } from './workspace/updateChecker';
import { useWorkspaceData } from './workspace/workspaceData';
import type { ConversionRecord, PicbedConfig, WorkspaceTab } from './workspace/types';

export function usePicbedWorkspace() {
  const activeTab = ref<WorkspaceTab>('convert');
  const { toast, toastKind, toastNoticeKey, showMessage, showError, clearNotice, clearToastTimer } =
    useWorkspaceNotices();
  const loading = ref(false);
  const { taskProgress, startTaskProgress, updateTaskProgress, finishTaskProgress, closeTaskProgress } = useTaskProgress();

  const configs = ref<PicbedConfig[]>([]);
  const {
    typeDefs,
    configForm,
    configErrors,
    secretVisibility,
    supportedTypes,
    selectedFields,
    typeLabel,
    fieldLabel,
    fieldPlaceholder,
    secretFieldVisible,
    toggleSecretField,
    handleConfigTypeChange,
    resetConfigForm,
    editConfig,
    validateConfigForm,
    mergeTypeDefs,
  } = useWorkspaceConfigForm({ activeTab, configs, showError, clearNotice });
  const records = ref<ConversionRecord[]>([]);
  const recordDetail = ref<ConversionRecord | null>(null);
  const recordDetailOpen = ref(false);
  const deleteTarget = ref<PicbedConfig | null>(null);
  const configTypeDropdownOpen = ref(false);

  const isAuthed = computed(() => true);
  const successRecords = computed(() => records.value.filter(item => item.status === 'success').length);

  function closeDropdowns() {
    targetDropdownOpen.value = false;
    localTargetDropdownOpen.value = false;
    configTypeDropdownOpen.value = false;
  }
  function handleGlobalPointerDown(event: PointerEvent) {
    const target = event.target;
    if (!(target instanceof Element)) {
      closeDropdowns();
      return;
    }
    if (!target.closest('.custom-select')) closeDropdowns();
  }
  function selectConfigType(value: string) {
    configForm.picbed_type = value;
    configTypeDropdownOpen.value = false;
    handleConfigTypeChange();
  }

  const request = createWorkspaceRequest();
  let reloadRecords: () => Promise<void> = async () => {};
  const {
    convertForm,
    pasteForm,
    batchFiles,
    targetDropdownOpen,
    uploadDragActive,
    githubProxyDialogOpen,
    githubProxyEnabled,
    githubProxyURL,
    targetConfigs,
    defaultTarget,
    selectedTargetConfig,
    totalImages,
    convertedCount,
    canConvertBatch,
    hasGithubImages,
    statusLabel,
    targetConfigLabel,
    selectTargetConfig,
    resetConvertForm,
    handleFiles,
    handleFileDrop,
    addPastedDocument,
    removeBatchFile,
    analyzeBatch,
    convertBatch,
    closeGithubProxyDialog,
    confirmGithubProxyConvert,
    downloadFile,
    downloadAll,
    restoreConvertWorkspace,
    stopConvertTaskPolling,
    togglePreview,
    changedLines,
  } = useWorkspaceConvert({
    configs,
    request,
    showMessage,
    showError,
    clearNotice,
    loadRecords: () => reloadRecords(),
    typeLabel,
    loading,
    startTaskProgress,
    updateTaskProgress,
    finishTaskProgress,
  });
  const {
    localTargetConfigId,
    localTargetDropdownOpen,
    localDocumentDragActive,
    localImageDragActive,
    localDocuments,
    localImages,
    localTargetConfigs,
    selectedLocalTargetConfig,
    localMatchedCount,
    localMissingCount,
    localConvertedCount,
    canUploadLocalBatch,
    localTargetConfigLabel,
    selectLocalTargetConfig,
    localStatusLabel,
    resetLocalUploadForm,
    handleLocalDocumentFiles,
    handleLocalDocumentDrop,
    handleLocalImageFiles,
    handleLocalImageDrop,
    removeLocalDocument,
    removeLocalImage,
    analyzeLocalBatch,
    uploadLocalBatch,
    restoreLocalUploadWorkspace,
    stopLocalUploadTaskPolling,
    downloadLocalFile,
    downloadAllLocalFiles,
  } = useWorkspaceLocalUpload({
    configs,
    request,
    showMessage,
    showError,
    clearNotice,
    loadRecords: () => reloadRecords(),
    typeLabel,
    loading,
    startTaskProgress,
    updateTaskProgress,
    finishTaskProgress,
  });
  const {
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
  } = useWorkspaceDownload({
    request,
    showMessage,
    showError,
    loading,
  });
  const { loadWorkspaceData, loadConfigs, loadRecords } = useWorkspaceData({
    request,
    typeDefs,
    configs,
    records,
    getTargetConfigId: () => convertForm.target_config_id,
    setTargetConfigId: id => {
      convertForm.target_config_id = id;
    },
    getDefaultTarget: () => defaultTarget.value,
    mergeTypeDefs,
  });
  reloadRecords = loadRecords;
  const { saveConfig, requestDeleteConfig, cancelDeleteConfig, confirmDeleteConfig, setDefault, testConfig } = useWorkspaceConfigActions({
    request,
    configForm,
    selectedFields,
    deleteTarget,
    loading,
    validateConfigForm,
    resetConfigForm,
    loadConfigs,
    showMessage,
    showError,
  });
  const {
    syncAccount,
    secretBackend,
    syncPassword,
    syncPatInput,
    syncRemoteState,
    syncBusy,
    syncConnected,
    syncTokenInvalid,
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
    autoSyncDisplayChecked,
    autoSyncToggleEnabled,
    autoSyncToggleTitle,
    setAutoSyncEnabled,
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
  } = useWorkspaceCloudSync({
    request,
    configs,
    showMessage,
    showError,
    reloadConfigs: () => loadConfigs(),
  });

  const {
    updateStatus,
    updateVersion,
    updateProgress,
    updateNotes,
    lastCheckAt,
    showUpToDate,
    autoCheckUpdate,
    setAutoCheckUpdate,
    checkUpdate,
    startUpdate,
    applyUpdate,
    openUpdatePackage,
  } = createUpdateChecker({
    request,
    showMessage,
    showError,
  });

  function clearWorkspaceDrafts() {
    resetConfigForm();
    resetConvertForm();
    resetLocalUploadForm();
    configTypeDropdownOpen.value = false;
    deleteTarget.value = null;
  }
  function setActiveTab(tab: WorkspaceTab) {
    if (activeTab.value === 'configs' || tab === 'configs') resetConfigForm();
    if (activeTab.value === 'convert' || tab === 'convert') resetConvertForm();
    if (activeTab.value === 'localUpload' || tab === 'localUpload') resetLocalUploadForm();
    activeTab.value = tab;
  }
  async function openRecordDetail(record: ConversionRecord) {
    loading.value = true;
    try {
      const data = await request<{ record: ConversionRecord }>(`/api/convert/records/${record.id}`);
      recordDetail.value = data.record;
      recordDetailOpen.value = true;
    } catch (err) {
      showError(err instanceof Error ? err.message : '读取转换记录详情失败');
    } finally {
      loading.value = false;
    }
  }
  function closeRecordDetail() {
    recordDetailOpen.value = false;
    recordDetail.value = null;
  }
  async function deleteRecords(ids: number[]) {
    const uniqueIds = Array.from(new Set(ids.filter(id => id > 0)));
    if (uniqueIds.length === 0) return;
    loading.value = true;
    try {
      const data = await request<{ message: string }>('/api/convert/records', {
        method: 'DELETE',
        body: JSON.stringify({ ids: uniqueIds }),
      });
      if (recordDetail.value && uniqueIds.includes(recordDetail.value.id)) closeRecordDetail();
      records.value = records.value.filter(record => !uniqueIds.includes(record.id));
      showMessage(data.message || `已删除 ${uniqueIds.length} 条转换记录`);
      await loadRecords();
    } catch (err) {
      showError(err instanceof Error ? err.message : '删除转换记录失败');
    } finally {
      loading.value = false;
    }
  }
  onMounted(() => {
    document.addEventListener('pointerdown', handleGlobalPointerDown);
    void loadWorkspaceData()
      .then(() => {
        restoreConvertWorkspace();
        restoreLocalUploadWorkspace();
      })
      .catch(err => {
        showError(err instanceof Error ? err.message : '初始化工作区失败');
      });
  });
  onBeforeUnmount(() => {
    clearToastTimer();
    stopConvertTaskPolling();
    stopLocalUploadTaskPolling();
    document.removeEventListener('pointerdown', handleGlobalPointerDown);
  });

  return {
    activeTab,
    toast,
    toastKind,
    toastNoticeKey,
    loading,
    taskProgress,
    closeTaskProgress,
    showMessage,
    showError,
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
    downloadDocuments,
    downloadDragActive,
    downloadItems,
    downloadTargetDir,
    downloadActive,
    downloadTotalCount,
    downloadSuccessCount,
    downloadFailedCount,
    canDownloadBatch,
    syncAccount,
    secretBackend,
    syncPassword,
    syncRemoteState,
    syncBusy,
    syncConnected,
    syncTokenInvalid,
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
    syncPatInput,
    passwordStatus,
    passwordBackend,
    lockSyncPassword,
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
    autoSyncDisplayChecked,
    autoSyncToggleEnabled,
    autoSyncToggleTitle,
    setAutoSyncEnabled,
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
    updateProgress,
    updateNotes,
    lastCheckAt,
    showUpToDate,
    autoCheckUpdate,
    setAutoCheckUpdate,
    checkUpdate,
    startUpdate,
    applyUpdate,
    openUpdatePackage,
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
    resetLocalUploadForm,
    clearWorkspaceDrafts,
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
    handleDownloadDocumentFiles,
    handleDownloadDocumentDrop,
    removeDownloadDocument,
    selectDownloadDir,
    analyzeDownloadDocuments,
    downloadAllImages,
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
  };
}
