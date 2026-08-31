import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mocks.invoke,
}));

import {
  bootTwrp,
  executeFullRom,
  factoryReset,
  flashImage,
  listDevices,
  rebootDevice,
} from "./tauri";

beforeEach(() => {
  mocks.invoke.mockReset();
  mocks.invoke.mockResolvedValue({});
});

describe("serial-scoped destructive IPC", () => {
  it("passes the selected serial to reboot actions", async () => {
    await rebootDevice("fastbootd", "ABC123");
    expect(mocks.invoke).toHaveBeenCalledWith("reboot_device", {
      target: "fastbootd",
      serial: "ABC123",
    });
  });

  it("passes image path and serial for temporary TWRP boot", async () => {
    await bootTwrp("D:\\ROM\\twrp.img", "ABC123");
    expect(mocks.invoke).toHaveBeenCalledWith("boot_twrp", {
      imagePath: "D:\\ROM\\twrp.img",
      serial: "ABC123",
    });
  });

  it("keeps Factory Reset confirmation in the backend request", async () => {
    await factoryReset("WIPE", "ABC123");
    expect(mocks.invoke).toHaveBeenCalledWith("factory_reset", {
      confirmation: "WIPE",
      serial: "ABC123",
    });
  });

  it("sends the exact manual flash guard values", async () => {
    await flashImage({
      serial: "ABC123",
      partition: "boot_b",
      imagePath: "D:\\ROM\\boot.img",
      confirmation: "FLASH",
    });
    expect(mocks.invoke).toHaveBeenCalledWith("flash_image", {
      serial: "ABC123",
      partition: "boot_b",
      imagePath: "D:\\ROM\\boot.img",
      confirmation: "FLASH",
    });
  });

  it("sends Full-ROM safety options without changing their meaning", async () => {
    await executeFullRom({
      path: "D:\\ROM\\images",
      serial: "ABC123",
      slotStrategy: "active",
      confirmation: "FLASH ROM WIPE",
      cleanDataAfter: true,
      rebootAfter: true,
    });
    expect(mocks.invoke).toHaveBeenCalledWith("execute_full_rom", {
      path: "D:\\ROM\\images",
      serial: "ABC123",
      slotStrategy: "active",
      confirmation: "FLASH ROM WIPE",
      cleanDataAfter: true,
      rebootAfter: true,
    });
  });

  it("uses the explicit multi-device listing command", async () => {
    await listDevices();
    expect(mocks.invoke).toHaveBeenCalledWith("list_devices");
  });
});
