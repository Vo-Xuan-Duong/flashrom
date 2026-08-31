import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import {
  backupSourceManagerConfig,
  inspectSourceManagerVault,
  inspectSpecialTools,
  preparePayloadInput,
  prepareSuperInput,
  stageSourceManagerConfig,
} from "./beta";

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

describe("beta ROM preparation IPC", () => {
  it("passes payload source and isolated workspace unchanged", async () => {
    await preparePayloadInput("D:\\ROM\\payload.bin", "D:\\Work\\payload");
    expect(invoke).toHaveBeenCalledWith("prepare_payload_input", {
      source: "D:\\ROM\\payload.bin",
      destination: "D:\\Work\\payload",
    });
  });

  it("passes super source and isolated workspace unchanged", async () => {
    await prepareSuperInput("D:\\ROM\\super.img", "D:\\Work\\super");
    expect(invoke).toHaveBeenCalledWith("prepare_super_input", {
      source: "D:\\ROM\\super.img",
      destination: "D:\\Work\\super",
    });
  });

  it("uses a read-only special tool diagnostic command", async () => {
    await inspectSpecialTools();
    expect(invoke).toHaveBeenCalledWith("inspect_special_tools");
  });
});

describe("source manager vault IPC", () => {
  it("pins an explicitly selected export into the chosen workspace", async () => {
    await backupSourceManagerConfig(
      "D:\\FlashROM-Backup",
      "obtainium",
      "D:\\Exports\\obtainium.json",
    );
    expect(invoke).toHaveBeenCalledWith("backup_source_manager_config", {
      workspace: "D:\\FlashROM-Backup",
      manager: "obtainium",
      sourcePath: "D:\\Exports\\obtainium.json",
    });
  });

  it("loads the vault without a device", async () => {
    await inspectSourceManagerVault("D:\\FlashROM-Backup");
    expect(invoke).toHaveBeenCalledWith("inspect_source_manager_vault", {
      workspace: "D:\\FlashROM-Backup",
    });
  });

  it("keeps serial, manager and exact staging confirmation in the backend request", async () => {
    await stageSourceManagerConfig({
      serial: "ABC123",
      workspace: "D:\\FlashROM-Backup",
      manager: "fdroid",
      confirmation: "STAGE CONFIG",
    });
    expect(invoke).toHaveBeenCalledWith("stage_source_manager_config", {
      serial: "ABC123",
      workspace: "D:\\FlashROM-Backup",
      manager: "fdroid",
      confirmation: "STAGE CONFIG",
    });
  });
});
