<script setup lang="ts">
import { onMounted, ref } from "vue";
import { KeyRound, Plus, RefreshCw, ShieldX } from "lucide-vue-next";
import { formatError, type KnowledgeOverview, knowledgeGrantCreate, knowledgeGrantRevoke, knowledgeOverviewLoad, knowledgePrincipalCreate, knowledgeTokenIssue, knowledgeTokenRevoke } from "@/lib/tauri";

const overview = ref<KnowledgeOverview | null>(null);
const loadError = ref<string | null>(null);
const actionError = ref<string | null>(null);
const busy = ref(false);

// grant 建立表單
const gPrincipal = ref("");
const gScope = ref("");
const gTask = ref("");
const gPurpose = ref("");
const gTtlHours = ref("24");
const grantCreated = ref(false);

// principal 建立表單
const pId = ref("");
const pName = ref("");
const pEmployee = ref("");
const pType = ref<"human" | "ai_employee">("human");

// token 明文（只顯示一次）
const issuedToken = ref<{ principal: string; token: string } | null>(null);

async function refresh() {
  loadError.value = null;
  try {
    overview.value = await knowledgeOverviewLoad();
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

function copyToken() {
  if (issuedToken.value) void navigator.clipboard?.writeText(issuedToken.value.token);
}
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

    <!-- Scopes（唯讀參考） -->
    <section class="mb-8">
      <h3 class="mb-2 font-medium">{{ $t("knowledgeView.scopes.title") }}</h3>
      <table class="w-full text-sm">
        <thead>
          <tr class="border-b text-left text-muted-foreground">
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.id") }}</th>
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.visibility") }}</th>
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.classification") }}</th>
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
            <option v-for="p in overview?.principals ?? []" :key="p.id" :value="p.id">{{ p.id }}</option>
          </select>
        </label>
        <label class="text-xs text-muted-foreground">
          <div>{{ $t("knowledgeView.col.scope") }}</div>
          <select v-model="gScope" class="w-40 rounded border bg-background px-2 py-1 text-sm" required>
            <option value="" disabled>{{ $t("knowledgeView.pick") }}</option>
            <option v-for="s in overview?.scopes ?? []" :key="s.id" :value="s.id">{{ s.id }}</option>
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
            <th class="py-1.5 pr-3">{{ $t("knowledgeView.col.departments") }}</th>
            <th class="py-1.5 pr-3">token</th>
            <th class="py-1.5"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="p in overview?.principals ?? []" :key="p.id" class="border-b">
            <td class="py-1.5 pr-3 font-mono text-xs">{{ p.id }}</td>
            <td class="py-1.5 pr-3">{{ p.principal_type }}</td>
            <td class="py-1.5 pr-3">{{ p.display_name || "—" }}</td>
            <td class="py-1.5 pr-3 text-xs">{{ p.attrs?.departments?.join(", ") || "—" }}</td>
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
