<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { ChevronDown, Download, FileText, RefreshCw, Search, Trash2, X } from 'lucide-vue-next';
import DateRangePanel from '../DateRangePanel.vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

const { records, recordDetail, recordDetailOpen, typeLabel, showMessage, showError, loadRecords, openRecordDetail, closeRecordDetail, deleteRecords } = useWorkspaceContext();
const selectedRecordIds = ref<number[]>([]);
const deletingRecords = ref(false);
const recordSearch = ref('');
const statusFilter = ref('');
const sourceFilter = ref('');
const targetFilter = ref('');
const timeRangeFilter = ref('');
const appliedCustomRange = ref<{ start: string; end: string } | null>(null);
const showCustomRange = ref(false);
type RecordFilterName = 'status' | 'source' | 'target' | 'time';
const openFilterDropdown = ref<RecordFilterName | ''>('');
const selectedRecordCount = computed(() => selectedRecordIds.value.length);
const sourceOptions = computed(() => Array.from(new Set(records.value.map(record => record.source_picbed))));
const targetOptions = computed(() => Array.from(new Set(records.value.map(record => record.target_picbed))));

/// 时间范围过滤的起止毫秒边界；自定义范围含起止当日全天
function rangeBounds(range: string): [number | null, number | null] {
  if (range === 'today') {
    const now = new Date();
    return [new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime(), null];
  }
  const days = range === '7d' ? 7 : range === '30d' ? 30 : 0;
  if (days) return [Date.now() - days * 86400000, null];
  const custom = range === 'custom' ? appliedCustomRange.value : null;
  if (custom) {
    return [new Date(`${custom.start}T00:00:00`).getTime(), new Date(`${custom.end}T23:59:59.999`).getTime()];
  }
  return [null, null];
}

/// 组合过滤：三个下拉 + 时间范围 + 关键字全列匹配，实时生效
const filteredRecords = computed(() => {
  const keyword = recordSearch.value.trim().toLowerCase();
  const status = statusFilter.value;
  const source = sourceFilter.value;
  const target = targetFilter.value;
  const [rangeStart, rangeEnd] = rangeBounds(timeRangeFilter.value);
  return records.value.filter(record => {
    if (source && record.source_picbed !== source) return false;
    if (target && record.target_picbed !== target) return false;
    if (status === 'success' && record.status !== 'success') return false;
    if (status === 'failed' && record.status === 'success') return false;
    const createdAt = new Date(record.created_at).getTime();
    if (rangeStart !== null && createdAt < rangeStart) return false;
    if (rangeEnd !== null && createdAt > rangeEnd) return false;
    if (!keyword) return true;
    const haystack = [
      record.original_filename,
      typeLabel(record.source_picbed),
      typeLabel(record.target_picbed),
      record.status === 'success' ? t('records.statusSuccess') : t('records.statusFailed'),
      record.status,
      String(record.image_count),
      new Date(record.created_at).toLocaleString(),
      record.details?.map(detail => `${detail.original_url}\n${detail.target_url}`).join('\n') || '',
    ]
      .join('\n')
      .toLowerCase();
    return haystack.includes(keyword);
  });
});
const visibleRecordIds = computed(() => new Set(filteredRecords.value.map(record => record.id)));
const allRecordsSelected = computed(
  () => visibleRecordIds.value.size > 0 && filteredRecords.value.every(record => selectedRecordIds.value.includes(record.id))
);

const statusFilterLabel = computed(() =>
  statusFilter.value === 'success'
    ? t('records.statusSuccess')
    : statusFilter.value === 'failed'
      ? t('records.statusFailed')
      : t('records.allStatus')
);
const sourceFilterLabel = computed(() => (sourceFilter.value ? typeLabel(sourceFilter.value) : t('records.allSources')));
const targetFilterLabel = computed(() => (targetFilter.value ? typeLabel(targetFilter.value) : t('records.allTargets')));
const timeRangeLabel = computed(() => {
  if (timeRangeFilter.value === 'custom') {
    const range = appliedCustomRange.value;
    return range ? t('records.rangeTo', { start: range.start, end: range.end }) : t('records.custom');
  }
  return (
    {
      today: t('records.today'),
      '7d': t('records.last7Days'),
      '30d': t('records.last30Days'),
    })[timeRangeFilter.value] || t('records.allTime');
});
const recordCountHint = computed(() =>
  filteredRecords.value.length === records.value.length
    ? t('records.totalCount', { count: records.value.length })
    : t('records.filteredCount', { filtered: filteredRecords.value.length, total: records.value.length })
);

function toggleFilterDropdown(name: RecordFilterName) {
  openFilterDropdown.value = openFilterDropdown.value === name ? '' : name;
  if (openFilterDropdown.value) showCustomRange.value = false;
}
function closeFilterDropdowns() {
  openFilterDropdown.value = '';
}
function handleOutsidePointerDown(event: PointerEvent) {
  if (event.target instanceof Element && (event.target.closest('.record-filter') || event.target.closest('.date-range-panel'))) return;
  openFilterDropdown.value = '';
  showCustomRange.value = false;
}
function pickCustomRange() {
  closeFilterDropdowns();
  showCustomRange.value = true;
}
function applyCustomRange(range: { start: string; end: string }) {
  appliedCustomRange.value = range;
  timeRangeFilter.value = 'custom';
  showCustomRange.value = false;
}
function clearCustomRange() {
  appliedCustomRange.value = null;
  timeRangeFilter.value = '';
  showCustomRange.value = false;
}
onMounted(() => document.addEventListener('pointerdown', handleOutsidePointerDown));
onBeforeUnmount(() => document.removeEventListener('pointerdown', handleOutsidePointerDown));

watch(
  records,
  items => {
    const availableIds = new Set(items.map(item => item.id));
    selectedRecordIds.value = selectedRecordIds.value.filter(id => availableIds.has(id));
  },
  { deep: false }
);

function toggleRecordSelection(recordId: number, checked: boolean) {
  if (checked) {
    if (!selectedRecordIds.value.includes(recordId)) selectedRecordIds.value = [...selectedRecordIds.value, recordId];
    return;
  }
  selectedRecordIds.value = selectedRecordIds.value.filter(id => id !== recordId);
}

function handleRecordSelectionChange(recordId: number, event: Event) {
  const target = event.target;
  if (!(target instanceof HTMLInputElement)) return;
  toggleRecordSelection(recordId, target.checked);
}

function handleAllRecordsSelectionChange(event: Event) {
  const target = event.target;
  if (!(target instanceof HTMLInputElement)) return;
  if (target.checked) {
    selectedRecordIds.value = Array.from(new Set([...selectedRecordIds.value, ...visibleRecordIds.value]));
    return;
  }
  selectedRecordIds.value = selectedRecordIds.value.filter(id => !visibleRecordIds.value.has(id));
}

async function deleteSelectedRecords() {
  if (selectedRecordIds.value.length === 0 || deletingRecords.value) return;
  const ids = [...selectedRecordIds.value];
  deletingRecords.value = true;
  try {
    await deleteRecords(ids);
    selectedRecordIds.value = selectedRecordIds.value.filter(id => !ids.includes(id));
  } finally {
    deletingRecords.value = false;
  }
}

/// 保存记录的转换结果：WebView2 不支持 blob 下载，须走系统保存对话框 + write_text_file
async function downloadRecordContent() {
  const detail = recordDetail.value;
  if (!detail?.converted_content) return;
  const content = detail.converted_content;
  const targetPath = await save({
    defaultPath: detail.original_filename || 'converted.md',
    filters: [{ name: 'Markdown', extensions: ['md'] }],
  });
  if (!targetPath) return;
  try {
    await invoke('write_text_file', { path: targetPath, content });
    showMessage(t('records.convertedSaved'));
  } catch (err) {
    showError(err instanceof Error ? err.message : t('records.saveFailed'));
  }
}

const urlTooltip = ref({
  visible: false,
  text: '',
  x: 0,
  y: 0,
});

function updateUrlTooltipPosition(event: MouseEvent | FocusEvent) {
  const maxWidth = Math.min(560, Math.max(260, window.innerWidth - 32));
  const maxHeight = 220;
  let x = 16;
  let y = 16;

  if (event instanceof MouseEvent) {
    x = event.clientX + 14;
    y = event.clientY + 16;
    if (y + maxHeight > window.innerHeight - 16) y = event.clientY - maxHeight - 16;
  } else if (event.currentTarget instanceof HTMLElement) {
    const rect = event.currentTarget.getBoundingClientRect();
    x = rect.left;
    y = rect.bottom + 8;
    if (y + maxHeight > window.innerHeight - 16) y = rect.top - maxHeight - 8;
  }

  urlTooltip.value.x = Math.max(16, Math.min(x, window.innerWidth - maxWidth - 16));
  urlTooltip.value.y = Math.max(16, Math.min(y, window.innerHeight - 80));
}

function showUrlTooltip(text: string, event: MouseEvent | FocusEvent) {
  if (!text) return;
  urlTooltip.value.visible = true;
  urlTooltip.value.text = text;
  updateUrlTooltipPosition(event);
}

/// 系统默认浏览器打开目标地址（WebView 内 target=_blank 不生效）
async function openExternalUrl(url: string) {
  try {
    await openUrl(url);
  } catch (err) {
    const reason = err instanceof Error ? err.message : String(err || t('common.unknown'));
    showError(t('records.openBrowserFailed', { reason }));
  }
}

function hideUrlTooltip() {
  urlTooltip.value.visible = false;
}
</script>

<template>
<section class="panel stack">
  <div class="section-title">
    <div>
      <p class="section-kicker">Timeline</p>
      <h2>{{ t('records.title') }}<span class="record-count-hint">{{ recordCountHint }}</span></h2>
    </div>
    <div class="record-toolbar-actions">
      <div class="record-search">
        <Search :size="16" />
        <input v-model="recordSearch" type="search" :placeholder="t('records.searchPlaceholder')" :aria-label="t('records.searchPlaceholder')" />
      </div>
      <div class="record-filter status-filter custom-select" :class="{ open: openFilterDropdown === 'status' }">
        <button
          class="select-trigger"
          type="button"
          :aria-expanded="openFilterDropdown === 'status'"
          :aria-label="t('records.statusFilterAria')"
          @click="toggleFilterDropdown('status')"
        >
          <span>{{ statusFilterLabel }}</span>
          <ChevronDown :size="16" class="select-chevron" />
        </button>
        <div v-if="openFilterDropdown === 'status'" class="select-menu">
          <button class="select-option" :class="{ selected: !statusFilter }" type="button" @click="statusFilter = ''; closeFilterDropdowns()">{{ t('records.allStatus') }}</button>
          <button class="select-option" :class="{ selected: statusFilter === 'success' }" type="button" @click="statusFilter = 'success'; closeFilterDropdowns()">{{ t('records.statusSuccess') }}</button>
          <button class="select-option" :class="{ selected: statusFilter === 'failed' }" type="button" @click="statusFilter = 'failed'; closeFilterDropdowns()">{{ t('records.statusFailed') }}</button>
        </div>
      </div>
      <div class="record-filter custom-select" :class="{ open: openFilterDropdown === 'source' }">
        <button
          class="select-trigger"
          type="button"
          :aria-expanded="openFilterDropdown === 'source'"
          :aria-label="t('records.sourceFilterAria')"
          @click="toggleFilterDropdown('source')"
        >
          <span>{{ sourceFilterLabel }}</span>
          <ChevronDown :size="16" class="select-chevron" />
        </button>
        <div v-if="openFilterDropdown === 'source'" class="select-menu">
          <button class="select-option" :class="{ selected: !sourceFilter }" type="button" @click="sourceFilter = ''; closeFilterDropdowns()">{{ t('records.allSources') }}</button>
          <button
            v-for="item in sourceOptions"
            :key="item"
            class="select-option"
            :class="{ selected: sourceFilter === item }"
            type="button"
            @click="sourceFilter = item; closeFilterDropdowns()"
          >{{ typeLabel(item) }}</button>
        </div>
      </div>
      <div class="record-filter custom-select" :class="{ open: openFilterDropdown === 'target' }">
        <button
          class="select-trigger"
          type="button"
          :aria-expanded="openFilterDropdown === 'target'"
          :aria-label="t('records.targetFilterAria')"
          @click="toggleFilterDropdown('target')"
        >
          <span>{{ targetFilterLabel }}</span>
          <ChevronDown :size="16" class="select-chevron" />
        </button>
        <div v-if="openFilterDropdown === 'target'" class="select-menu">
          <button class="select-option" :class="{ selected: !targetFilter }" type="button" @click="targetFilter = ''; closeFilterDropdowns()">{{ t('records.allTargets') }}</button>
          <button
            v-for="item in targetOptions"
            :key="item"
            class="select-option"
            :class="{ selected: targetFilter === item }"
            type="button"
            @click="targetFilter = item; closeFilterDropdowns()"
          >{{ typeLabel(item) }}</button>
        </div>
      </div>
      <div class="record-filter time-filter custom-select" :class="{ open: openFilterDropdown === 'time' }">
        <button
          class="select-trigger"
          type="button"
          :aria-expanded="openFilterDropdown === 'time'"
          :aria-label="t('records.timeFilterAria')"
          @click="toggleFilterDropdown('time')"
        >
          <span>{{ timeRangeLabel }}</span>
          <ChevronDown :size="16" class="select-chevron" />
        </button>
        <div v-if="openFilterDropdown === 'time'" class="select-menu">
          <button class="select-option" :class="{ selected: !timeRangeFilter }" type="button" @click="timeRangeFilter = ''; closeFilterDropdowns()">{{ t('records.allTime') }}</button>
          <button class="select-option" :class="{ selected: timeRangeFilter === 'today' }" type="button" @click="timeRangeFilter = 'today'; closeFilterDropdowns()">{{ t('records.today') }}</button>
          <button class="select-option" :class="{ selected: timeRangeFilter === '7d' }" type="button" @click="timeRangeFilter = '7d'; closeFilterDropdowns()">{{ t('records.last7Days') }}</button>
          <button class="select-option" :class="{ selected: timeRangeFilter === '30d' }" type="button" @click="timeRangeFilter = '30d'; closeFilterDropdowns()">{{ t('records.last30Days') }}</button>
          <button class="select-option" :class="{ selected: timeRangeFilter === 'custom' }" type="button" @click="pickCustomRange">{{ t('records.custom') }}</button>
        </div>
        <DateRangePanel
          v-if="showCustomRange"
          :initial="appliedCustomRange"
          @apply="applyCustomRange"
          @clear="clearCustomRange"
        />
      </div>
      <button class="secondary" type="button" @click="loadRecords">
        <RefreshCw :size="18" />{{ t('records.refresh') }}
      </button>
      <button class="danger" type="button" :disabled="deletingRecords || selectedRecordCount === 0" @click="deleteSelectedRecords">
        <Trash2 :size="18" />{{ t('records.delete') }}<span v-if="selectedRecordCount"> {{ selectedRecordCount }}</span>
      </button>
    </div>
  </div>
  <div class="table-wrap records-table-wrap">
    <table>
      <thead>
        <tr>
          <th class="record-select-col">
            <label class="record-select-check" :aria-label="t('records.selectAllAria')">
              <input
                type="checkbox"
                :checked="allRecordsSelected"
                :disabled="filteredRecords.length === 0"
                @change="handleAllRecordsSelectionChange"
              />
            </label>
          </th>
          <th>{{ t('records.colFile') }}</th>
          <th>{{ t('records.colSource') }}</th>
          <th>{{ t('records.colTarget') }}</th>
          <th>{{ t('records.colStatus') }}</th>
          <th>{{ t('records.colImages') }}</th>
          <th>{{ t('records.colTime') }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="record in filteredRecords" :key="record.id" class="clickable-row" @click="openRecordDetail(record)">
          <td class="record-select-col" @click.stop>
            <label class="record-select-check" :aria-label="t('records.selectAria', { name: record.original_filename })">
              <input
                type="checkbox"
                :checked="selectedRecordIds.includes(record.id)"
                @change="handleRecordSelectionChange(record.id, $event)"
              />
            </label>
          </td>
          <td>{{ record.original_filename }}</td>
          <td>{{ typeLabel(record.source_picbed) }}</td>
          <td>{{ typeLabel(record.target_picbed) }}</td>
          <td>
            <span :class="['status', record.status]">{{
              record.status === 'success' ? t('records.statusSuccess') : t('records.statusFailed')
            }}</span>
          </td>
          <td>{{ record.image_count }}</td>
          <td>{{ new Date(record.created_at).toLocaleString() }}</td>
        </tr>
      </tbody>
    </table>
  </div>
  <p v-if="records.length === 0" class="empty">{{ t('records.empty') }}</p>
  <p v-else-if="filteredRecords.length === 0" class="empty">{{ t('records.noMatch', { keyword: recordSearch.trim() }) }}</p>
</section>

<div v-if="recordDetailOpen && recordDetail" class="modal-backdrop" role="presentation" @click.self="closeRecordDetail">
  <section class="confirm-dialog record-detail-dialog" role="dialog" aria-modal="true">
    <header class="record-detail-header">
      <div class="record-detail-title">
        <div class="dialog-icon record-detail-icon"><FileText :size="22" /></div>
        <div>
          <p class="section-kicker">Record</p>
          <h2>{{ recordDetail.original_filename }}</h2>
        </div>
      </div>
      <button class="ghost icon-only" type="button" :aria-label="t('records.detailCloseAria')" @click="closeRecordDetail"><X :size="18" /></button>
    </header>

    <div class="record-detail-summary">
      <div class="record-stat">
        <span>{{ t('records.colSource') }}</span>
        <strong>{{ typeLabel(recordDetail.source_picbed) }}</strong>
      </div>
      <div class="record-stat">
        <span>{{ t('records.colTarget') }}</span>
        <strong>{{ typeLabel(recordDetail.target_picbed) }}</strong>
      </div>
      <div class="record-stat">
        <span>{{ t('records.colStatus') }}</span>
        <strong><span :class="['status', recordDetail.status]">{{ recordDetail.status === 'success' ? t('records.statusSuccess') : t('records.statusFailed') }}</span></strong>
      </div>
      <div class="record-stat">
        <span>{{ t('records.colImages') }}</span>
        <strong>{{ recordDetail.image_count }}</strong>
      </div>
    </div>

    <p v-if="recordDetail.error_message" class="notice error record-detail-error">{{ recordDetail.error_message }}</p>

    <section class="record-detail-section">
      <div class="record-detail-section-head">
        <div>
          <p class="section-kicker">Images</p>
          <h3>{{ t('records.detailImages') }}</h3>
        </div>
        <div class="record-detail-head-actions">
          <span>{{ t('records.detailCount', { count: recordDetail.details?.length || 0 }) }}</span>
          <button class="secondary" type="button" :disabled="!recordDetail.converted_content" @click="downloadRecordContent">
            <Download :size="18" />{{ t('records.downloadConverted') }}
          </button>
        </div>
      </div>
      <div v-if="recordDetail.details?.length" class="record-detail-table-wrap">
        <table class="record-detail-table">
          <thead>
            <tr>
              <th>{{ t('records.colIndex') }}</th>
              <th>{{ t('records.colSourceUrl') }}</th>
              <th>{{ t('records.colTargetUrl') }}</th>
              <th>{{ t('records.colStatus') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(detail, index) in recordDetail.details" :key="detail.id">
              <td><span class="record-index">{{ index + 1 }}</span></td>
              <td>
                <code
                  class="url-preview"
                  tabindex="0"
                  :aria-label="detail.original_url"
                  @mouseenter="showUrlTooltip(detail.original_url, $event)"
                  @mousemove="updateUrlTooltipPosition($event)"
                  @mouseleave="hideUrlTooltip"
                  @focus="showUrlTooltip(detail.original_url, $event)"
                  @blur="hideUrlTooltip"
                >{{ detail.original_url }}</code>
              </td>
              <td>
                <a
                  v-if="detail.target_url"
                  class="url-preview"
                  :href="detail.target_url"
                  @click.prevent="openExternalUrl(detail.target_url)"
                  @mouseenter="showUrlTooltip(detail.target_url, $event)"
                  @mousemove="updateUrlTooltipPosition($event)"
                  @mouseleave="hideUrlTooltip"
                  @focus="showUrlTooltip(detail.target_url, $event)"
                  @blur="hideUrlTooltip"
                >
                  {{ detail.target_url }}
                </a>
                <span v-else>-</span>
              </td>
              <td><span :class="['status', detail.status]">{{ detail.status === 'success' ? t('records.statusSuccess') : t('records.statusFailed') }}</span></td>
            </tr>
          </tbody>
        </table>
      </div>
      <p v-else class="empty record-detail-empty">{{ t('records.detailEmpty') }}</p>
    </section>
  </section>
  <div
    v-if="urlTooltip.visible"
    class="url-tooltip"
    :style="{ left: `${urlTooltip.x}px`, top: `${urlTooltip.y}px` }"
    role="tooltip"
  >{{ urlTooltip.text }}</div>
</div>

</template>
