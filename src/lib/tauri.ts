import { invoke } from "@tauri-apps/api/core";

export type DeviceMode =
  | "Android"
  | "Recovery"
  | "Fastboot"
  | "FastbootD"
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
