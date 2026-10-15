import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createRouter, createWebHashHistory } from 'vue-router'
import App from './App.vue'
import './style.css'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'home', component: () => import('./views/HomeView.vue') },
    { path: '/settings', name: 'settings', component: () => import('./views/SettingsView.vue') },
    {
      path: '/p/:id',
      component: () => import('./views/ProjectView.vue'),
      children: [
        { path: '', redirect: (to) => ({ name: 'plan', params: to.params }) },
        { path: 'plan', name: 'plan', component: () => import('./views/project/PlanTab.vue') },
        { path: 'run', name: 'run', component: () => import('./views/project/RunTab.vue') },
        { path: 'review', name: 'review', component: () => import('./views/project/ReviewTab.vue') },
        { path: 'export', name: 'export', component: () => import('./views/project/ExportTab.vue') },
        { path: 'activity', name: 'activity', component: () => import('./views/project/ActivityTab.vue') },
      ],
    },
  ],
})

createApp(App).use(createPinia()).use(router).mount('#app')
