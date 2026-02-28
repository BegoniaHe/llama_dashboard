<script setup lang="ts">
import {
  ArrowPathIcon,
  ChatBubbleLeftRightIcon,
  CircleStackIcon,
  Cog6ToothIcon,
  CpuChipIcon,
  ServerIcon,
} from '@heroicons/vue/24/outline'
import { computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useChatStore } from '../stores/chat'
import { useModelStore } from '../stores/models'

const { t } = useI18n()
const router = useRouter()
const models = useModelStore()
const app = useAppStore()
const chat = useChatStore()

onMounted(() => {
  models.fetchModels()
})

const loadedCount = computed(() => models.loadedModels.length)
const availableCount = computed(() => models.models.length)
const conversationCount = computed(() => chat.conversations.length)

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB'
  return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB'
}
</script>

<template>
  <div class="p-4 lg:p-6 space-y-6">
    <!-- Page header -->
    <div>
      <h1 class="text-2xl font-bold text-base-content">{{ t('dashboard.title') }}</h1>
      <p class="text-base-content/60 text-sm mt-1">{{ t('dashboard.subtitle') }}</p>
    </div>

    <!-- Stats grid -->
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
      <!-- Models Loaded -->
      <div class="stat bg-base-200 rounded-xl shadow-sm">
        <div class="stat-figure text-primary">
          <CpuChipIcon class="w-8 h-8" />
        </div>
        <div class="stat-title">{{ t('dashboard.modelsLoaded') }}</div>
        <div class="stat-value text-primary">{{ loadedCount }}</div>
      </div>

      <!-- Models Available -->
      <div class="stat bg-base-200 rounded-xl shadow-sm">
        <div class="stat-figure text-secondary">
          <CircleStackIcon class="w-8 h-8" />
        </div>
        <div class="stat-title">{{ t('dashboard.modelsAvailable') }}</div>
        <div class="stat-value text-secondary">{{ availableCount }}</div>
      </div>

      <!-- Server Status -->
      <div class="stat bg-base-200 rounded-xl shadow-sm">
        <div class="stat-figure" :class="app.serverOnline ? 'text-success' : 'text-error'">
          <ServerIcon class="w-8 h-8" />
        </div>
        <div class="stat-title">{{ t('dashboard.serverStatus') }}</div>
        <div class="stat-value text-lg" :class="app.serverOnline ? 'text-success' : 'text-error'">
          {{ app.serverOnline ? t('common.online') : t('common.offline') }}
        </div>
      </div>

      <!-- Conversations -->
      <div class="stat bg-base-200 rounded-xl shadow-sm">
        <div class="stat-figure text-accent">
          <ChatBubbleLeftRightIcon class="w-8 h-8" />
        </div>
        <div class="stat-title">{{ t('dashboard.conversations') }}</div>
        <div class="stat-value text-accent">{{ conversationCount }}</div>
      </div>
    </div>

    <!-- Quick Actions -->
    <div>
      <h2 class="text-lg font-semibold text-base-content mb-3">
        {{ t('dashboard.quickActions') }}
      </h2>
      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
        <button
          class="btn btn-outline btn-primary justify-start gap-3 h-auto py-3"
          @click="router.push('/chat')"
        >
          <ChatBubbleLeftRightIcon class="w-5 h-5" />
          <div class="text-left">
            <div class="font-medium">{{ t('dashboard.startChat') }}</div>
            <div class="text-xs opacity-60">{{ t('dashboard.startChatDesc') }}</div>
          </div>
        </button>

        <button
          class="btn btn-outline btn-secondary justify-start gap-3 h-auto py-3"
          @click="router.push('/models')"
        >
          <CpuChipIcon class="w-5 h-5" />
          <div class="text-left">
            <div class="font-medium">{{ t('dashboard.manageModels') }}</div>
            <div class="text-xs opacity-60">{{ t('dashboard.manageModelsDesc') }}</div>
          </div>
        </button>

        <button class="btn btn-outline justify-start gap-3 h-auto py-3" @click="models.rescan()">
          <ArrowPathIcon class="w-5 h-5" />
          <div class="text-left">
            <div class="font-medium">{{ t('dashboard.rescanModels') }}</div>
            <div class="text-xs opacity-60">{{ t('dashboard.rescanModelsDesc') }}</div>
          </div>
        </button>

        <button
          class="btn btn-outline justify-start gap-3 h-auto py-3"
          @click="router.push('/settings')"
        >
          <Cog6ToothIcon class="w-5 h-5" />
          <div class="text-left">
            <div class="font-medium">{{ t('dashboard.settingsAction') }}</div>
            <div class="text-xs opacity-60">{{ t('dashboard.settingsDesc') }}</div>
          </div>
        </button>
      </div>
    </div>

    <!-- Loaded Models table -->
    <div v-if="loadedCount > 0">
      <h2 class="text-lg font-semibold text-base-content mb-3">
        {{ t('dashboard.loadedModels') }}
      </h2>
      <div class="overflow-x-auto bg-base-200 rounded-xl">
        <table class="table table-zebra">
          <thead>
            <tr>
              <th>{{ t('models.name') }}</th>
              <th>{{ t('models.architecture') }}</th>
              <th>{{ t('models.quantization') }}</th>
              <th>{{ t('models.size') }}</th>
              <th>{{ t('models.actions') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="model in models.loadedModels" :key="model.id">
              <td class="font-medium">{{ model.id }}</td>
              <td>
                <span class="badge badge-ghost badge-sm">{{ model.architecture || '—' }}</span>
              </td>
              <td>
                <span class="badge badge-ghost badge-sm">{{ model.quantization || '—' }}</span>
              </td>
              <td>{{ formatSize(model.size) }}</td>
              <td class="space-x-2">
                <button class="btn btn-ghost btn-xs" @click="models.unloadModel(model.id)">
                  {{ t('dashboard.unload') }}
                </button>
                <button
                  class="btn btn-primary btn-xs"
                  @click="
                    () => {
                      chat.createConversation(model.id)
                      router.push('/chat')
                    }
                  "
                >
                  {{ t('dashboard.chat') }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
