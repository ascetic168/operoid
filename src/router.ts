import { createRouter, createWebHistory } from "vue-router";

const routes = [
  { path: "/", redirect: "/instances" },
  { path: "/factories", name: "factories", component: () => import("@/views/FactoriesView.vue") },
  { path: "/templates", name: "templates", component: () => import("@/views/EmployeeTemplateView.vue") },
  { path: "/instances", name: "instances", component: () => import("@/views/EmployeeInstanceView.vue") },
  { path: "/instances/:id/chat", name: "employee-chat", props: true, component: () => import("@/views/EmployeeChatView.vue") },
  { path: "/operations", name: "operations", component: () => import("@/views/OperationsView.vue") },
  { path: "/brains", name: "brains", component: () => import("@/views/BrainsView.vue") },
  { path: "/config", name: "config", component: () => import("@/views/ConfigView.vue") },
];

export default createRouter({
  history: createWebHistory(),
  routes,
});
