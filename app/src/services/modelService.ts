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

/** Whether a manifest model group is present and verified (ipc/setup.rs:275). */
export function checkModelExists(modelId: string): Promise<boolean> {
  return invoke("check_model_exists", { modelId });
}

/** Download an optional model group in the background (ipc/setup.rs:362). */
export function downloadOptionalModel(modelId: string): Promise<void> {
  return invoke("download_optional_model", { modelId });
}

/** Delete a model group from disk (ipc/setup.rs:429). */
export function deleteModel(modelId: string): Promise<void> {
  return invoke("delete_model", { modelId });
}

/** Start the model setup wizard downloads (ipc/setup.rs:57). */
export function startModelSetup(selectedIds: string[]): Promise<void> {
  return invoke("start_model_setup", { selectedIds });
}

/** Cancel an in-flight model setup (ipc/setup.rs:165). */
export function cancelModelSetup(): Promise<void> {
  return invoke("cancel_model_setup");
}

/** Mark the setup wizard as completed (ipc/setup.rs:177). */
export function completeSetupWizard(): Promise<void> {
  return invoke("complete_setup_wizard");
}

/** Bring the wizard window to the front (ipc/setup.rs:420). */
export function revealWizard(): Promise<void> {
  return invoke("reveal_wizard");
}

/** App update check (ipc/setup.rs:11). */
export function checkForUpdates(): Promise<UpdateReport> {
  return invoke("check_for_updates");
}

/** Model manifest update check (ipc/setup.rs:19). */
export function checkForModelUpdates(): Promise<ModelUpdateReport> {
  return invoke("check_for_model_updates");
}

/** System validation report for the wizard (ipc/setup.rs:47). */
export function getRuntimeReport(): Promise<RuntimeReport> {
  return invoke("get_runtime_report");
}

/** Whether the setup wizard has been completed (ipc/setup.rs:171). */
export function getOnboardingStatus(): Promise<boolean> {
  return invoke("get_onboarding_status");
}
