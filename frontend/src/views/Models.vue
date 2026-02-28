<script setup lang="ts">
import {
  ArrowPathIcon,
  CpuChipIcon,
  MagnifyingGlassIcon,
  Squares2X2Icon,
  StarIcon,
  TableCellsIcon,
} from '@heroicons/vue/24/outline'
import { StarIcon as StarSolidIcon } from '@heroicons/vue/24/solid'
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useChatStore } from '../stores/chat'
import { useModelStore } from '../stores/models'

const { t } = useI18n()
const router = useRouter()
const models = useModelStore()
const chat = useChatStore()

const searchQuery = ref('')
const statusFilter = ref('all')
const viewMode = ref<'grid' | 'table'>('grid')

const showLoadDialog = ref(false)
const loadTarget = ref<string | null>(null)
const loadCtxSize = ref(4096)
const loadGpuLayers = ref(-1)

onMounted(() => {
  models.fetchModels()
})

const filteredModels = computed(() => {
  let list = models.models
  if (searchQuery.value) {
    const q = searchQuery.value.toLowerCase()
    list = list.filter(
      (m) =>
        m.id.toLowerCase().includes(q) ||
        (m.architecture && m.architecture.toLowerCase().includes(q)) ||
        (m.quantization && m.quantization.toLowerCase().includes(q)),
    )
  }
  if (statusFilter.value !== 'all') {
    list = list.filter((m) => m.status === statusFilter.value)
  }
  return list
})

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB'
  return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB'
}

function statusBadge(status: string) {
  switch (status) {
    case 'loaded':
      return 'badge-success'
    case 'loading':
      return 'badge-warning'
    case 'error':
      return 'badge-error'
    default:
      return 'badge-ghost'
  }
}

function statusLabel(status: string) {
  switch (status) {
    case 'loaded':
      return t('models.loaded')
    case 'loading':
      return t('models.loadingStatus')
    case 'error':
      return t('models.errorStatus')
    default:
      return t('models.unloaded')
  }
}

function openLoadDialog(id: string) {
  loadTarget.value = id
  loadCtxSize.value = 4096
  loadGpuLayers.value = -1
  showLoadDialog.value = true
}

async function confirmLoad() {
  if (!loadTarget.value) return
  showLoadDialog.value = false
  await models.loadModel(loadTarget.value, {
    ctx_size: loadCtxSize.value,
    n_gpu_layers: loadGpuLayers.value,
  })
}

function openChat(modelId: string) {
  chat.createConversation(modelId)
  router.push('/chat')
}
</script>

<template>
  <div class="p-4 lg:p-6 space-y-6">
    <!-- Page header -->
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
      <div>
        <h1 class="text-2xl font-bold text-base-content">{{ t('models.title') }}</h1>
        <p class="text-base-content/60 text-sm mt-1">{{ t('models.subtitle') }}</p>
      </div>
      <button class="btn btn-primary btn-sm gap-2" @click="models.rescan()">
        <ArrowPathIcon class="w-4 h-4" />
        {{ t('models.rescan') }}
      </button>
    </div>

    <!-- Filter bar -->
    <div class="flex flex-col sm:flex-row gap-3">
      <!-- Search -->
      <label class="input input-bordered flex items-center gap-2 flex-1">
        <MagnifyingGlassIcon class="w-4 h-4 opacity-50" />
        <input
          v-model="searchQuery"
          type="text"
          class="grow"
          :placeholder="t('models.searchPlaceholder')"
        />
      </label>

      <!-- Status filter -->
      <select v-model="statusFilter" class="select select-bordered w-full sm:w-40">
        <option value="all">{{ t('models.allStatus') }}</option>
        <option value="loaded">{{ t('models.loaded') }}</option>
        <option value="unloaded">{{ t('models.unloaded') }}</option>
        <option value="loading">{{ t('models.loadingStatus') }}</option>
        <option value="error">{{ t('models.errorStatus') }}</option>
      </select>

      <!-- View mode toggle -->
      <div class="join">
        <button
          :class="['btn btn-sm join-item', viewMode === 'grid' && 'btn-active']"
          :title="t('models.gridView')"
          @click="viewMode = 'grid'"
        >
          <Squares2X2Icon class="w-4 h-4" />
        </button>
        <button
          :class="['btn btn-sm join-item', viewMode === 'table' && 'btn-active']"
          :title="t('models.tableView')"
          @click="viewMode = 'table'"
        >
          <TableCellsIcon class="w-4 h-4" />
        </button>
      </div>
    </div>

    <!-- Loading spinner -->
    <div v-if="models.loading" class="flex justify-center py-12">
      <span class="loading loading-spinner loading-lg text-primary" />
    </div>

    <!-- Empty state -->
    <div v-else-if="filteredModels.length === 0" class="text-center py-16 text-base-content/50">
      <CpuChipIcon class="w-12 h-12 mx-auto mb-3 opacity-30" />
      <p>
        {{ t('models.emptyState', { link: '' }) }}
        <router-link to="/settings" class="link link-primary">{{
          t('models.settingsLink')
        }}</router-link>
      </p>
    </div>

    <!-- Grid view -->
    <div
      v-else-if="viewMode === 'grid'"
      class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4"
    >
      <div
        v-for="model in filteredModels"
        :key="model.id"
        class="card bg-base-200 shadow-sm hover:shadow-md transition-shadow cursor-pointer"
        @click="router.push(`/models/${encodeURIComponent(model.id)}`)"
      >
        <div class="card-body p-4 gap-3">
          <!-- Header -->
          <div class="flex items-start justify-between">
            <h3 class="card-title text-sm font-semibold line-clamp-1">{{ model.id }}</h3>
            <button
              class="btn btn-ghost btn-xs btn-square"
              @click.stop="models.toggleFavorite(model.id)"
            >
              <StarSolidIcon v-if="model.favorite" class="w-4 h-4 text-warning" />
              <StarIcon v-else class="w-4 h-4 opacity-40" />
            </button>
          </div>

          <!-- Info badges -->
          <div class="flex flex-wrap gap-1.5">
            <span class="badge badge-ghost badge-sm">{{ model.architecture || '—' }}</span>
            <span class="badge badge-ghost badge-sm">{{ model.quantization || '—' }}</span>
            <span class="badge badge-ghost badge-sm">{{ formatSize(model.size) }}</span>
          </div>

          <!-- Status + actions -->
          <div class="flex items-center justify-between mt-1">
            <span :class="['badge badge-sm', statusBadge(model.status)]">
              {{ statusLabel(model.status) }}
            </span>
            <div class="flex gap-1" @click.stop>
              <button
                v-if="model.status === 'loaded'"
                class="btn btn-error btn-xs"
                @click="models.unloadModel(model.id)"
              >
                {{ t('models.unload') }}
              </button>
              <button
                v-if="model.status === 'loaded'"
                class="btn btn-primary btn-xs"
                @click="openChat(model.id)"
              >
                {{ t('models.chat') }}
              </button>
              <button
                v-if="model.status === 'unloaded'"
                class="btn btn-primary btn-xs"
                @click="openLoadDialog(model.id)"
              >
                {{ t('models.load') }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Table view -->
    <div v-else class="overflow-x-auto bg-base-200 rounded-xl">
      <table class="table table-zebra">
        <thead>
          <tr>
            <th />
            <th>{{ t('models.name') }}</th>
            <th>{{ t('models.architecture') }}</th>
            <th>{{ t('models.quantization') }}</th>
            <th>{{ t('models.size') }}</th>
            <th>{{ t('models.context') }}</th>
            <th>{{ t('models.status') }}</th>
            <th>{{ t('models.actions') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="model in filteredModels"
            :key="model.id"
            class="hover cursor-pointer"
            @click="router.push(`/models/${encodeURIComponent(model.id)}`)"
          >
            <td @click.stop>
              <button
                class="btn btn-ghost btn-xs btn-square"
                @click="models.toggleFavorite(model.id)"
              >
                <StarSolidIcon v-if="model.favorite" class="w-4 h-4 text-warning" />
                <StarIcon v-else class="w-4 h-4 opacity-40" />
              </button>
            </td>
            <td class="font-medium">{{ model.id }}</td>
            <td>
              <span class="badge badge-ghost badge-sm">{{ model.architecture || '—' }}</span>
            </td>
            <td>
              <span class="badge badge-ghost badge-sm">{{ model.quantization || '—' }}</span>
            </td>
            <td>{{ formatSize(model.size) }}</td>
            <td>{{ model.context_length?.toLocaleString() || '—' }}</td>
            <td>
              <span :class="['badge badge-sm', statusBadge(model.status)]">
                {{ statusLabel(model.status) }}
              </span>
            </td>
            <td @click.stop>
              <div class="flex gap-1">
                <button
                  v-if="model.status === 'loaded'"
                  class="btn btn-error btn-xs"
                  @click="models.unloadModel(model.id)"
                >
                  {{ t('models.unload') }}
                </button>
                <button
                  v-if="model.status === 'loaded'"
                  class="btn btn-primary btn-xs"
                  @click="openChat(model.id)"
                >
                  {{ t('models.chat') }}
                </button>
                <button
                  v-if="model.status === 'unloaded'"
                  class="btn btn-primary btn-xs"
                  @click="openLoadDialog(model.id)"
                >
                  {{ t('models.load') }}
                </button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Load Model Dialog -->
    <dialog :class="['modal', showLoadDialog && 'modal-open']">
      <div class="modal-box">
        <h3 class="font-bold text-lg">{{ t('models.loadModel') }}</h3>
        <p class="text-sm text-base-content/60 mt-1">{{ loadTarget }}</p>

        <div class="form-control mt-4">
          <label class="label">
            <span class="label-text">{{ t('models.contextSize') }}</span>
          </label>
          <input
            v-model.number="loadCtxSize"
            type="number"
            class="input input-bordered"
            min="512"
            max="131072"
            step="512"
          />
        </div>

        <div class="form-control mt-3">
          <label class="label">
            <span class="label-text">{{ t('models.gpuLayers') }}</span>
          </label>
          <input
            v-model.number="loadGpuLayers"
            type="number"
            class="input input-bordered"
            min="-1"
            max="999"
          />
        </div>

        <div class="modal-action">
          <button class="btn btn-ghost" @click="showLoadDialog = false">
            {{ t('common.cancel') }}
          </button>
          <button class="btn btn-primary" @click="confirmLoad">
            {{ t('models.load') }}
          </button>
        </div>
      </div>
      <div class="modal-backdrop" @click="showLoadDialog = false" />
    </dialog>
  </div>
</template>
