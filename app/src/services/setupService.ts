import { invoke } from "@tauri-apps/api/core";

/**
 * Mirror of `VoxManifest` / `ModelGroup` / `ModelEntry`
 * (setup/manifest.rs). Note the serde renames: `size_bytes` → `size`,
 * `archive_type` → `archive`.
 */
export interface ModelEntry {
  id: string;
  path: string;
  size: number;
  sha256: string;
  archive: string | null;
  required: boolean;
}

export interface ModelGroup {
  id: string;
  name: string;
  category: string;
  version: string;
  files: ModelEntry[];
}

export interface VoxManifest {
  models_version: string;
  release_notes?: string[] | null;
  total_size_bytes: number;
  model_groups: ModelGroup[];
}

/** Mirror of `UpdateReport` (setup/update_check.rs:5). */
export interface UpdateReport {
  current_version: string;
  latest_version: string;
  update_available: boolean;
  release_notes: string[];
  update_command: string;
}

/** Mirror of `ModelUpdateReport` (setup/update_check.rs:93). */
export interface ModelUpdateReport {
  local_version: string;
  remote_version: string;
  update_available: boolean;
  outdated_models: string[];
}

/** Mirror of `RuntimeReport` (setup/runtime_check.rs:9). */
export interface RuntimeReport {
  write_access: boolean;
  available_space_gb: number;
  total_space_gb: number;
  required_space_gb: number;
  disk_space_ok: boolean;
  mic_access: boolean;
  ram_gb: number;
  cpu_cores: number;
  settings_exists: boolean;
  models_dir_exists: boolean;
  models_dir: string;
  models_missing: string[];
  models_verified: boolean;
  setup_completed: boolean;
}

/** Fetch the model manifest (ipc/setup.rs:27). */
export function fetchManifest(): Promise<VoxManifest> {
  return invoke("fetch_manifest");
}

/** Whether a manifest model group is present and verified. */
export function checkModelExists(modelId: string): Promise<boolean> {
  return invoke<boolean>("manage_models", { payload: { action: "exists", model_id: modelId } });
}

/** Download an optional model group in the background. */
export function downloadOptionalModel(modelId: string): Promise<void> {
  return invoke<void>("manage_models", { payload: { action: "download", model_id: modelId } });
}

/** Delete a model group from disk. */
export function deleteModel(modelId: string): Promise<void> {
  return invoke<void>("manage_models", { payload: { action: "delete", model_id: modelId } });
}

/** Start the model setup wizard downloads. */
export function startModelSetup(selectedIds: string[]): Promise<void> {
  return invoke<void>("manage_models", { payload: { action: "start_setup", selected_ids: selectedIds } });
}

/** Cancel an in-flight model setup. */
export function cancelModelSetup(): Promise<void> {
  return invoke<void>("manage_models", { payload: { action: "cancel" } });
}

/** Mark the setup wizard as completed (ipc/settings.rs). */
export function completeSetupWizard(): Promise<void> {
  return invoke("complete_setup_wizard");
}

/** Bring the wizard window to the front. */
export function revealWizard(): Promise<void> {
  return invoke("reveal_wizard");
}

/** Unified update check result. */
export interface UnifiedUpdateReport {
  app?: UpdateReport | null;
  models?: ModelUpdateReport | null;
}

/** Check for updates across application, models, or both. */
export function checkUpdates(scope: "app" | "models" | "all" = "all"): Promise<UnifiedUpdateReport> {
  return invoke("check_updates", { scope });
}

/** App update check. */
export async function checkForUpdates(): Promise<UpdateReport> {
  const res = await checkUpdates("app");
  return res.app!;
}

/** Model manifest update check. */
export async function checkForModelUpdates(): Promise<ModelUpdateReport> {
  const res = await checkUpdates("models");
  return res.models!;
}

/** System validation report for the wizard (ipc/setup.rs:47). */
export function getRuntimeReport(): Promise<RuntimeReport> {
  return invoke("get_runtime_report");
}

/** Whether the setup wizard has been completed (ipc/setup.rs:171). */
export function getOnboardingStatus(): Promise<boolean> {
  return invoke("get_onboarding_status");
}
