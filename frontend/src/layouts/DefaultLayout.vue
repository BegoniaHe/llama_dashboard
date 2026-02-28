<script setup lang="ts">
import {
  Bars3Icon,
  ChatBubbleLeftRightIcon,
  ChevronLeftIcon,
  Cog6ToothIcon,
  CpuChipIcon,
  HomeIcon,
  LanguageIcon,
  MoonIcon,
  SunIcon,
} from '@heroicons/vue/24/outline'
import { onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink, useRoute } from 'vue-router'
import { getHealth } from '../api'
import { useAppStore } from '../stores/app'

const { t } = useI18n()
const app = useAppStore()
const route = useRoute()

let healthInterval: ReturnType<typeof setInterval>

async function checkHealth() {
  try {
    await getHealth()
    app.serverOnline = true
  } catch {
    app.serverOnline = false
  }
}

onMounted(() => {
  checkHealth()
  healthInterval = setInterval(checkHealth, 10000)
})

onUnmounted(() => {
  clearInterval(healthInterval)
})

const navItems = [
  { path: '/', icon: HomeIcon, labelKey: 'nav.dashboard' },
  { path: '/models', icon: CpuChipIcon, labelKey: 'nav.models' },
  { path: '/chat', icon: ChatBubbleLeftRightIcon, labelKey: 'nav.chat' },
  { path: '/settings', icon: Cog6ToothIcon, labelKey: 'nav.settings' },
]

function isActive(path: string) {
  if (path === '/') return route.path === '/'
  return route.path.startsWith(path)
}

function toggleLocale() {
  app.setLocale(app.locale === 'en' ? 'zh-CN' : 'en')
}

function closeMobileSidebar() {
  if (window.innerWidth < 1024) {
    app.sidebarCollapsed = true
  }
}
</script>

<template>
  <!-- Mobile drawer backdrop -->
  <div
    v-if="!app.sidebarCollapsed"
    class="fixed inset-0 bg-black/50 z-40 lg:hidden"
    @click="app.toggleSidebar()"
  />

  <div class="flex h-screen overflow-hidden bg-base-100">
    <!--  Sidebar  -->
    <aside
      :class="[
        'fixed z-50 lg:static lg:z-auto flex flex-col h-full bg-base-300 border-r border-base-content/10 transition-all duration-300 shrink-0',
        app.sidebarCollapsed
          ? '-translate-x-full lg:translate-x-0 lg:w-20 overflow-hidden'
          : 'translate-x-0 w-72',
      ]"
    >
      <!-- Logo -->
      <div class="flex items-center gap-3 px-5 h-16 border-b border-base-content/10 shrink-0">
        <div
          class="w-8 h-8 rounded-lg bg-primary flex items-center justify-center text-primary-content font-bold text-sm shrink-0"
        >
          L
        </div>
        <span
          v-if="!app.sidebarCollapsed"
          class="font-semibold text-base-content whitespace-nowrap"
        >
          {{ t('nav.appName') }}
        </span>
      </div>

      <!-- Nav links -->
      <nav class="flex-1 overflow-y-auto custom-scrollbar px-3 py-4 space-y-1">
        <RouterLink
          v-for="item in navItems"
          :key="item.path"
          :to="item.path"
          :class="[
            'flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-colors duration-150 no-underline',
            app.sidebarCollapsed ? 'justify-center' : '',
            isActive(item.path)
              ? 'bg-primary/15 text-primary font-semibold'
              : 'text-base-content/60 hover:bg-base-content/8 hover:text-base-content',
          ]"
          :title="t(item.labelKey)"
          @click="closeMobileSidebar"
        >
          <component :is="item.icon" class="w-5 h-5 shrink-0" />
          <span v-if="!app.sidebarCollapsed" class="truncate">{{ t(item.labelKey) }}</span>
        </RouterLink>
      </nav>

      <!-- Bottom controls -->
      <div class="px-3 py-4 border-t border-base-content/10 space-y-2 shrink-0">
        <!-- Theme toggle -->
        <button
          :class="[
            'flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-colors duration-150 w-full',
            app.sidebarCollapsed ? 'justify-center' : '',
            'text-base-content/60 hover:bg-base-content/8 hover:text-base-content',
          ]"
          :title="app.theme === 'dark' ? t('settings.light') : t('settings.dark')"
          @click="app.toggleTheme()"
        >
          <SunIcon v-if="app.theme === 'dark'" class="w-5 h-5 shrink-0" />
          <MoonIcon v-else class="w-5 h-5 shrink-0" />
          <span v-if="!app.sidebarCollapsed" class="truncate">
            {{ app.theme === 'dark' ? t('settings.light') : t('settings.dark') }}
          </span>
        </button>

        <!-- Language toggle -->
        <button
          :class="[
            'flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-colors duration-150 w-full',
            app.sidebarCollapsed ? 'justify-center' : '',
            'text-base-content/60 hover:bg-base-content/8 hover:text-base-content',
          ]"
          title="Language"
          @click="toggleLocale"
        >
          <LanguageIcon class="w-5 h-5 shrink-0" />
          <span v-if="!app.sidebarCollapsed" class="truncate">{{
            app.locale === 'en' ? '中文' : 'English'
          }}</span>
        </button>
      </div>
    </aside>

    <!--  Main content area  -->
    <div class="flex-1 flex flex-col overflow-hidden">
      <!-- Header -->
      <header
        class="h-16 bg-base-200/50 backdrop-blur border-b border-base-content/10 flex items-center px-4 lg:px-6 gap-4 shrink-0"
      >
        <button class="btn btn-ghost btn-sm btn-square" @click="app.toggleSidebar()">
          <Bars3Icon v-if="app.sidebarCollapsed" class="w-5 h-5" />
          <ChevronLeftIcon v-else class="w-5 h-5" />
        </button>

        <!-- Spacer -->
        <div class="flex-1" />

        <!-- Server status -->
        <div class="flex items-center gap-2 text-sm">
          <span
            :class="['w-2.5 h-2.5 rounded-full', app.serverOnline ? 'bg-success' : 'bg-error']"
          />
          <span class="hidden sm:inline text-base-content/70">
            {{ app.serverOnline ? t('common.online') : t('common.offline') }}
          </span>
        </div>
      </header>

      <!-- Page content -->
      <main class="flex-1 overflow-y-auto custom-scrollbar">
        <RouterView />
      </main>
    </div>
  </div>
</template>
