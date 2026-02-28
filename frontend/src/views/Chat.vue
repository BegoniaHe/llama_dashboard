<script setup lang="ts">
import {
  AdjustmentsHorizontalIcon,
  ChatBubbleOvalLeftEllipsisIcon,
  CpuChipIcon,
  PaperAirplaneIcon,
  PlusIcon,
  StopIcon,
  TrashIcon,
  UserIcon,
} from '@heroicons/vue/24/outline'
import hljs from 'highlight.js'
import 'highlight.js/styles/github-dark.css'
import MarkdownIt from 'markdown-it'
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useChatStore } from '../stores/chat'
import { useModelStore } from '../stores/models'

const { t } = useI18n()
const chatStore = useChatStore()
const modelsStore = useModelStore()

const messageInput = ref('')
const messagesContainer = ref<HTMLDivElement>()
const showParams = ref(false)
const selectedModel = ref('')

// Inference params
const temperature = ref(0.8)
const maxTokens = ref(2048)
const topP = ref(0.95)
const systemPrompt = ref('')

const md = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: true,
  highlight(str: string, lang: string) {
    if (lang && hljs.getLanguage(lang)) {
      try {
        return hljs.highlight(str, { language: lang }).value
      } catch {
        /* ignore */
      }
    }
    return ''
  },
})

onMounted(() => {
  modelsStore.fetchModels()
})

const activeConversation = computed(() => chatStore.getActiveConversation())
const loadedModels = computed(() => modelsStore.loadedModels)

// Auto-select first loaded model
watch(
  loadedModels,
  (list) => {
    if (!selectedModel.value && list.length > 0) {
      selectedModel.value = list[0]!.id
    }
  },
  { immediate: true },
)

// Auto-scroll on new messages
watch(
  () => chatStore.streamingContent,
  () => {
    nextTick(() => {
      if (messagesContainer.value) {
        messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
      }
    })
  },
)

watch(
  () => activeConversation.value?.messages?.length,
  () => {
    nextTick(() => {
      if (messagesContainer.value) {
        messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
      }
    })
  },
)

function createConversation() {
  if (!selectedModel.value && loadedModels.value.length > 0) {
    selectedModel.value = loadedModels.value[0]!.id
  }
  if (selectedModel.value) {
    chatStore.createConversation(selectedModel.value, systemPrompt.value || undefined)
  }
}

async function sendMessage() {
  const content = messageInput.value.trim()
  if (!content || chatStore.generating) return
  messageInput.value = ''
  await chatStore.sendMessage(content, {
    maxTokens: maxTokens.value,
    temperature: temperature.value,
    topP: topP.value,
  })
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    sendMessage()
  }
}

function renderMarkdown(content: string): string {
  return md.render(content)
}
</script>

<template>
  <div class="flex h-full overflow-hidden">
    <!--  Conversation list (left panel)  -->
    <div
      class="w-64 shrink-0 bg-base-200 border-r border-base-content/10 flex flex-col hidden md:flex"
    >
      <!-- New chat button -->
      <div class="p-3 border-b border-base-content/10">
        <!-- Model selector -->
        <select
          v-if="loadedModels.length > 0"
          v-model="selectedModel"
          class="select select-bordered select-sm w-full mb-2"
        >
          <option v-for="m in loadedModels" :key="m.id" :value="m.id">{{ m.id }}</option>
        </select>
        <div v-else class="text-xs text-base-content/50 mb-2">
          {{ t('chat.noModels') }}
          <router-link to="/models" class="link link-primary">{{
            t('chat.loadModelLink')
          }}</router-link>
        </div>
        <button
          class="btn btn-primary btn-sm w-full gap-2"
          :disabled="!selectedModel"
          @click="createConversation"
        >
          <PlusIcon class="w-4 h-4" />
          {{ t('chat.newChat') }}
        </button>
      </div>

      <!-- Conversations list -->
      <div class="flex-1 overflow-y-auto custom-scrollbar">
        <ul class="menu p-2 gap-1">
          <li v-for="conv in chatStore.conversations" :key="conv.id">
            <a
              :class="[
                'flex items-center gap-2 text-sm',
                chatStore.activeConversationId === conv.id && 'active',
              ]"
              @click="chatStore.setActive(conv.id)"
            >
              <ChatBubbleOvalLeftEllipsisIcon class="w-4 h-4 shrink-0" />
              <span class="flex-1 truncate">
                {{ conv.messages[0]?.content?.slice(0, 30) || conv.model }}
              </span>
              <button
                class="btn btn-ghost btn-xs btn-square opacity-0 group-hover:opacity-100"
                @click.stop="chatStore.deleteConversation(conv.id)"
              >
                <TrashIcon class="w-3 h-3" />
              </button>
            </a>
          </li>
        </ul>
      </div>
    </div>

    <!--  Main chat area  -->
    <div class="flex-1 flex flex-col overflow-hidden">
      <!-- No conversation state -->
      <div
        v-if="!activeConversation"
        class="flex-1 flex items-center justify-center text-base-content/40"
      >
        <div class="text-center space-y-3">
          <ChatBubbleOvalLeftEllipsisIcon class="w-16 h-16 mx-auto opacity-30" />
          <h3 class="text-lg font-medium">{{ t('chat.noConversation') }}</h3>
          <p class="text-sm">{{ t('chat.selectModelPrompt') }}</p>
        </div>
      </div>

      <!-- Active conversation -->
      <template v-else>
        <!-- Messages -->
        <div ref="messagesContainer" class="flex-1 overflow-y-auto custom-scrollbar p-4 space-y-2">
          <template v-for="(msg, idx) in activeConversation.messages" :key="idx">
            <!-- Skip system messages from display -->
            <template v-if="msg.role !== 'system'">
              <!-- User message -->
              <div v-if="msg.role === 'user'" class="chat chat-end">
                <div class="chat-image avatar placeholder">
                  <div class="bg-primary text-primary-content rounded-full w-8">
                    <UserIcon class="w-4 h-4" />
                  </div>
                </div>
                <div class="chat-bubble chat-bubble-primary whitespace-pre-wrap">
                  {{ msg.content }}
                </div>
              </div>

              <!-- Assistant message -->
              <div v-else class="chat chat-start">
                <div class="chat-image avatar placeholder">
                  <div class="bg-base-300 text-base-content rounded-full w-8">
                    <CpuChipIcon class="w-4 h-4" />
                  </div>
                </div>
                <div class="chat-bubble prose-chat" v-html="renderMarkdown(msg.content)" />
              </div>
            </template>
          </template>

          <!-- Streaming content -->
          <div v-if="chatStore.generating && chatStore.streamingContent" class="chat chat-start">
            <div class="chat-image avatar placeholder">
              <div class="bg-base-300 text-base-content rounded-full w-8">
                <CpuChipIcon class="w-4 h-4" />
              </div>
            </div>
            <div class="chat-bubble prose-chat">
              <span v-html="renderMarkdown(chatStore.streamingContent)" />
              <span class="inline-block w-2 h-4 bg-current ml-0.5 cursor-blink" />
            </div>
          </div>

          <!-- Generating indicator -->
          <div v-if="chatStore.generating && !chatStore.streamingContent" class="chat chat-start">
            <div class="chat-image avatar placeholder">
              <div class="bg-base-300 text-base-content rounded-full w-8">
                <CpuChipIcon class="w-4 h-4" />
              </div>
            </div>
            <div class="chat-bubble">
              <span class="loading loading-dots loading-sm" />
            </div>
          </div>
        </div>

        <!-- Input area -->
        <div class="border-t border-base-content/10 bg-base-200/50 p-3 shrink-0">
          <!-- Parameters panel -->
          <div
            v-if="showParams"
            class="mb-3 p-3 bg-base-300 rounded-lg grid grid-cols-2 sm:grid-cols-4 gap-3"
          >
            <div class="form-control">
              <label class="label py-0">
                <span class="label-text text-xs">{{ t('chat.temperature') }}</span>
              </label>
              <input
                v-model.number="temperature"
                type="number"
                class="input input-bordered input-xs"
                min="0"
                max="2"
                step="0.1"
              />
            </div>
            <div class="form-control">
              <label class="label py-0">
                <span class="label-text text-xs">{{ t('chat.maxTokens') }}</span>
              </label>
              <input
                v-model.number="maxTokens"
                type="number"
                class="input input-bordered input-xs"
                min="1"
                max="32768"
                step="256"
              />
            </div>
            <div class="form-control">
              <label class="label py-0">
                <span class="label-text text-xs">{{ t('chat.topP') }}</span>
              </label>
              <input
                v-model.number="topP"
                type="number"
                class="input input-bordered input-xs"
                min="0"
                max="1"
                step="0.05"
              />
            </div>
            <div class="form-control">
              <label class="label py-0">
                <span class="label-text text-xs">{{ t('chat.systemPrompt') }}</span>
              </label>
              <input
                v-model="systemPrompt"
                type="text"
                class="input input-bordered input-xs"
                :placeholder="t('chat.systemPromptPlaceholder')"
              />
            </div>
          </div>

          <div class="flex items-end gap-2">
            <!-- Params toggle -->
            <button
              :class="['btn btn-ghost btn-sm btn-square', showParams && 'btn-active']"
              :title="t('chat.params')"
              @click="showParams = !showParams"
            >
              <AdjustmentsHorizontalIcon class="w-5 h-5" />
            </button>

            <!-- Message input -->
            <textarea
              v-model="messageInput"
              class="textarea textarea-bordered flex-1 min-h-10 max-h-32 resize-none leading-snug"
              rows="1"
              :placeholder="t('chat.typeMessage')"
              :disabled="chatStore.generating"
              @keydown="handleKeydown"
            />

            <!-- Send / Stop -->
            <button
              v-if="chatStore.generating"
              class="btn btn-error btn-sm gap-1"
              @click="chatStore.stopGeneration()"
            >
              <StopIcon class="w-4 h-4" />
              {{ t('chat.stop') }}
            </button>
            <button
              v-else
              class="btn btn-primary btn-sm gap-1"
              :disabled="!messageInput.trim()"
              @click="sendMessage"
            >
              <PaperAirplaneIcon class="w-4 h-4" />
              {{ t('chat.send') }}
            </button>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>
