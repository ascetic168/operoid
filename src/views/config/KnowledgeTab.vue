<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { KeyRound, Plus, RefreshCw, Save, ShieldX, Trash2 } from "lucide-vue-next";
import { formatError, type KnowledgeOverview, type PolicyRuleView, type SecurityLevel, knowledgeAttrsSet, knowledgeGrantCreate, knowledgeGrantRevoke, knowledgeOverviewLoad, knowledgePolicySave, knowledgePrincipalCreate, knowledgeScopeSave, knowledgeTokenIssue, knowledgeTokenRevoke } from "@/lib/tauri";

const LEVELS: SecurityLevel[] = ["public", "internal", "confidential", "secret"];

const overview = ref<KnowledgeOverview | null>(null);
const loadError = ref<string | null>(null);
const actionError = ref<string | null>(null);
const busy = ref(false);

// ---- grants 建立表單 ----
const gPrincipal = ref("");
const gScope = ref("");
const gTask = ref("");
const gPurpose = ref("");
const gTtlHours = ref("24");
const grantCreated = ref(false);

// ---- scope 建立表單（upsert）----
const sId = ref("");
const sVisibility = ref("department");
const sClassification = ref<SecurityLevel>("internal");
const sSources = ref("");
const sDept = ref("");
const sProj = ref("");
const scopeSaved = ref(false);

// ---- policy 規則草稿 ----
interface DraftRule {
  key: number;
  id: string;
  priority: number;
  effect: "allow" | "deny";
  principals: string;
  scopes: string;
  classifications: string;
  departments: string;
  projects: string;
  department_membership: boolean;
  project_membership: boolean;
}
let keySeq = 0;
const drafts = ref<DraftRule[]>([]);
const policyVersion = ref<number | null>(null);
const policySaved = ref(false);

// ---- principal 建立表單 ----
const pId = ref("");
const pName = ref("");
const pType = ref<"human" | "ai_employee">("human");
const pEmployee = ref("");

// ---- token 明文（只顯示一次）----
const issuedToken = ref<{ principal: string; token: string } | null>(null);

const csvSplit = (s: string): string[] =>
  s.split(",").map((t) => t.trim()).filter(Boolean);
const csvJoin = (a?: string[] | null): string => (a ?? []).join(", ");
const levelsCsvSplit = (s: string): SecurityLevel[] =>
  csvSplit(s).filter((t): t is SecurityLevel => (LEVELS as string[]).includes(t));

async function refresh() {
  loadError.value = null;
  try {
    overview.value = await knowledgeOverviewLoad();
    policyVersion.value = overview.value?.policy?.version ?? null;
    drafts.value = (overview.value?.policy?.rules ?? []).map((r) => ({
      key: keySeq++,
      id: r.id,
      priority: r.priority,
      effect: r.effect,
      principals: csvJoin(r.principals),
      scopes: csvJoin(r.scopes),
      classifications: csvJoin(r.classifications),
      departments: csvJoin(r.departments),
      projects: csvJoin(r.projects),
      department_membership: r.department_membership ?? false,
      project_membership: r.project_membership ?? false,
    }));
  } catch (e) {
    loadError.value = formatError(e);
  }
}
onMounted(refresh);

function fmtTime(iso: string): string {
  return iso.replace("T", " ").replace(/\.\d+Z$/, "Z");
}

async function withBusy(fn: () => Promise<void>) {
  actionError.value = null;
  busy.value = true;
  try {
    await fn();
    await refresh();
  } catch (e) {
    actionError.value = formatError(e);
  } finally {
    busy.value = false;
  }
}

function addRule() {
  drafts.value.push({
    key: keySeq,
    id: `rule-${keySeq++}`,
    priority: 10,
    effect: "allow",
    principals: "",
    scopes: "",
    classifications: "",
    departments: "",
    projects: "",
    department_membership: false,
    project_membership: false,
  });
}

function removeRule(key: number) {
  drafts.value = drafts.value.filter((d) => d.key !== key);
}

async function savePolicy() {
  const rules: PolicyRuleView[] = drafts.value.map((d) => ({
    id: d.id,
    priority: d.priority,
    effect: d.effect,
    principals: csvSplit(d.principals).length ? csvSplit(d.principals) : null,
    principal_types: null,
    scopes: csvSplit(d.scopes).length ? csvSplit(d.scopes) : null,
    departments: csvSplit(d.departments).length ? csvSplit(d.departments) : null,
    projects: csvSplit(d.projects).length ? csvSplit(d.projects) : null,
    classifications: levelsCsvSplit(d.classifications).length ? levelsCsvSplit(d.classifications) : null,
    department_membership: d.department_membership,
    project_membership: d.project_membership,
  }));
  await withBusy(async () => {
    const p = await knowledgePolicySave(rules);
    policyVersion.value = p.version;
    policySaved.value = true;
    setTimeout(() => (policySaved.value = false), 2500);
  });
}

async function createGrant() {
  if (!gPrincipal.value || !gScope.value) return;
  const ttl = Math.max(1, Math.floor(Number(gTtlHours.value) * 3600) || 0);
  await withBusy(async () => {
    await knowledgeGrantCreate({
      principal_id: gPrincipal.value,
      scope_id: gScope.value,
      task_id: gTask.value || undefined,
      purpose: gPurpose.value || undefined,
      ttl_secs: ttl,
    });
    grantCreated.value = true;
    gTask.value = "";
    gPurpose.value = "";
  });
}

async function revokeGrant(id: string) {
  await withBusy(() => knowledgeGrantRevoke(id).then(() => undefined));
}

async function saveScope() {
  if (!sId.value.trim() || !sSources.value.trim()) return;
  await withBusy(async () => {
    await knowledgeScopeSave({
      id: sId.value.trim(),
      visibility: sVisibility.value,
      classification: sClassification.value,
      source_ids: csvSplit(sSources.value),
      department: sDept.value.trim() || undefined,
      project: sProj.value.trim() || undefined,
    });
    scopeSaved.value = true;
    sId.value = "";
    sSources.value = "";
    sDept.value = "";
    sProj.value = "";
  });
}

async function createPrincipal() {
  if (!pId.value.trim()) return;
  await withBusy(async () => {
    await knowledgePrincipalCreate({
      id: pId.value.trim(),
      display_name: pName.value.trim() || undefined,
      principal_type: pType.value,
      employee_id: pEmployee.value.trim() || undefined,
    });
    pId.value = "";
    pName.value = "";
    pEmployee.value = "";
  });
}

async function issueToken(id: string) {
  await withBusy(async () => {
    const { token } = await knowledgeTokenIssue(id);
    issuedToken.value = { principal: id, token };
  });
}

async function revokeToken(id: string) {
  await withBusy(() => knowledgeTokenRevoke(id).then(() => undefined));
}

function clearanceValue(p: { attrs?: { clearance?: SecurityLevel | null } | null }): string {
  return p.attrs?.clearance ?? "none";
}

async function setClearance(id: string, level: string) {
  const p = overview.value?.principals.find((x) => x.id === id);
  const clearance = level === "none" ? null : (level as SecurityLevel);
  await withBusy(() =>
    knowledgeAttrsSet(id, {
      clearance,
      departments: p?.attrs?.departments ?? [],
      projects: p?.attrs?.projects ?? [],
      roles: p?.attrs?.roles ?? [],
    }).then(() => undefined),
  );
}

function copyToken() {
  if (issuedToken.value) void navigator.clipboard?.writeText(issuedToken.value.token);
}

const principalOptions = computed(() => overview.value?.principals ?? []);
const scopeOptions = computed(() => overview.value?.scopes ?? []);
</script>

<template>
  <div>
    <h2 class="mb-1 text-lg font-semibold">{{ $t("knowledgeView.title") }}</h2>
    <p class="mb-6 text-sm text-muted-foreground">{{ $t("knowledgeView.desc") }}</p>

    <div v-if="loadError" class="mb-4 rounded border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
      {{ loadError }}
    </div>
    <div v-if="actionError" class="mb-4 rounded border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
      {{ actionError }}
    </div>

    <div class="mb-4 flex justify-end">
      <button class="inline-flex items-center gap-1.5 rounded border px-2.5 py-1.5 text-sm hover:bg-accent" :disabled="busy" @click="refresh">
        <RefreshCw class="size-4" />{{ $t("knowledgeView.refresh") }}
      </button>
    </div>

    <!-- Scopes -->
    <section class="mb-8">
      <h3 class="mb-2 font-medium">{{ $t("knowledgeView.scopes.title") }}</h3>
      <table class="mb-3 w-full text-sm">
        <thead>
          <tr class="border-b text-left text-muted-foreground">
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.id") }}</th>
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.visibility") }}</th>
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.level") }}</th>
            <th class="py-1.5 pr-3">sources</th>
            <th class="py-1.5">{{ $t("knowledgeView.col.belonging") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="s in overview?.scopes ?? []" :key="s.id" class="border-b">
            <td class="py-1.5 pr-3 font-mono">{{ s.id }}</td>
            <td class="py-1.5 pr-3">{{ s.visibility }}</td>
            <td class="py-1.5 pr-3">{{ s.classification }}</td>
            <td class="py-1.5 pr-3 font-mono text-xs">{{ s.source_ids.join(", ") }}</td>
            <td class="py-1.5 text-xs text-muted-foreground">{{ s.department ?? s.project ?? "—" }}</td>
          </tr>
        </tbody>
      </table>

      <form class="flex flex-wrap items-end gap-2 rounded border p-3" @submit.prevent="saveScope">
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.col.id") }}</div>
          <input v-model="sId" class="w-36 rounded border bg-background px-2 py-1 font-mono text-sm" placeholder="dept-marketing" required />
        </label>
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.col.visibility") }}</div>
          <select v-model="sVisibility" class="w-32 rounded border bg-background px-2 py-1 text-sm">
            <option value="company">company</option>
            <option value="department">department</option>
            <option value="project">project</option>
            <option value="restricted">restricted</option>
          </select>
        </label>
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.col.level") }}</div>
          <select v-model="sClassification" class="w-32 rounded border bg-background px-2 py-1 text-sm">
            <option v-for="lv in LEVELS" :key="lv" :value="lv">{{ lv }}</option>
          </select>
        </label>
        <label class="text-xs text-muted-foreground">
          <div>sources</div>
          <input v-model="sSources" class="w-40 rounded border bg-background px-2 py-1 font-mono text-sm" placeholder="src-a, src-b" required />
        </label>
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.scopes.department") }}</div>
          <input v-model="sDept" class="w-28 rounded border bg-background px-2 py-1 text-sm" />
        </label>
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.scopes.project") }}</div>
          <input v-model="sProj" class="w-28 rounded border bg-background px-2 py-1 text-sm" />
        </label>
        <button
          type="submit"
          class="inline-flex items-center gap-1.5 rounded border px-2.5 py-1.5 text-sm hover:bg-accent"
          :disabled="busy || !sId.trim() || !sSources.trim()"
        >
          <Plus class="size-4" />{{ $t("knowledgeView.scopes.save") }}
        </button>
        <span v-if="scopeSaved" class="text-xs text-emerald-500">{{ $t("knowledgeView.saved") }}</span>
      </form>
    </section>

    <!-- Policy -->
    <section class="mb-8">
      <div class="mb-2 flex items-center gap-2">
        <h3 class="font-medium">{{ $t("knowledgeView.policy.title") }}</h3>
        <span v-if="policyVersion !== null" class="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">v{{ policyVersion }}</span>
        <span v-if="policySaved" class="text-xs text-emerald-500">{{ $t("knowledgeView.policy.saved") }}</span>
      </div>
      <p class="mb-2 text-xs text-muted-foreground">{{ $t("knowledgeView.policy.hint") }}</p>

      <div v-for="d in drafts" :key="d.key" class="mb-2 flex flex-wrap items-center gap-2 rounded border p-2">
        <input v-model="d.id" class="w-32 rounded border bg-background px-2 py-1 font-mono text-xs" placeholder="rule-id" />
        <input v-model.number="d.priority" type="number" class="w-16 rounded border bg-background px-2 py-1 text-xs" />
        <select v-model="d.effect" class="w-20 rounded border bg-background px-2 py-1 text-xs">
          <option value="allow">allow</option>
          <option value="deny">deny</option>
        </select>
        <input v-model="d.principals" class="w-40 rounded border bg-background px-2 py-1 font-mono text-xs" :placeholder="$t('knowledgeView.policy.principals')" />
        <input v-model="d.scopes" class="w-36 rounded border bg-background px-2 py-1 font-mono text-xs" :placeholder="$t('knowledgeView.policy.scopes')" />
        <input v-model="d.classifications" class="w-36 rounded border bg-background px-2 py-1 font-mono text-xs" :placeholder="$t('knowledgeView.policy.levels')" />
        <input v-model="d.departments" class="w-32 rounded border bg-background px-2 py-1 font-mono text-xs" :placeholder="$t('knowledgeView.policy.departments')" />
        <label class="flex items-center gap-1 text-xs text-muted-foreground">
          <input v-model="d.department_membership" type="checkbox" />
          {{ $t("knowledgeView.policy.deptMember") }}
        </label>
        <button class="ml-auto rounded border p-1 text-destructive hover:bg-accent" :title="$t('knowledgeView.policy.delete')" @click="removeRule(d.key)">
          <Trash2 class="size-3.5" />
        </button>
      </div>

      <div class="flex gap-2">
        <button class="inline-flex items-center gap-1.5 rounded border px-2.5 py-1.5 text-sm hover:bg-accent" @click="addRule">
          <Plus class="size-4" />{{ $t("knowledgeView.policy.addRule") }}
        </button>
        <button
          class="inline-flex items-center gap-1.5 rounded border px-2.5 py-1.5 text-sm hover:bg-accent"
          :disabled="busy"
          @click="savePolicy"
        >
          <Save class="size-4" />{{ $t("knowledgeView.policy.save") }}
        </button>
      </div>
    </section>

    <!-- Grants -->
    <section class="mb-8">
      <h3 class="mb-2 font-medium">{{ $t("knowledgeView.grants.title") }}</h3>
      <table class="mb-3 w-full text-sm">
        <thead>
          <tr class="border-b text-left text-muted-foreground">
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.principal") }}</th>
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.scope") }}</th>
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.expires") }}</th>
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.state") }}</th>
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.task") }}</th>
            <th class="py-1.5"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="g in overview?.grants ?? []" :key="g.id" class="border-b">
            <td class="py-1.5 pr-3 font-mono text-xs">{{ g.principal_id }}</td>
            <td class="py-1.5 pr-3 font-mono">{{ g.scope_id }}</td>
            <td class="py-1.5 pr-3 text-xs">{{ fmtTime(g.expires_at) }}</td>
            <td class="py-1.5 pr-3">
              <span :class="g.state === 'active' ? 'text-emerald-500' : 'text-muted-foreground'">{{ g.state }}</span>
            </td>
            <td class="py-1.5 pr-3 text-xs text-muted-foreground">{{ g.task_id ?? g.purpose ?? "—" }}</td>
            <td class="py-1.5 text-right">
              <button
                v-if="g.state === 'active'"
                class="inline-flex items-center gap-1 rounded border px-2 py-0.5 text-xs hover:bg-accent"
                :disabled="busy"
                @click="revokeGrant(g.id)"
              >
                <ShieldX class="size-3.5" />{{ $t("knowledgeView.grants.revoke") }}
              </button>
            </td>
          </tr>
          <tr v-if="!(overview?.grants ?? []).length">
            <td colspan="6" class="py-2 text-muted-foreground">{{ $t("knowledgeView.grants.empty") }}</td>
          </tr>
        </tbody>
      </table>

      <form class="flex flex-wrap items-end gap-2 rounded border p-3" @submit.prevent="createGrant">
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.col.principal") }}</div>
          <select v-model="gPrincipal" class="w-44 rounded border bg-background px-2 py-1 text-sm" required>
            <option value="" disabled>{{ $t("knowledgeView.pick") }}</option>
            <option v-for="p in principalOptions" :key="p.id" :value="p.id">{{ p.id }}</option>
          </select>
        </label>
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.col.scope") }}</div>
          <select v-model="gScope" class="w-40 rounded border bg-background px-2 py-1 text-sm" required>
            <option value="" disabled>{{ $t("knowledgeView.pick") }}</option>
            <option v-for="s in scopeOptions" :key="s.id" :value="s.id">{{ s.id }}</option>
          </select>
        </label>
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.col.task") }}</div>
          <input v-model="gTask" class="w-28 rounded border bg-background px-2 py-1 text-sm" />
        </label>
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.col.purpose") }}</div>
          <input v-model="gPurpose" class="w-36 rounded border bg-background px-2 py-1 text-sm" />
        </label>
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.grants.ttlHours") }}</div>
          <input v-model="gTtlHours" type="number" min="1" step="1" class="w-20 rounded border bg-background px-2 py-1 text-sm" />
        </label>
        <button
          type="submit"
          class="inline-flex items-center gap-1.5 rounded border px-2.5 py-1.5 text-sm hover:bg-accent"
          :disabled="busy || !gPrincipal || !gScope"
        >
          <Plus class="size-4" />{{ $t("knowledgeView.grants.create") }}
        </button>
        <span v-if="grantCreated" class="text-xs text-emerald-500">{{ $t("knowledgeView.grants.created") }}</span>
      </form>
    </section>

    <!-- Principals & tokens -->
    <section class="mb-8">
      <h3 class="mb-2 font-medium">{{ $t("knowledgeView.principals.title") }}</h3>
      <table class="mb-3 w-full text-sm">
        <thead>
          <tr class="border-b text-left text-muted-foreground">
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.id") }}</th>
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.type") }}</th>
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.displayName") }}</th>
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.level") }}</th>
            <th class="py-1.5 pr-3">token</th>
            <th class="py-1.5"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="p in overview?.principals ?? []" :key="p.id" class="border-b">
            <td class="py-1.5 pr-3 font-mono text-xs">{{ p.id }}</td>
            <td class="py-1.5 pr-3">{{ p.principal_type }}</td>
            <td class="py-1.5 pr-3">{{ p.display_name || "—" }}</td>
            <td class="py-1.5 pr-3">
              <select
                class="rounded border bg-background px-1.5 py-0.5 text-xs"
                :value="clearanceValue(p)"
                :disabled="busy"
                @change="setClearance(p.id, ($event.target as HTMLSelectElement).value)"
              >
                <option value="none">{{ $t("knowledgeView.principals.noClearance") }}</option>
                <option v-for="lv in LEVELS" :key="lv" :value="lv">{{ lv }}</option>
              </select>
            </td>
            <td class="py-1.5 pr-3">
              <span :class="p.token_hash ? 'text-emerald-500' : 'text-muted-foreground'">
                {{ p.token_hash ? $t("knowledgeView.principals.hasToken") : $t("knowledgeView.principals.noToken") }}
              </span>
            </td>
            <td class="py-1.5 text-right">
              <button
                class="mr-1 inline-flex items-center gap-1 rounded border px-2 py-0.5 text-xs hover:bg-accent"
                :disabled="busy"
                @click="issueToken(p.id)"
              >
                <KeyRound class="size-3.5" />{{ p.token_hash ? $t("knowledgeView.principals.rotate") : $t("knowledgeView.principals.issue") }}
              </button>
              <button
                v-if="p.token_hash"
                class="inline-flex items-center gap-1 rounded border px-2 py-0.5 text-xs hover:bg-accent"
                :disabled="busy"
                @click="revokeToken(p.id)"
              >
                <ShieldX class="size-3.5" />{{ $t("knowledgeView.principals.revokeToken") }}
              </button>
            </td>
          </tr>
        </tbody>
      </table>

      <form class="flex flex-wrap items-end gap-2 rounded border p-3" @submit.prevent="createPrincipal">
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.col.id") }}</div>
          <input v-model="pId" class="w-40 rounded border bg-background px-2 py-1 font-mono text-sm" placeholder="ai:carol / user-dave" required />
        </label>
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.col.displayName") }}</div>
          <input v-model="pName" class="w-32 rounded border bg-background px-2 py-1 text-sm" />
        </label>
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.col.type") }}</div>
          <select v-model="pType" class="w-32 rounded border bg-background px-2 py-1 text-sm">
            <option value="human">human</option>
            <option value="ai_employee">ai_employee</option>
          </select>
        </label>
        <label class="text-xs text-muted-foreground">
          <div>employee_id</div>
          <input v-model="pEmployee" class="w-28 rounded border bg-background px-2 py-1 font-mono text-sm" />
        </label>
        <button
          type="submit"
          class="inline-flex items-center gap-1.5 rounded border px-2.5 py-1.5 text-sm hover:bg-accent"
          :disabled="busy || !pId.trim()"
        >
          <Plus class="size-4" />{{ $t("knowledgeView.principals.create") }}
        </button>
      </form>
    </section>

    <!-- token 明文（僅一次） -->
    <div v-if="issuedToken" class="fixed inset-0 z-50 flex items-center justify-center bg-black/40" @click.self="issuedToken = null">
      <div class="w-[28rem] rounded border bg-background p-4">
        <h3 class="mb-2 font-medium">{{ $t("knowledgeView.token.title", { principal: issuedToken.principal }) }}</h3>
        <p class="mb-2 text-xs text-amber-500">{{ $t("knowledgeView.token.once") }}</p>
        <code class="mb-3 block break-all rounded bg-muted p-2 font-mono text-xs">{{ issuedToken.token }}</code>
        <div class="flex justify-end gap-2">
          <button class="rounded border px-2.5 py-1 text-sm hover:bg-accent" @click="copyToken">
            {{ $t("knowledgeView.token.copy") }}
          </button>
          <button class="rounded border px-2.5 py-1 text-sm hover:bg-accent" @click="issuedToken = null">
            {{ $t("common.close") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
