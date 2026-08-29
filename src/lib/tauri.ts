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

export type RebootTarget = "android" | "bootloader" | "fastbootd" | "recovery";

export function detectDevice(): Promise<DeviceSnapshot> {
  return invoke<DeviceSnapshot>("detect_device");
}

export function rebootDevice(target: RebootTarget): Promise<ActionResult> {
  return invoke<ActionResult>("reboot_device", { target });
}
