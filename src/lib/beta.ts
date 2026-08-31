import { invoke } from "@tauri-apps/api/core";
import type { DeviceSnapshot } from "./tauri";

export interface SpecialToolStatus {
  name: string;
  source: string;
  path: string;
  available: boolean;
  version: string | null;
  diagnostic: string;
}

export interface SpecialToolsStatus {
  payloadDumper: SpecialToolStatus;
  lpunpack: SpecialToolStatus;
  simg2img: SpecialToolStatus;
  payloadReady: boolean;
  superReady: boolean;
  diagnostic: string;
}

export interface PreparedArtifact {
  name: string;
  path: string;
  size: number;
}

export interface PreparedRomInput {
  source: string;
  destination: string;
  kind: string;
  artifacts: PreparedArtifact[];
  totalBytes: number;
  ignoredImageCount: number;
  diagnostic: string;
}

export type SourceManagerId = "obtainium" | "fdroid";

export interface SourceManagerConfigEntry {
  manager: SourceManagerId | string;
  packageName: string;
  label: string;
  fileName: string;
  localPath: string;
  size: number;
  sha256: string;
}

export interface SourceManagerManifest {
  version: number;
  entries: SourceManagerConfigEntry[];
}

export interface SourceManagerBackupResult {
  manifestPath: string;
  entry: SourceManagerConfigEntry;
  diagnostic: string;
}

export interface SourceManagerStageResult {
  manager: SourceManagerId | string;
  packageName: string;
  managerInstalled: boolean;
  remotePath: string;
  sha256: string;
  manualImportRequired: boolean;
  importHint: string;
  diagnostic: string;
}

export function inspectSpecialTools(): Promise<SpecialToolsStatus> {
  return invoke<SpecialToolsStatus>("inspect_special_tools");
}

export function preparePayloadInput(source: string, destination: string): Promise<PreparedRomInput> {
  return invoke<PreparedRomInput>("prepare_payload_input", { source, destination });
}

export function prepareSuperInput(source: string, destination: string): Promise<PreparedRomInput> {
  return invoke<PreparedRomInput>("prepare_super_input", { source, destination });
}

export function inspectSourceManagerVault(workspace: string): Promise<SourceManagerManifest> {
  return invoke<SourceManagerManifest>("inspect_source_manager_vault", { workspace });
}

export function backupSourceManagerConfig(
  workspace: string,
  manager: SourceManagerId,
  sourcePath: string,
): Promise<SourceManagerBackupResult> {
  return invoke<SourceManagerBackupResult>("backup_source_manager_config", {
    workspace,
    manager,
    sourcePath,
  });
}

export function stageSourceManagerConfig(options: {
  serial: string;
  workspace: string;
  manager: SourceManagerId;
  confirmation: string;
}): Promise<SourceManagerStageResult> {
  return invoke<SourceManagerStageResult>("stage_source_manager_config", options);
}

export function listBetaDevices(): Promise<DeviceSnapshot[]> {
  return invoke<DeviceSnapshot[]>("list_devices");
}
