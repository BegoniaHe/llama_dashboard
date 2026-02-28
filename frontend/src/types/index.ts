// ── Model types ─────────────────────────────────────────

export interface ModelInfo {
  id: string
  filename: string
  path: string
  size: number
  architecture: string | null
  parameters: string | null
  context_length: number | null
  file_type: string | null
  quantization: string | null
  chat_template: string | null
}

export type ModelStatus = 'unloaded' | 'loading' | 'loaded' | 'error'

export interface ModelEntry extends ModelInfo {
  status: ModelStatus
  loaded_at?: string
  last_used?: string
  favorite?: boolean
  alias?: string
}

// ── Chat types ──────────────────────────────────────────

export interface ChatMessage {
  role: 'system' | 'user' | 'assistant'
  content: string
}

export interface ChatCompletionRequest {
  model?: string
  messages: ChatMessage[]
  max_tokens?: number
  temperature?: number
  top_p?: number
  stream?: boolean
  stop?: string[]
  frequency_penalty?: number
  presence_penalty?: number
  seed?: number
}

export interface ChatCompletionChoice {
  index: number
  message: ChatMessage
  finish_reason: string | null
}

export interface ChatCompletionUsage {
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
}

export interface ChatCompletionResponse {
  id: string
  object: string
  created: number
  model: string
  choices: ChatCompletionChoice[]
  usage: ChatCompletionUsage
}

export interface ChatCompletionChunk {
  id: string
  object: string
  created: number
  model: string
  choices: {
    index: number
    delta: { role?: string; content?: string }
    finish_reason: string | null
  }[]
}

// ── API model list ──────────────────────────────────────

export interface OpenAIModel {
  id: string
  object: string
  owned_by: string
}

export interface OpenAIModelList {
  object: string
  data: OpenAIModel[]
}

// ── Health ──────────────────────────────────────────────

export interface HealthResponse {
  status: string
}

// ── Config ──────────────────────────────────────────────

export interface AppConfig {
  model_dirs: string[]
  default_ctx_size: number
  default_n_gpu_layers: number
  default_temperature: number
  api_key?: string
}

// ── System ──────────────────────────────────────────────

export interface SystemInfo {
  version: string
  uptime_seconds: number
  models_loaded: number
  models_available: number
}

// ── WebSocket events ────────────────────────────────────

export interface WsEvent {
  type: string
  timestamp: string
  data: Record<string, unknown>
}

// ── Tokenize ────────────────────────────────────────────

export interface TokenizeRequest {
  content: string
  add_special?: boolean
  parse_special?: boolean
}

export interface TokenizeResponse {
  tokens: number[]
}

export interface DetokenizeRequest {
  tokens: number[]
}

export interface DetokenizeResponse {
  content: string
}
