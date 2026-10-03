import { hasRole, session } from '@front/api-client'
import { createRouter, createWebHistory } from 'vue-router'
import { bumpSessionTick, sessionActive } from './sessionActive'

export const router = createRouter({
  history: createWebHistory("/user/"),
  routes: [
    { path: '/login', name: 'login', component: () => import('./views/LoginView.vue') },
    { path: '/forbidden', name: 'forbidden', component: () => import('./views/ForbiddenView.vue') },
    {
      path: '/',
      name: 'home',
      component: () => import('./views/HomeView.vue'),
      meta: { auth: true, role: "user" as 'admin' | 'manager' | 'user' },
    },
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
})

// 角色守衛：未登入 → 登入頁；角色不足 → 403 頁（伺服器端 403 才是真相——此為呈現層）。
router.beforeEach((to) => {
  if (to.meta.auth && !sessionActive()) return { name: 'login', query: { redirect: to.fullPath } }
  const role = to.meta.role as 'admin' | 'manager' | 'user' | undefined
  const s = session()
  if (role && s && !hasRole(s.principal.roles, role)) return { name: 'forbidden' }
})
// 導航後更新 session 響應式 tick（login/logout/401 清除皆經導航呈現）。
router.afterEach(() => bumpSessionTick())
