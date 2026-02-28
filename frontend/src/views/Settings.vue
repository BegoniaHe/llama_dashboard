<script setup lang="ts">
import {
  CheckCircleIcon,
  ExclamationCircleIcon,
  FolderIcon,
  PlusIcon,
  TrashIcon,
} from '@heroicons/vue/24/outline'
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { getConfig, updateConfig } from '../api'
import { useAppStore } from '../stores/app'

const { t } = useI18n()
const app = useAppStore()

const modelDirs = ref<string[]>([])
const newDir = ref('')
const ctxSize = ref(4096)
const gpuLayers = ref(-1)
const temperature = ref(0.8)
const apiKey = ref('')

const saving = ref(false)
const saveMessage = ref('')
const saveError = ref(false)

onMounted(async () => {
  try {
    const config = await getConfig()
    modelDirs.value = config.model_dirs || []
    ctxSize.value = config.default_ctx_size || 4096
    gpuLayers.value = config.default_n_gpu_layers ?? -1
    temperature.value = config.default_temperature || 0.8
    apiKey.value = config.api_key || ''
  } catch {
    // use defaults
  }
})

function addDir() {
  const dir = newDir.value.trim()
  if (dir && !modelDirs.value.includes(dir)) {
    modelDirs.value.push(dir)
    newDir.value = ''
  }
}

function removeDir(idx: number) {
  modelDirs.value.splice(idx, 1)
}

async function saveSettings() {
  saving.value = true
  saveMessage.value = ''
  saveError.value = false
  try {
    await updateConfig({
      model_dirs: modelDirs.value,
      default_ctx_size: ctxSize.value,
      default_n_gpu_layers: gpuLayers.value,
      default_temperature: temperature.value,
      api_key: apiKey.value || undefined,
    })
    saveMessage.value = t('settings.saved')
    saveError.value = false
    setTimeout(() => {
      saveMessage.value = ''
    }, 3000)
  } catch {
    saveMessage.value = t('settings.saveFailed')
    saveError.value = true
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div class="p-4 lg:p-6 space-y-6 max-w-3xl">
    <!-- Page header -->
    <div>
      <h1 class="text-2xl font-bold text-base-content">{{ t('settings.title') }}</h1>
      <p class="text-base-content/60 text-sm mt-1">{{ t('settings.subtitle') }}</p>
    </div>

    <!-- Model Directories -->
    <div class="card bg-base-200 shadow-sm">
      <div class="card-body">
        <h2 class="card-title text-base gap-2">
          <FolderIcon class="w-5 h-5" />
          {{ t('settings.modelDirs') }}
        </h2>

        <!-- Directory list -->
        <div v-if="modelDirs.length > 0" class="space-y-2 mt-2">
          <div
            v-for="(dir, idx) in modelDirs"
            :key="idx"
            class="flex items-center gap-2 bg-base-300 rounded-lg px-3 py-2"
          >
            <span class="flex-1 font-mono text-sm truncate">{{ dir }}</span>
            <button class="btn btn-ghost btn-xs btn-square text-error" @click="removeDir(idx)">
              <TrashIcon class="w-4 h-4" />
            </button>
          </div>
        </div>

        <!-- Add directory -->
        <div class="flex gap-2 mt-2">
          <input
            v-model="newDir"
            type="text"
            class="input input-bordered input-sm flex-1"
            :placeholder="t('settings.addDirPlaceholder')"
            @keydown.enter="addDir"
          />
          <button class="btn btn-primary btn-sm gap-1" @click="addDir">
            <PlusIcon class="w-4 h-4" />
            {{ t('settings.addDir') }}
          </button>
        </div>
      </div>
    </div>

    <!-- Default Inference Parameters -->
    <div class="card bg-base-200 shadow-sm">
      <div class="card-body">
        <h2 class="card-title text-base">{{ t('settings.defaultParams') }}</h2>

        <div class="grid grid-cols-1 sm:grid-cols-3 gap-4 mt-2">
          <div class="form-control">
            <label class="label">
              <span class="label-text">{{ t('settings.ctxSize') }}</span>
            </label>
            <input
              v-model.number="ctxSize"
              type="number"
              class="input input-bordered input-sm"
              min="512"
              max="131072"
              step="512"
            />
          </div>

          <div class="form-control">
            <label class="label">
              <span class="label-text">{{ t('settings.gpuLayers') }}</span>
            </label>
            <input
              v-model.number="gpuLayers"
              type="number"
              class="input input-bordered input-sm"
              min="-1"
              max="999"
            />
          </div>

          <div class="form-control">
            <label class="label">
              <span class="label-text">{{ t('settings.temperature') }}</span>
            </label>
            <input
              v-model.number="temperature"
              type="number"
              class="input input-bordered input-sm"
              min="0"
              max="2"
              step="0.1"
            />
          </div>
        </div>
      </div>
    </div>

    <!-- API Key -->
    <div class="card bg-base-200 shadow-sm">
      <div class="card-body">
        <h2 class="card-title text-base">{{ t('settings.apiKey') }}</h2>
        <div class="form-control mt-2">
          <input
            v-model="apiKey"
            type="password"
            class="input input-bordered input-sm"
            :placeholder="t('settings.apiKeyPlaceholder')"
          />
        </div>
      </div>
    </div>

    <!-- Appearance -->
    <div class="card bg-base-200 shadow-sm">
      <div class="card-body">
        <h2 class="card-title text-base">{{ t('settings.appearance') }}</h2>

        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 mt-2">
          <!-- Theme -->
          <div class="form-control">
            <label class="label">
              <span class="label-text">{{ t('settings.theme') }}</span>
            </label>
            <label class="label cursor-pointer justify-start gap-3">
              <span class="text-sm">{{ t('settings.light') }}</span>
              <input
                type="checkbox"
                class="toggle toggle-primary"
                :checked="app.theme === 'dark'"
                @change="app.toggleTheme()"
              />
              <span class="text-sm">{{ t('settings.dark') }}</span>
            </label>
          </div>

          <!-- Language -->
          <div class="form-control">
            <label class="label">
              <span class="label-text">{{ t('settings.language') }}</span>
            </label>
            <select
              :value="app.locale"
              class="select select-bordered select-sm"
              @change="app.setLocale(($event.target as HTMLSelectElement).value as 'en' | 'zh-CN')"
            >
              <option value="en">English</option>
              <option value="zh-CN">中文</option>
            </select>
          </div>
        </div>
      </div>
    </div>

    <!-- Save button -->
    <div class="flex items-center gap-3">
      <button class="btn btn-primary" :disabled="saving" @click="saveSettings">
        <span v-if="saving" class="loading loading-spinner loading-sm" />
        {{ t('settings.saveConfig') }}
      </button>

      <div v-if="saveMessage" class="flex items-center gap-1 text-sm">
        <CheckCircleIcon v-if="!saveError" class="w-4 h-4 text-success" />
        <ExclamationCircleIcon v-else class="w-4 h-4 text-error" />
        <span :class="saveError ? 'text-error' : 'text-success'">{{ saveMessage }}</span>
      </div>
    </div>
  </div>
</template>
