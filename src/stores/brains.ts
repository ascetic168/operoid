import { defineStore } from "pinia";
import { ref } from "vue";
import {
  brainsList,
  brainsAdd,
  brainsRemove,
  brainsSetActive,
  brainsSetActiveSource,
  brainSources,
  brainSourceAdd,
  brainSourceRemove,
  DEFAULT_BRAIN_ID,
  formatError,
  type BrainEntry,
  type GbrainSource,
  type AddBrainReq,
} from "@/lib/tauri";
import { useConfigStore } from "@/stores/config";

/** 腦管理 store：腦清單（registry）+ 作用中腦/來源 + 各腦 sources（live）。 */
export const useBrainsStore = defineStore("brains", () => {
  const brains = ref<BrainEntry[]>([]);
  const activeId = ref<string | null>(null);
  const activeDotGbrain = ref<string | null>(null);
  const activeSourceId = ref<string | null>(null);

  // BrainsView 右欄正在檢視的腦（預設=作用中腦）與其 sources。
  const selectedBrainId = ref<string | null>(null);
  const sources = ref<GbrainSource[]>([]);
  const sourcesLoading = ref(false);
  const sourcesError = ref<string | null>(null);

  async function load() {
    const l = await brainsList();
    brains.value = l.brains;
    activeId.value = l.active_id;
    activeDotGbrain.value = l.active_dot_gbrain;
    if (!selectedBrainId.value) selectedBrainId.value = l.active_id;
    // active_source_id 由 app config 提供（透過 config store）；這裡不重覆管。
    if (selectedBrainId.value) await loadSources(selectedBrainId.value);
  }

  /** 切作用中腦：寫回後端 + 重整設定頁 config + 重載該腦 sources + 重設來源。 */
  async function setActive(id: string) {
    await brainsSetActive(id);
    activeId.value = id;
    activeSourceId.value = null;
    await brainsSetActiveSource(null);
    selectedBrainId.value = id;
    const l = await brainsList();
    activeDotGbrain.value = l.active_dot_gbrain;
    // 作用中腦變了 → 設定頁的 config、工廠的 LLM 端點都要反映新腦
    const config = useConfigStore();
    await Promise.all([config.loadGbrain(), config.loadApp()]);
    await loadSources(id);
  }

  async function add(req: AddBrainReq) {
    const b = await brainsAdd(req);
    brains.value = [...brains.value, b];
    return b;
  }

  async function remove(id: string) {
    await brainsRemove(id);
    brains.value = brains.value.filter((b) => b.id !== id);
    if (activeId.value === id) {
      await setActive(DEFAULT_BRAIN_ID);
    }
    if (selectedBrainId.value === id) selectedBrainId.value = activeId.value;
  }

  async function loadSources(brainId: string) {
    selectedBrainId.value = brainId;
    sourcesLoading.value = true;
    sourcesError.value = null;
    try {
      sources.value = await brainSources(brainId);
      // 檢視的是「作用中腦」且尚未選來源 → 預設選 default(否則第一個),
      // 讓工廠/sync 一上來就有目標(在 BrainsView 檢視其他腦時不會覆蓋作用中來源)。
      if (brainId === activeId.value && !activeSourceId.value) {
        const def = sources.value.find((s) => s.id === "default") ?? sources.value[0];
        if (def) {
          activeSourceId.value = def.id;
          void brainsSetActiveSource(def.id);
        }
      }
    } catch (e) {
      sourcesError.value = formatError(e);
      sources.value = [];
    } finally {
      sourcesLoading.value = false;
    }
  }

  async function addSource(brainId: string, sourceId: string, path: string) {
    await brainSourceAdd(brainId, sourceId, path);
    await loadSources(brainId);
  }

  async function removeSource(brainId: string, sourceId: string) {
    await brainSourceRemove(brainId, sourceId);
    if (activeSourceId.value === sourceId) {
      activeSourceId.value = null;
      await brainsSetActiveSource(null);
    }
    await loadSources(brainId);
  }

  /** 設作用中來源（作用中腦內）。 */
  async function setActiveSource(sourceId: string | null) {
    activeSourceId.value = sourceId;
    await brainsSetActiveSource(sourceId);
  }

  return {
    brains,
    activeId,
    activeDotGbrain,
    activeSourceId,
    selectedBrainId,
    sources,
    sourcesLoading,
    sourcesError,
    load,
    setActive,
    add,
    remove,
    loadSources,
    addSource,
    removeSource,
    setActiveSource,
  };
});
