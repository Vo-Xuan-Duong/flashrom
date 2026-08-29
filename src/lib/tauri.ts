import { invoke } from "@tauri-apps/api/core";

export type DeviceMode =
  | "Android"
  | "Recovery"
  | "Fastboot"
  | "FastbootD"
  | "ADB Sideload"
  | "ADB Unauthorized"
  | "ADB Offline"
  | "Disconnected";

export type BootLayout = "single" | "ab" | "unknown";
export type SlotStrategy = "active" | "both";

export interface DeviceSnapshot {
  connected: boolean;
  serial: string | null;
  mode: DeviceMode | string;
  slot: string | null;
  product: string | null;
  tool: "adb" | "fastboot" | null;
  bootLayout: BootLayout;
  bootPartitions: string[];
  diagnostic: string;
}

export interface ActionResult {
  command: string;
  success: boolean;
  status: number;
  output: string;
}

export interface FlashExecutionResult extends ActionResult {
  partition: string;
  imageSize: number;
  partitionSize: number;
  requiredMode: string;
  product: string | null;
}

export interface ProcessOutputEvent {
  operationId: string;
  stream: "stdout" | "stderr" | string;
  data: string;
}

export interface RomArtifact {
  name: string;
  path: string;
  kind: string;
  size: number;
}

export interface RomInspection {
  path: string;
  kind: string;
  size: number;
  artifacts: RomArtifact[];
  diagnostic: string;
}

export interface FlashPlanStep {
  image: string;
  imagePath: string;
  partition: string;
  requiredMode: "Fastboot" | "FastbootD" | "Unknown" | string;
  commandPreview: string;
  state:
    | "resolved"
    | "blocked"
    | "needs_partition_metadata"
    | "needs_compatibility_check"
    | "unsupported"
    | string;
  warning: string | null;
}

export interface FlashPlan {
  romKind: string;
  bootLayout: BootLayout;
  slotStrategy: SlotStrategy;
  activeSlot: string | null;
  steps: FlashPlanStep[];
  warnings: string[];
  readyForValidation: boolean;
}

export interface PartitionTargetMetadata {
  name: string;
  logical: boolean | null;
  sizeBytes: number | null;
  partitionType: string | null;
  recommendedMode: "Fastboot" | "FastbootD" | "Unknown" | string;
}

export interface PartitionMetadata {
  basePartition: string;
  hasSlot: boolean | null;
  targets: PartitionTargetMetadata[];
  diagnostic: string;
}

export interface RomProductEvidence {
  product: string;
  source: string;
  key: string;
}

export interface RomCompatibility {
  deviceProduct: string | null;
  romProducts: string[];
  evidence: RomProductEvidence[];
  status: "matched" | "mismatch" | "unknown" | string;
  safeToAutoFlash: boolean;
  diagnostic: string;
}

export interface FinalFlashPlanStep {
  image: string;
  imagePath: string;
  imageSize: number;
  basePartition: string;
  partition: string;
  partitionSize: number | null;
  logical: boolean | null;
  requiredMode: "Fastboot" | "FastbootD" | "Unknown" | string;
  phase: number;
  state: "ready" | "blocked" | "manual_only" | string;
  commandPreview: string;
  warning: string | null;
}

export interface FinalFlashPlan {
  compatibility: RomCompatibility;
  activeSlot: string | null;
  slotStrategy: SlotStrategy;
  bootloaderUnlocked: boolean | null;
  snapshotUpdateStatus: string | null;
  currentMode: string;
  steps: FinalFlashPlanStep[];
  warnings: string[];
  requiresModeSwitch: boolean;
  readyForExecution: boolean;
}

export interface ExecutionPreviewAction {
  index: number;
  kind: "preflight" | "mode_transition" | "revalidate_step" | "flash_preview" | "post_write_check" | "finish" | string;
  mode: string | null;
  partition: string | null;
  image: string | null;
  policyClass: "boot_chain" | "system_payload" | "avb_metadata" | string | null;
  commandPreview: string | null;
  description: string;
}

export interface ExecutionPreview {
  finalPlan: FinalFlashPlan;
  actions: ExecutionPreviewAction[];
  orderingPolicy: string;
  orderingPolicyComplete: boolean;
  blockedReason: string | null;
  automaticExecutionEnabled: boolean;
  diagnostic: string;
}

export type RebootTarget = "android" | "bootloader" | "fastbootd" | "recovery";

export function detectDevice(): Promise<DeviceSnapshot> {
  return invoke<DeviceSnapshot>("detect_device");
}

export function rebootDevice(target: RebootTarget): Promise<ActionResult> {
  return invoke<ActionResult>("reboot_device", { target });
}

export function bootTwrp(imagePath: string): Promise<ActionResult> {
  return invoke<ActionResult>("boot_twrp", { imagePath });
}

export function factoryReset(confirmation: string): Promise<ActionResult> {
  return invoke<ActionResult>("factory_reset", { confirmation });
}

export function inspectRom(path: string): Promise<RomInspection> {
  return invoke<RomInspection>("inspect_rom", { path });
}

export function generateFlashPlan(options: {
  path: string;
  bootLayout: BootLayout;
  activeSlot: string | null;
  slotStrategy: SlotStrategy;
  serial: string | null;
}): Promise<FlashPlan> {
  return invoke<FlashPlan>("generate_flash_plan", options);
}

export function inspectPartitions(
  serial: string,
  partitions: string[],
): Promise<PartitionMetadata[]> {
  return invoke<PartitionMetadata[]>("inspect_partitions", { serial, partitions });
}

export function inspectRomCompatibility(
  path: string,
  serial: string,
): Promise<RomCompatibility> {
  return invoke<RomCompatibility>("inspect_rom_compatibility", { path, serial });
}

export function resolveFinalFlashPlan(options: {
  path: string;
  serial: string;
  slotStrategy: SlotStrategy;
}): Promise<FinalFlashPlan> {
  return invoke<FinalFlashPlan>("resolve_final_flash_plan", options);
}

export function buildExecutionPreview(options: {
  path: string;
  serial: string;
  slotStrategy: SlotStrategy;
}): Promise<ExecutionPreview> {
  return invoke<ExecutionPreview>("build_execution_preview", options);
}

export function adbSideload(serial: string, zipPath: string): Promise<ActionResult> {
  return invoke<ActionResult>("adb_sideload", { serial, zipPath });
}

export function flashImage(options: {
  serial: string;
  partition: string;
  imagePath: string;
  confirmation: string;
}): Promise<FlashExecutionResult> {
  return invoke<FlashExecutionResult>("flash_image", options);
}
