import { defineStore } from "pinia";
import { ref } from "vue";
import {
  AGENT_WS,
  agentEnsureWorkspace,
  agentListTemplates,
  agentListEmployees,
  agentCreateTemplate,
  agentDeployInstance,
  agentDeleteTemplate,
  agentDeleteEmployee,
  agentRenameTemplate,
  agentRenameEmployee,
  agentSendMessage,
  type EmployeeTemplate,
  type Employee,
} from "@/lib/tauri";

/** Agent-OS store：員工模板 ＋ 員工實體（GUI 管理面）。workspace 預設 `ws-default`。 */
export const useAgentStore = defineStore("agent", () => {
  const templates = ref<EmployeeTemplate[]>([]);
  const employees = ref<Employee[]>([]);
  const workspaceId = ref<string>(AGENT_WS);
  const loading = ref(false);
  const error = ref<string | null>(null);

  /** 確保 `ws-default` 存在，並載入模板／實體清單。 */
  async function ensureAndLoad() {
    loading.value = true;
    error.value = null;
    try {
      const r = await agentEnsureWorkspace();
      workspaceId.value = r.workspace_id;
      await Promise.all([loadTemplates(), loadEmployees()]);
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function loadTemplates() {
    templates.value = await agentListTemplates(workspaceId.value);
  }
  async function loadEmployees() {
    employees.value = await agentListEmployees(workspaceId.value);
  }

  async function createTemplate(name: string, brainId: string | null, role: string | null) {
    await agentCreateTemplate(name, brainId, role, workspaceId.value);
    await loadTemplates();
  }
  async function renameTemplate(id: string, name: string) {
    await agentRenameTemplate(id, name);
    await loadTemplates();
  }
  async function deleteTemplate(id: string) {
    await agentDeleteTemplate(id);
    await loadTemplates();
  }

  async function deployInstance(templateId: string, name: string) {
    await agentDeployInstance(templateId, name);
    await loadEmployees();
  }
  async function renameEmployee(id: string, name: string) {
    await agentRenameEmployee(id, name);
    await loadEmployees();
  }
  async function deleteEmployee(id: string) {
    await agentDeleteEmployee(id);
    await loadEmployees();
  }

  /** 溝通：訊息 → 員工 Inbox（Assigned task）＋喚醒。員工狀態由排程器非同步驅動（即時觀察屬 6d）。 */
  async function sendMessage(employeeId: string, text: string, commitmentId: string | null = null) {
    await agentSendMessage(employeeId, text, commitmentId);
  }

  /** 模板 id → 模板（供實體頁顯示來源模板名）。 */
  function templateById(id: string | null): EmployeeTemplate | undefined {
    if (!id) return undefined;
    return templates.value.find((t) => t.id === id);
  }

  return {
    templates,
    employees,
    workspaceId,
    loading,
    error,
    ensureAndLoad,
    loadTemplates,
    loadEmployees,
    createTemplate,
    renameTemplate,
    deleteTemplate,
    deployInstance,
    renameEmployee,
    deleteEmployee,
    sendMessage,
    templateById,
  };
});
