import { invoke } from "@tauri-apps/api/core";

export type DeviceMode =
  | "Android"
  | "Recovery"
  | "Fastboot"
  | "FastbootD"
  | "Fastboot Unknown"
  | "ADB Sideload"
  | "ADB Unauthorized"
  | "ADB Offline"
  | "Disconnected";

export type BootLayout = "single" | "ab" | "unknown";
export type SlotStrategy = "active" | "both";
export type RestoreStrategy =
  | "google_play"
  | "source_manager"
  | "local_apk_backup"
  | "manual"
  | "skip";

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

export interface OperationStatus {
  active: boolean;
  kind: string | null;
  serial: string | null;
  startedUnixMs: number | null;
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
  kind:
    | "preflight"
    | "mode_transition"
    | "revalidate_step"
    | "flash_preview"
    | "post_write_check"
    | "finish"
    | string;
  mode: string | null;
  partition: string | null;
  image: string | null;
  commandPreview: string | null;
  description: string;
  policyClass: string | null;
}

export interface ExecutionPreview {
  finalPlan: FinalFlashPlan;
  actions: ExecutionPreviewAction[];
  blockedReason: string | null;
  automaticExecutionEnabled: boolean;
  orderingPolicy: string;
  orderingPolicyComplete: boolean;
  diagnostic: string;
}

export interface ExecutionGuardStep {
  index: number;
  image: string;
  imagePath: string;
  partition: string;
  requiredMode: string;
  policyClass: string;
  imageSize: number;
  sha256: string;
}

export interface ExecutionGuardReport {
  finalPlan: FinalFlashPlan;
  orderingPolicy: string;
  steps: ExecutionGuardStep[];
  stateStableDuringHashing: boolean;
  readyForExecutor: boolean;
  diagnostic: string;
}

export interface FullRomStepResult {
  index: number;
  image: string;
  partition: string;
  requiredMode: string;
  status: "pending" | "running" | "success" | "failed" | string;
  command: string | null;
  exitCode: number | null;
  diagnostic: string;
}

export interface FullRomExecutionReport {
  operationId: string;
  success: boolean;
  journalPath: string;
  steps: FullRomStepResult[];
  cleanDataPerformed: boolean;
  rebootRequested: boolean;
  diagnostic: string;
}

export interface RestoreApp {
  packageName: string;
  installerPackage: string | null;
  sourceKind: string;
  restoreStrategy: RestoreStrategy;
  enabledByDefault: boolean;
}

export interface RestoreProfileCounts {
  total: number;
  googlePlay: number;
  sourceManager: number;
  localApkBackup: number;
}

export interface RestoreProfile {
  version: number;
  serial: string;
  deviceProduct: string | null;
  androidRelease: string | null;
  sdkLevel: string | null;
  apps: RestoreApp[];
  counts: RestoreProfileCounts;
  diagnostic: string;
}

export interface RestoreProfileConfigApp {
  packageName: string;
  installerPackage: string | null;
  sourceKind: string;
  restoreStrategy: RestoreStrategy;
  enabled: boolean;
}

export interface RestoreProfileConfig {
  version: number;
  deviceProduct: string | null;
  androidRelease: string | null;
  sdkLevel: string | null;
  apps: RestoreProfileConfigApp[];
}

export interface RestoreProfileSaveResult {
  path: string;
  appCount: number;
  diagnostic: string;
}

export interface ApkBackupFile {
  remotePath: string;
  localPath: string;
  size: number;
  sha256: string;
}

export interface ApkBackupPackageResult {
  packageName: string;
  success: boolean;
  files: ApkBackupFile[];
  diagnostic: string;
}

export interface ApkBackupReport {
  destination: string;
  packages: ApkBackupPackageResult[];
  successCount: number;
  failureCount: number;
  totalFiles: number;
  diagnostic: string;
}

export interface LocalRestorePackageResult {
  packageName: string;
  success: boolean;
  apkCount: number;
  command: string | null;
  diagnostic: string;
}

export interface LocalRestoreReport {
  sourceDirectory: string;
  packages: LocalRestorePackageResult[];
  successCount: number;
  failureCount: number;
  diagnostic: string;
}

export interface RestoreVerification {
  expectedCount: number;
  installedCount: number;
  missingCount: number;
  installed: string[];
  missing: string[];
  diagnostic: string;
}

export type RebootTarget = "android" | "bootloader" | "fastbootd" | "recovery";

export function detectDevice(): Promise<DeviceSnapshot> {
  return invoke<DeviceSnapshot>("detect_device");
}

export function listDevices(): Promise<DeviceSnapshot[]> {
  return invoke<DeviceSnapshot[]>("list_devices");
}

export function rebootDevice(target: RebootTarget, serial: string): Promise<ActionResult> {
  return invoke<ActionResult>("reboot_device", { target, serial });
}

export function bootTwrp(imagePath: string, serial: string): Promise<ActionResult> {
  return invoke<ActionResult>("boot_twrp", { imagePath, serial });
}

export function factoryReset(confirmation: string, serial: string): Promise<ActionResult> {
  return invoke<ActionResult>("factory_reset", { confirmation, serial });
}

export function getOperationStatus(): Promise<OperationStatus> {
  return invoke<OperationStatus>("get_operation_status");
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

export function buildExecutionGuard(options: {
  path: string;
  serial: string;
  slotStrategy: SlotStrategy;
}): Promise<ExecutionGuardReport> {
  return invoke<ExecutionGuardReport>("build_execution_guard", options);
}

export function executeFullRom(options: {
  path: string;
  serial: string;
  slotStrategy: SlotStrategy;
  confirmation: string;
  cleanDataAfter: boolean;
  rebootAfter: boolean;
}): Promise<FullRomExecutionReport> {
  return invoke<FullRomExecutionReport>("execute_full_rom", options);
}

export function scanRestoreProfile(serial: string): Promise<RestoreProfile> {
  return invoke<RestoreProfile>("scan_restore_profile", { serial });
}

export function saveRestoreProfile(
  directory: string,
  profile: RestoreProfileConfig,
): Promise<RestoreProfileSaveResult> {
  return invoke<RestoreProfileSaveResult>("save_restore_profile", { directory, profile });
}

export function loadRestoreProfile(directory: string): Promise<RestoreProfileConfig> {
  return invoke<RestoreProfileConfig>("load_restore_profile", { directory });
}

export function backupRestoreApks(options: {
  serial: string;
  destination: string;
  packages: string[];
}): Promise<ApkBackupReport> {
  return invoke<ApkBackupReport>("backup_restore_apks", options);
}

export function restoreLocalApks(options: {
  serial: string;
  sourceDirectory: string;
  packages: string[];
}): Promise<LocalRestoreReport> {
  return invoke<LocalRestoreReport>("restore_local_apks", options);
}

export function verifyRestorePackages(
  serial: string,
  expectedPackages: string[],
): Promise<RestoreVerification> {
  return invoke<RestoreVerification>("verify_restore_packages", {
    serial,
    expectedPackages,
  });
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
