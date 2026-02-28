import * as api from '@/api'
import type { ModelEntry, ModelStatus } from '@/types'
import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

export const useModelStore = defineStore('models', () => {
  const models = ref<ModelEntry[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  const loadedModels = computed(() => models.value.filter((m) => m.status === 'loaded'))
  const availableModels = computed(() => models.value)
  const favoriteModels = computed(() => models.value.filter((m) => m.favorite))

  async function fetchModels() {
    loading.value = true
    error.value = null
    try {
      models.value = await api.getModels()
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      loading.value = false
    }
  }

  async function loadModel(id: string, params?: { ctx_size?: number; n_gpu_layers?: number }) {
    const model = models.value.find((m) => m.id === id)
    if (model) model.status = 'loading' as ModelStatus
    try {
      await api.loadModel(id, params)
      if (model) model.status = 'loaded'
    } catch (e) {
      if (model) model.status = 'error'
      throw e
    }
  }

  async function unloadModel(id: string) {
    await api.unloadModel(id)
    const model = models.value.find((m) => m.id === id)
    if (model) model.status = 'unloaded'
  }

  async function rescan() {
    await api.scanModels()
    await fetchModels()
  }

  async function toggleFavorite(id: string) {
    const model = models.value.find((m) => m.id === id)
    if (model) {
      model.favorite = !model.favorite
      try {
        await api.toggleFavorite(id)
      } catch {
        model.favorite = !model.favorite
      }
    }
  }

  function updateModelStatus(id: string, status: ModelStatus) {
    const model = models.value.find((m) => m.id === id)
    if (model) model.status = status
  }

  return {
    models,
    loading,
    error,
    loadedModels,
    availableModels,
    favoriteModels,
    fetchModels,
    loadModel,
    unloadModel,
    rescan,
    toggleFavorite,
    updateModelStatus,
  }
})
