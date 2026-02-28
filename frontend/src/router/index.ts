import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'dashboard',
      component: () => import('@/views/Dashboard.vue'),
      meta: { title: 'Dashboard' },
    },
    {
      path: '/models',
      name: 'models',
      component: () => import('@/views/Models.vue'),
      meta: { title: 'Models' },
    },
    {
      path: '/models/:id',
      name: 'model-detail',
      component: () => import('@/views/ModelDetail.vue'),
      meta: { title: 'Model Detail' },
    },
    {
      path: '/chat',
      name: 'chat',
      component: () => import('@/views/Chat.vue'),
      meta: { title: 'Chat' },
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('@/views/Settings.vue'),
      meta: { title: 'Settings' },
    },
  ],
})

router.beforeEach((to) => {
  document.title = `${(to.meta.title as string) || 'Llama Dashboard'} — Llama Dashboard`
})

export default router
