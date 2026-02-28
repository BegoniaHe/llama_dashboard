import type {
  AppConfig,
  ChatCompletionRequest,
  ChatCompletionResponse,
  DetokenizeRequest,
  DetokenizeResponse,
  HealthResponse,
  ModelEntry,
  OpenAIModelList,
  TokenizeRequest,
  TokenizeResponse,
} from '@/types'
import api from './client'

//  Health

export async function getHealth(): Promise<HealthResponse> {
  const { data } = await api.get<HealthResponse>('/health')
  return data
}

//  OpenAI-compatible

export async function getOpenAIModels(): Promise<OpenAIModelList> {
  const { data } = await api.get<OpenAIModelList>('/v1/models')
  return data
}

export async function chatCompletion(req: ChatCompletionRequest): Promise<ChatCompletionResponse> {
  const { data } = await api.post<ChatCompletionResponse>('/v1/chat/completions', {
    ...req,
    stream: false,
  })
  return data
}

export function chatCompletionStream(
  req: ChatCompletionRequest,
  onChunk: (text: string) => void,
  onDone: () => void,
  onError: (err: Error) => void,
): AbortController {
  const controller = new AbortController()

  fetch('/v1/chat/completions', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(localStorage.getItem('api_key')
        ? { Authorization: `Bearer ${localStorage.getItem('api_key')}` }
        : {}),
    },
    body: JSON.stringify({ ...req, stream: true }),
    signal: controller.signal,
  })
    .then(async (response) => {
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`)
      }
      const reader = response.body?.getReader()
      if (!reader) throw new Error('No response body')

      const decoder = new TextDecoder()
      let buffer = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })
        const lines = buffer.split('\n')
        buffer = lines.pop() || ''

        for (const line of lines) {
          const trimmed = line.trim()
          if (!trimmed || !trimmed.startsWith('data: ')) continue
          const payload = trimmed.slice(6)
          if (payload === '[DONE]') {
            onDone()
            return
          }
          try {
            const chunk = JSON.parse(payload)
            const content = chunk.choices?.[0]?.delta?.content
            if (content) onChunk(content)
            if (chunk.choices?.[0]?.finish_reason) {
              onDone()
              return
            }
          } catch {
            // skip malformed chunks
          }
        }
      }
      onDone()
    })
    .catch((err) => {
      if (err.name !== 'AbortError') onError(err)
    })

  return controller
}

//  Model management API

export async function getModels(): Promise<ModelEntry[]> {
  const { data } = await api.get<ModelEntry[]>('/api/models')
  return data
}

export async function getModelDetails(id: string): Promise<ModelEntry> {
  const { data } = await api.get<ModelEntry>(`/api/models/${encodeURIComponent(id)}/details`)
  return data
}

export async function loadModel(
  id: string,
  params?: { ctx_size?: number; n_gpu_layers?: number },
): Promise<void> {
  await api.post(`/api/models/${encodeURIComponent(id)}/load`, params)
}

export async function unloadModel(id: string): Promise<void> {
  await api.post(`/api/models/${encodeURIComponent(id)}/unload`)
}

export async function scanModels(): Promise<void> {
  await api.post('/api/models/scan')
}

export async function toggleFavorite(id: string): Promise<void> {
  await api.put(`/api/models/${encodeURIComponent(id)}/favorite`)
}

//  Tokenize

export async function tokenize(req: TokenizeRequest): Promise<TokenizeResponse> {
  const { data } = await api.post<TokenizeResponse>('/tokenize', req)
  return data
}

export async function detokenize(req: DetokenizeRequest): Promise<DetokenizeResponse> {
  const { data } = await api.post<DetokenizeResponse>('/detokenize', req)
  return data
}

//  Config

export async function getConfig(): Promise<AppConfig> {
  const { data } = await api.get<AppConfig>('/api/config')
  return data
}

export async function updateConfig(config: Partial<AppConfig>): Promise<void> {
  await api.put('/api/config', config)
}
