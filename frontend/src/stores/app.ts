import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import i18n from '../i18n'

export type Theme = 'light' | 'dark'
export type Locale = 'en' | 'zh-CN'

export const useAppStore = defineStore('app', () => {
  const theme = ref<Theme>((localStorage.getItem('theme') as Theme) || 'dark')
  const sidebarCollapsed = ref(false)
  const serverOnline = ref(false)
  const locale = ref<Locale>((localStorage.getItem('locale') as Locale) || 'en')

  watch(theme, (val) => {
    localStorage.setItem('theme', val)
    document.documentElement.setAttribute('data-theme', val)
  })

  watch(locale, (val) => {
    localStorage.setItem('locale', val)
    i18n.global.locale.value = val
  })

  // Apply saved theme on init
  document.documentElement.setAttribute('data-theme', theme.value)

  function toggleTheme() {
    theme.value = theme.value === 'dark' ? 'light' : 'dark'
  }

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value
  }

  function setLocale(val: Locale) {
    locale.value = val
  }

  return {
    theme,
    sidebarCollapsed,
    serverOnline,
    locale,
    toggleTheme,
    toggleSidebar,
    setLocale,
  }
})
