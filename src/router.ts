import { createRouter, createWebHistory } from "vue-router";

const routes = [
  { path: "/", name: "factories", component: () => import("@/views/FactoriesView.vue") },
  { path: "/operations", name: "operations", component: () => import("@/views/OperationsView.vue") },
  { path: "/config", name: "config", component: () => import("@/views/ConfigView.vue") },
];

export default createRouter({
  history: createWebHistory(),
  routes,
});
