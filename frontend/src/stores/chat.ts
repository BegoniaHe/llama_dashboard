import { chatCompletionStream } from '@/api'
import type { ChatMessage } from '@/types'
import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface Conversation {
  id: string
  title: string
  model: string
  messages: ChatMessage[]
  createdAt: string
}

export const useChatStore = defineStore('chat', () => {
  const conversations = ref<Conversation[]>([])
  const activeConversationId = ref<string | null>(null)
  const generating = ref(false)
  const streamingContent = ref('')
  const abortController = ref<AbortController | null>(null)

  function getActiveConversation(): Conversation | undefined {
    return conversations.value.find((c) => c.id === activeConversationId.value)
  }

  function createConversation(model: string, systemPrompt?: string): string {
    const id = crypto.randomUUID()
    const messages: ChatMessage[] = []
    if (systemPrompt) {
      messages.push({ role: 'system', content: systemPrompt })
    }
    conversations.value.unshift({
      id,
      title: 'New Chat',
      model,
      messages,
      createdAt: new Date().toISOString(),
    })
    activeConversationId.value = id
    return id
  }

  function deleteConversation(id: string) {
    conversations.value = conversations.value.filter((c) => c.id !== id)
    if (activeConversationId.value === id) {
      activeConversationId.value = conversations.value[0]?.id ?? null
    }
  }

  function setActive(id: string) {
    activeConversationId.value = id
  }

  async function sendMessage(
    content: string,
    options: {
      maxTokens?: number
      temperature?: number
      topP?: number
    } = {},
  ) {
    const conv = getActiveConversation()
    if (!conv) return

    conv.messages.push({ role: 'user', content })

    // Auto-generate title from first user message
    if (conv.title === 'New Chat') {
      conv.title = content.slice(0, 50) + (content.length > 50 ? '...' : '')
    }

    generating.value = true
    streamingContent.value = ''

    const controller = chatCompletionStream(
      {
        model: conv.model,
        messages: [...conv.messages],
        max_tokens: options.maxTokens ?? 2048,
        temperature: options.temperature ?? 0.7,
        top_p: options.topP ?? 0.9,
        stream: true,
      },
      (text) => {
        streamingContent.value += text
      },
      () => {
        conv.messages.push({
          role: 'assistant',
          content: streamingContent.value,
        })
        streamingContent.value = ''
        generating.value = false
        abortController.value = null
      },
      (err) => {
        conv.messages.push({
          role: 'assistant',
          content: `Error: ${err.message}`,
        })
        streamingContent.value = ''
        generating.value = false
        abortController.value = null
      },
    )

    abortController.value = controller
  }

  function stopGeneration() {
    abortController.value?.abort()
    const conv = getActiveConversation()
    if (conv && streamingContent.value) {
      conv.messages.push({
        role: 'assistant',
        content: streamingContent.value,
      })
    }
    streamingContent.value = ''
    generating.value = false
    abortController.value = null
  }

  return {
    conversations,
    activeConversationId,
    generating,
    streamingContent,
    getActiveConversation,
    createConversation,
    deleteConversation,
    setActive,
    sendMessage,
    stopGeneration,
  }
})
