# FlashROM

FlashROM is a Windows-first Android flashing utility built with **Rust + Tauri 2 + React + TypeScript**. The goal is to replace repetitive ADB/Fastboot command-line work with a safer GUI that keeps the executed commands and device state visible.

> [!WARNING]
> Flashing Android partitions can permanently erase data or make a device unbootable. FlashROM should validate device state, ROM compatibility, partition targets, and bootloader status before destructive operations are enabled.

## Initial scope

The first milestone focuses on safe device-management primitives:

- Detect devices connected through ADB or Fastboot.
- Distinguish Android, Bootloader/Fastboot, and FastbootD when possible.
- Detect single-slot vs A/B boot layouts.
- Map single-slot devices to `boot`, and A/B devices to `boot_a` / `boot_b`.
- Allow a manual boot-layout override when device metadata is unreliable.
- Read basic Fastboot metadata such as current slot and product.
- Accept a TWRP `.img` and ROM package/folder through native Tauri drag-and-drop.
- Reboot between Android, Bootloader, FastbootD, and Recovery.
- Keep ADB/Fastboot process execution isolated in the Rust backend.
- Add flashing only after validation and flash-plan layers are in place.

## Stack

- Tauri 2
- Rust
- React 19
- TypeScript
- Vite
- pnpm

## Development prerequisites (Windows)

1. Install Microsoft C++ Build Tools / Visual Studio Build Tools with Desktop development with C++.
2. Install Rust using rustup and use the MSVC toolchain.
3. Install Node.js LTS.
4. Enable pnpm through Corepack or install pnpm directly.
5. Install Android SDK Platform Tools, or place them under `tools/platform-tools/`.

```powershell
rustup default stable-msvc
corepack enable
pnpm install
pnpm tauri dev
```

FlashROM resolves `adb` and `fastboot` in this order:

1. Directory specified by `FLASHROM_PLATFORM_TOOLS`.
2. `tools/platform-tools/` inside the project working directory.
3. System `PATH`.

Example:

```powershell
$env:FLASHROM_PLATFORM_TOOLS="D:\Android\platform-tools"
pnpm tauri dev
```

## Boot partition detection

FlashROM tries to identify the boot layout automatically.

In Fastboot it prefers:

```text
fastboot getvar has-slot:boot
fastboot getvar current-slot
```

In Android/Recovery through ADB it checks slot-related properties such as `ro.boot.slot_suffix`, `ro.boot.slot`, and `ro.build.ab_update`.

The resulting targets are:

```text
Single-slot
└── boot

A/B
├── boot_a
└── boot_b
```

The UI also exposes `Auto detect`, `1 partition`, and `2 partitions (A/B)` so the layout can be overridden before any future flash command is generated.

## Flash inputs

The desktop UI currently accepts native file drops:

```text
TWRP
└── *.img

ROM
└── ROM file or folder
```

Dropping these inputs only stores their local paths. It does **not** execute `fastboot flash`, sideload, erase, or any other destructive command yet.

## Architecture

```text
React / TypeScript UI
        |
        | Tauri invoke + native drag/drop
        v
Rust commands
        |
        +-- Android device detection
        +-- single-slot / A-B detection
        +-- ADB/Fastboot process runner
        +-- device-state validation (next)
        +-- ROM analyzer (next)
        +-- flash planner (next)
        |
        v
adb / fastboot
```

## Roadmap

### v0.1 - Device foundation

- [x] Project bootstrap
- [x] Device detection
- [x] ADB/Fastboot mode detection
- [x] Single-slot / A-B boot detection
- [x] Manual boot-layout override
- [x] TWRP drag-and-drop input
- [x] ROM drag-and-drop input
- [x] Reboot actions
- [ ] Realtime process streaming

### v0.2 - Safe manual flashing

- [ ] Inspect dropped TWRP / ROM inputs
- [ ] Read partition/device metadata
- [ ] Validate target partition
- [ ] Choose active/both slots for A/B devices
- [ ] Command preview and confirmation
- [ ] Flash progress and result handling

### v0.3 - ROM flash wizard

- [ ] Scan ROM folder
- [ ] Detect common image layouts
- [ ] Generate a flash plan
- [ ] Validate device codename / A-B slot / dynamic partitions
- [ ] Execute and verify the plan

### Later

- `payload.bin` extraction workflow
- `super.img` / dynamic partition tooling
- ADB sideload
- Backup/restore helpers
- Device profiles
- Release builds and auto-update

## License

A license has not been selected yet.
