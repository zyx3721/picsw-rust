<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { ChevronDown, Download, FileText, RefreshCw, Search, Trash2, X } from 'lucide-vue-next';
import DateRangePanel from '../DateRangePanel.vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

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
      record.status === 'success' ? '成功' : '失败',
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

const statusFilterLabel = computed(() => (statusFilter.value === 'success' ? '成功' : statusFilter.value === 'failed' ? '失败' : '全部状态'));
const sourceFilterLabel = computed(() => (sourceFilter.value ? typeLabel(sourceFilter.value) : '全部源图床'));
const targetFilterLabel = computed(() => (targetFilter.value ? typeLabel(targetFilter.value) : '全部目标图床'));
const timeRangeLabel = computed(() => {
  if (timeRangeFilter.value === 'custom') {
    const range = appliedCustomRange.value;
    return range ? `${range.start} 至 ${range.end}` : '自定义';
  }
  return ({ today: '今天', '7d': '近 7 天', '30d': '近 30 天' })[timeRangeFilter.value] || '全部时间';
});
const recordCountHint = computed(() =>
  filteredRecords.value.length === records.value.length
    ? `共 ${records.value.length} 条`
    : `${filteredRecords.value.length} / ${records.value.length} 条`
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
    showMessage('转换结果已保存');
  } catch (err) {
    showError(err instanceof Error ? err.message : '保存转换结果失败');
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
    const reason = err instanceof Error ? err.message : String(err || '未知错误');
    showError(`打开浏览器失败：${reason}`);
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
      <h2>转换历史<span class="record-count-hint">{{ recordCountHint }}</span></h2>
    </div>
    <div class="record-toolbar-actions">
      <div class="record-search">
        <Search :size="16" />
        <input v-model="recordSearch" type="search" placeholder="搜索转换历史记录" aria-label="搜索转换历史记录" />
      </div>
      <div class="record-filter status-filter custom-select" :class="{ open: openFilterDropdown === 'status' }">
        <button
          class="select-trigger"
          type="button"
          :aria-expanded="openFilterDropdown === 'status'"
          aria-label="按状态筛选"
          @click="toggleFilterDropdown('status')"
        >
          <span>{{ statusFilterLabel }}</span>
          <ChevronDown :size="16" class="select-chevron" />
        </button>
        <div v-if="openFilterDropdown === 'status'" class="select-menu">
          <button class="select-option" :class="{ selected: !statusFilter }" type="button" @click="statusFilter = ''; closeFilterDropdowns()">全部状态</button>
          <button class="select-option" :class="{ selected: statusFilter === 'success' }" type="button" @click="statusFilter = 'success'; closeFilterDropdowns()">成功</button>
          <button class="select-option" :class="{ selected: statusFilter === 'failed' }" type="button" @click="statusFilter = 'failed'; closeFilterDropdowns()">失败</button>
        </div>
      </div>
      <div class="record-filter custom-select" :class="{ open: openFilterDropdown === 'source' }">
        <button
          class="select-trigger"
          type="button"
          :aria-expanded="openFilterDropdown === 'source'"
          aria-label="按源图床筛选"
          @click="toggleFilterDropdown('source')"
        >
          <span>{{ sourceFilterLabel }}</span>
          <ChevronDown :size="16" class="select-chevron" />
        </button>
        <div v-if="openFilterDropdown === 'source'" class="select-menu">
          <button class="select-option" :class="{ selected: !sourceFilter }" type="button" @click="sourceFilter = ''; closeFilterDropdowns()">全部源图床</button>
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
          aria-label="按目标图床筛选"
          @click="toggleFilterDropdown('target')"
        >
          <span>{{ targetFilterLabel }}</span>
          <ChevronDown :size="16" class="select-chevron" />
        </button>
        <div v-if="openFilterDropdown === 'target'" class="select-menu">
          <button class="select-option" :class="{ selected: !targetFilter }" type="button" @click="targetFilter = ''; closeFilterDropdowns()">全部目标图床</button>
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
          aria-label="按时间范围筛选"
          @click="toggleFilterDropdown('time')"
        >
          <span>{{ timeRangeLabel }}</span>
          <ChevronDown :size="16" class="select-chevron" />
        </button>
        <div v-if="openFilterDropdown === 'time'" class="select-menu">
          <button class="select-option" :class="{ selected: !timeRangeFilter }" type="button" @click="timeRangeFilter = ''; closeFilterDropdowns()">全部时间</button>
          <button class="select-option" :class="{ selected: timeRangeFilter === 'today' }" type="button" @click="timeRangeFilter = 'today'; closeFilterDropdowns()">今天</button>
          <button class="select-option" :class="{ selected: timeRangeFilter === '7d' }" type="button" @click="timeRangeFilter = '7d'; closeFilterDropdowns()">近 7 天</button>
          <button class="select-option" :class="{ selected: timeRangeFilter === '30d' }" type="button" @click="timeRangeFilter = '30d'; closeFilterDropdowns()">近 30 天</button>
          <button class="select-option" :class="{ selected: timeRangeFilter === 'custom' }" type="button" @click="pickCustomRange">自定义</button>
        </div>
        <DateRangePanel
          v-if="showCustomRange"
          :initial="appliedCustomRange"
          @apply="applyCustomRange"
          @clear="clearCustomRange"
        />
      </div>
      <button class="secondary" type="button" @click="loadRecords">
        <RefreshCw :size="18" />刷新
      </button>
      <button class="danger" type="button" :disabled="deletingRecords || selectedRecordCount === 0" @click="deleteSelectedRecords">
        <Trash2 :size="18" />删除<span v-if="selectedRecordCount"> {{ selectedRecordCount }}</span>
      </button>
    </div>
  </div>
  <div class="table-wrap records-table-wrap">
    <table>
      <thead>
        <tr>
          <th class="record-select-col">
            <label class="record-select-check" aria-label="全选历史记录">
              <input
                type="checkbox"
                :checked="allRecordsSelected"
                :disabled="filteredRecords.length === 0"
                @change="handleAllRecordsSelectionChange"
              />
            </label>
          </th>
          <th>文件</th>
          <th>源图床</th>
          <th>目标图床</th>
          <th>状态</th>
          <th>图片数</th>
          <th>时间</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="record in filteredRecords" :key="record.id" class="clickable-row" @click="openRecordDetail(record)">
          <td class="record-select-col" @click.stop>
            <label class="record-select-check" :aria-label="`选择 ${record.original_filename}`">
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
              record.status === 'success' ? '成功' : '失败'
            }}</span>
          </td>
          <td>{{ record.image_count }}</td>
          <td>{{ new Date(record.created_at).toLocaleString() }}</td>
        </tr>
      </tbody>
    </table>
  </div>
  <p v-if="records.length === 0" class="empty">暂无转换记录</p>
  <p v-else-if="filteredRecords.length === 0" class="empty">无匹配“{{ recordSearch.trim() }}”的记录</p>
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
      <button class="ghost icon-only" type="button" aria-label="关闭详情" @click="closeRecordDetail"><X :size="18" /></button>
    </header>

    <div class="record-detail-summary">
      <div class="record-stat">
        <span>源图床</span>
        <strong>{{ typeLabel(recordDetail.source_picbed) }}</strong>
      </div>
      <div class="record-stat">
        <span>目标图床</span>
        <strong>{{ typeLabel(recordDetail.target_picbed) }}</strong>
      </div>
      <div class="record-stat">
        <span>状态</span>
        <strong><span :class="['status', recordDetail.status]">{{ recordDetail.status === 'success' ? '成功' : '失败' }}</span></strong>
      </div>
      <div class="record-stat">
        <span>图片数</span>
        <strong>{{ recordDetail.image_count }}</strong>
      </div>
    </div>

    <p v-if="recordDetail.error_message" class="notice error record-detail-error">{{ recordDetail.error_message }}</p>

    <section class="record-detail-section">
      <div class="record-detail-section-head">
        <div>
          <p class="section-kicker">Images</p>
          <h3>替换明细</h3>
        </div>
        <div class="record-detail-head-actions">
          <span>{{ recordDetail.details?.length || 0 }} 条</span>
          <button class="secondary" type="button" :disabled="!recordDetail.converted_content" @click="downloadRecordContent">
            <Download :size="18" />下载转换结果
          </button>
        </div>
      </div>
      <div v-if="recordDetail.details?.length" class="record-detail-table-wrap">
        <table class="record-detail-table">
          <thead>
            <tr>
              <th>序号</th>
              <th>源地址</th>
              <th>目标地址</th>
              <th>状态</th>
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
              <td><span :class="['status', detail.status]">{{ detail.status === 'success' ? '成功' : '失败' }}</span></td>
            </tr>
          </tbody>
        </table>
      </div>
      <p v-else class="empty record-detail-empty">暂无替换明细</p>
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
