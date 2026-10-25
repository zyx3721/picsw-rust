<script setup lang="ts">
import { ChevronDown, Eye, EyeOff, KeyRound, PlugZap, Plus, RefreshCw, Trash2 } from 'lucide-vue-next';
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

const {
  loading,
  configForm,
  configErrors,
  configs,
  configTypeDropdownOpen,
  supportedTypes,
  selectedFields,
  typeLabel,
  typeDescription,
  fieldLabel,
  fieldPlaceholder,
  secretFieldVisible,
  toggleSecretField,
  selectConfigType,
  handleConfigTypeChange,
  resetConfigForm,
  saveConfig,
  testConfig,
  editConfig,
  setDefault,
  requestDeleteConfig,
} = useWorkspaceContext();

const minioSslDropdownOpen = ref(false);
const minioSslOptions = ['false', 'true'] as const;

function selectMinioUseSSL(value: 'false' | 'true') {
  configForm.values.use_ssl = value;
  delete configErrors.fields.use_ssl;
  minioSslDropdownOpen.value = false;
}

function closeMinioSslDropdown(event: PointerEvent) {
  const target = event.target;
  if (target instanceof Element && target.closest('.minio-ssl-select')) return;
  minioSslDropdownOpen.value = false;
}

onMounted(() => document.addEventListener('pointerdown', closeMinioSslDropdown));
onBeforeUnmount(() => document.removeEventListener('pointerdown', closeMinioSslDropdown));
</script>

<template>
<section class="grid two-cols">
  <form class="panel stack" @submit.prevent="saveConfig">
    <div class="section-title">
      <div>
        <p class="section-kicker">Credentials</p>
        <h2>{{ configForm.id ? t('config.editTitle') : t('config.addTitle') }}</h2>
      </div>
      <span class="secure-badge"><KeyRound :size="16" /> {{ t('config.typeValidated') }}</span>
    </div>
    <div class="field-row">
      <label class="select-field config-type-field">
        <span>{{ t('config.typeLabel') }}</span>
        <div class="custom-select" :class="{ open: configTypeDropdownOpen }">
          <button
            class="select-trigger"
            type="button"
            :aria-expanded="configTypeDropdownOpen"
            @click="configTypeDropdownOpen = !configTypeDropdownOpen"
          >
            <span>{{ typeLabel(configForm.picbed_type) }}</span>
            <ChevronDown :size="18" class="select-chevron" />
          </button>
          <div v-if="configTypeDropdownOpen" class="select-menu">
            <button
              v-for="item in supportedTypes"
              :key="item.value"
              class="select-option"
              type="button"
              :class="{ selected: item.value === configForm.picbed_type }"
              @click="selectConfigType(item.value)"
            >
              <span>{{ typeLabel(item.value) }}</span>
              <small>{{ typeDescription(item.value) }}</small>
            </button>
          </div>
        </div>
      </label>
      <label
        >{{ t('config.typeLabel') }}<select v-model="configForm.picbed_type" @change="handleConfigTypeChange">
          <option v-for="item in supportedTypes" :key="item.value" :value="item.value">
            {{ typeLabel(item.value) }}
          </option>
        </select></label
      ><label :class="{ invalid: configErrors.config_name }"
        ><span class="field-label-text">{{ t('config.nameLabel') }}</span
        ><input
          v-model.trim="configForm.config_name"
          :class="{ invalid: configErrors.config_name }"
          :placeholder="t('config.namePlaceholder')"
          @input="configErrors.config_name = ''"
      /></label>
    </div>
    <div class="dynamic-fields">
      <label
        v-for="field in selectedFields"
        :key="field.key"              :class="{ required: field.required, invalid: configErrors.fields[field.key] }"
        ><span class="field-label-text">{{ fieldLabel(field) }}</span>
        <div class="secret-input-wrap" :class="{ invalid: configErrors.fields[field.key] }">
          <div
            v-if="configForm.picbed_type === 'minio' && field.key === 'use_ssl'"
            class="custom-select minio-ssl-select"
            :class="{ open: minioSslDropdownOpen, invalid: configErrors.fields[field.key] }"
          >
            <button
              class="select-trigger"
              type="button"
              :aria-expanded="minioSslDropdownOpen"
              @click="minioSslDropdownOpen = !minioSslDropdownOpen"
            >
              <span>{{ configForm.values[field.key] || 'false' }}</span>
              <ChevronDown :size="18" class="select-chevron" />
            </button>
            <div v-if="minioSslDropdownOpen" class="select-menu">
              <button
                v-for="option in minioSslOptions"
                :key="option"
                class="select-option"
                type="button"
                :class="{ selected: (configForm.values[field.key] || 'false') === option }"
                @click="selectMinioUseSSL(option)"
              >
                <span>{{ option }}</span>
              </button>
            </div>
          </div>
          <input
            v-else
            v-model.trim="configForm.values[field.key]"
            :class="{ invalid: configErrors.fields[field.key] }"
            :type="field.secret && !secretFieldVisible(field.key) ? 'password' : 'text'"
            :placeholder="fieldPlaceholder(field)"
            autocomplete="off"
            @input="delete configErrors.fields[field.key]"
          />
          <button
            v-if="field.secret"
            class="secret-toggle"
            type="button"
            :aria-label="secretFieldVisible(field.key) ? t('config.hideSecretAria') : t('config.showSecretAria')"
            @click="toggleSecretField(field.key)"
          >
            <EyeOff v-if="secretFieldVisible(field.key)" :size="18" />
            <Eye v-else :size="18" />
          </button>
        </div></label>
    </div>
    <label class="checkbox"
      ><input v-model="configForm.is_default" type="checkbox" />{{ t('config.setAsDefault') }}</label
    >
    <div class="actions">
      <button class="secondary" type="button" @click="resetConfigForm">
        <RefreshCw :size="18" />{{ t('common.clear') }}</button
      ><button class="secondary" type="button" :disabled="loading" @click="testConfig()">
        <PlugZap :size="18" />{{ t('config.test') }}</button
      ><button class="primary" :disabled="loading" type="submit">
        <Plus :size="18" />{{ t('config.save') }}
      </button>
    </div>
  </form>
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">Saved</p>
        <h2>{{ t('config.savedTitle') }}</h2>
      </div>
      <span class="count-badge">{{ configs.length }}</span>
    </div>
    <div class="config-list">
      <div
        v-for="item in configs"
        :key="item.id"
        class="config-row"
        :data-provider="item.picbed_type"
      >
        <div>
          <strong>{{ item.config_name }}</strong
          ><span
            >{{ typeLabel(item.picbed_type) }}{{ item.is_default ? t('common.defaultSuffix') : '' }}</span
          >
        </div>
        <div class="row-actions">
          <button class="ghost" type="button" @click="editConfig(item)">{{ t('config.edit') }}</button
          ><button class="ghost" type="button" :disabled="loading" @click="testConfig(item)">{{ t('config.testShort') }}</button
          ><button class="ghost" type="button" @click="setDefault(item.id)">{{ t('config.defaultShort') }}</button
          ><button
            class="danger icon-only"
            type="button"
            :aria-label="t('config.deleteAria')"
            @click="requestDeleteConfig(item)"
          >
            <Trash2 :size="17" />
          </button>
        </div>
      </div>
      <p v-if="configs.length === 0" class="empty">{{ t('config.empty') }}</p>
    </div>
  </div>
</section>
</template>
