<script setup lang="ts">
import { ArrowLeftIcon, ChatBubbleLeftRightIcon } from '@heroicons/vue/24/outline'
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import { getModelDetails } from '../api'
import { useChatStore } from '../stores/chat'
import { useModelStore } from '../stores/models'
import type { ModelEntry } from '../types'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const models = useModelStore()
const chat = useChatStore()

const modelId = computed(() => decodeURIComponent(route.params.id as string))
const detail = ref<ModelEntry | null>(null)
const loading = ref(true)

const loadCtxSize = ref(4096)
const loadGpuLayers = ref(-1)

onMounted(async () => {
  try {
    const data = await getModelDetails(modelId.value)
    detail.value = data
  } catch {
    // fallback to list
    const m = models.models.find((m) => m.id === modelId.value)
    if (m) detail.value = m
  } finally {
    loading.value = false
  }
})

const model = computed(() => {
  // prefer live store data for status updates
  const storeModel = models.models.find((m) => m.id === modelId.value)
  if (storeModel && detail.value) {
    return { ...detail.value, status: storeModel.status }
  }
  return detail.value
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

async function handleLoad() {
  await models.loadModel(modelId.value, {
    ctx_size: loadCtxSize.value,
    n_gpu_layers: loadGpuLayers.value,
  })
}

function openChat() {
  chat.createConversation(modelId.value)
  router.push('/chat')
}
</script>

<template>
  <div class="p-4 lg:p-6 space-y-6">
    <!-- Back link -->
    <button class="btn btn-ghost btn-sm gap-2" @click="router.push('/models')">
      <ArrowLeftIcon class="w-4 h-4" />
      {{ t('modelDetail.backToModels') }}
    </button>

    <!-- Loading -->
    <div v-if="loading" class="flex justify-center py-16">
      <span class="loading loading-spinner loading-lg text-primary" />
    </div>

    <template v-else-if="model">
      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <!-- Model info (left 2 cols) -->
        <div class="lg:col-span-2 space-y-4">
          <div class="card bg-base-200 shadow-sm">
            <div class="card-body">
              <h2 class="card-title">{{ t('modelDetail.modelInfo') }}</h2>
              <div class="overflow-x-auto">
                <table class="table">
                  <tbody>
                    <tr>
                      <th class="w-40">{{ t('modelDetail.id') }}</th>
                      <td class="font-mono text-sm">{{ model.id }}</td>
                    </tr>
                    <tr>
                      <th>{{ t('modelDetail.architecture') }}</th>
                      <td>
                        <span class="badge badge-ghost">{{
                          model.architecture || t('modelDetail.unknown')
                        }}</span>
                      </td>
                    </tr>
                    <tr>
                      <th>{{ t('modelDetail.quantization') }}</th>
                      <td>
                        <span class="badge badge-ghost">{{
                          model.quantization || t('modelDetail.unknown')
                        }}</span>
                      </td>
                    </tr>
                    <tr>
                      <th>{{ t('modelDetail.fileSize') }}</th>
                      <td>{{ formatSize(model.size) }}</td>
                    </tr>
                    <tr>
                      <th>{{ t('modelDetail.contextLength') }}</th>
                      <td>{{ model.context_length?.toLocaleString() || '—' }}</td>
                    </tr>
                    <tr v-if="model.parameters">
                      <th>{{ t('modelDetail.parameters') }}</th>
                      <td>{{ model.parameters }}</td>
                    </tr>
                    <tr>
                      <th>{{ t('modelDetail.status') }}</th>
                      <td>
                        <span :class="['badge badge-sm', statusBadge(model.status)]">
                          {{ model.status }}
                        </span>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </div>

          <!-- Chat Template -->
          <div v-if="model.chat_template" class="card bg-base-200 shadow-sm">
            <div class="card-body">
              <h2 class="card-title text-base">{{ t('modelDetail.chatTemplate') }}</h2>
              <div class="mockup-code max-h-48 overflow-y-auto custom-scrollbar text-xs">
                <pre><code>{{ model.chat_template }}</code></pre>
              </div>
            </div>
          </div>
        </div>

        <!-- Actions (right col) -->
        <div class="space-y-4">
          <div class="card bg-base-200 shadow-sm">
            <div class="card-body">
              <h2 class="card-title text-base">{{ t('modelDetail.actions') }}</h2>

              <div v-if="model.status === 'loaded'" class="space-y-3">
                <button class="btn btn-primary w-full gap-2" @click="openChat">
                  <ChatBubbleLeftRightIcon class="w-5 h-5" />
                  {{ t('modelDetail.openChat') }}
                </button>
                <button
                  class="btn btn-error btn-outline w-full"
                  @click="models.unloadModel(model.id)"
                >
                  {{ t('modelDetail.unloadModel') }}
                </button>
              </div>

              <div v-else class="space-y-3">
                <div class="form-control">
                  <label class="label">
                    <span class="label-text">{{ t('modelDetail.contextSize') }}</span>
                  </label>
                  <input
                    v-model.number="loadCtxSize"
                    type="number"
                    class="input input-bordered input-sm"
                    min="512"
                    max="131072"
                    step="512"
                  />
                </div>

                <div class="form-control">
                  <label class="label">
                    <span class="label-text">{{ t('modelDetail.gpuLayers') }}</span>
                  </label>
                  <input
                    v-model.number="loadGpuLayers"
                    type="number"
                    class="input input-bordered input-sm"
                    min="-1"
                    max="999"
                  />
                </div>

                <button
                  class="btn btn-primary w-full"
                  :disabled="model.status === 'loading'"
                  @click="handleLoad"
                >
                  <span
                    v-if="model.status === 'loading'"
                    class="loading loading-spinner loading-sm"
                  />
                  {{ t('modelDetail.loadModel') }}
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>

    <!-- Not found -->
    <div v-else class="text-center py-16 text-base-content/50">
      <p>Model not found.</p>
    </div>
  </div>
</template>
