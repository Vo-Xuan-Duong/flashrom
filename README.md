# FlashROM

FlashROM is a Windows-first Android flashing utility built with **Rust + Tauri 2 + React + TypeScript**. The goal is to replace repetitive ADB/Fastboot command-line work with a safer GUI that keeps the executed commands and device state visible.

> [!WARNING]
> Flashing Android partitions and wiping userdata can permanently erase data or make a device unbootable. FlashROM validates device state and requires explicit confirmation before destructive operations are enabled.

## Current capabilities

- Detect devices connected through ADB or Fastboot.
- Distinguish Android, Recovery, Bootloader/Fastboot, and FastbootD when possible.
- Detect single-slot vs A/B boot layouts.
- Map single-slot devices to `boot`, and A/B devices to `boot_a` / `boot_b`.
- Allow a manual boot-layout override when device metadata is unreliable.
- Read basic Fastboot metadata such as current slot and product.
- Accept a TWRP `.img` and ROM package/folder through native Tauri drag-and-drop.
- Temporarily boot a TWRP image with `fastboot boot` after validating the selected file and device mode.
- Perform a guarded Factory Reset with `fastboot -w` after an exact `WIPE` confirmation.
- Analyze dropped ROM inputs locally and classify ZIP, `payload.bin`, `super.img`, image files, image folders, and common Fastboot ROM layouts.
- List detected ROM artifacts and image sizes before any write operation is allowed.
- Reboot between Android, Bootloader, FastbootD, and Recovery.
- Scope actions to the detected device serial.
- Keep ADB/Fastboot process execution isolated in the Rust backend.

ROM partition flashing remains disabled until the flash-plan validation layer is complete.

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

The desktop UI accepts native file drops:

```text
TWRP
└── *.img

ROM
└── ROM file or folder
```

The TWRP input can be used with the guarded temporary boot action:

```text
fastboot -s <serial> boot "<twrp.img>"
```

Requirements:

- Device must be connected through classic Bootloader/Fastboot.
- Selected path must exist.
- Selected input must be an `.img` file.
- The detected device serial is always included in the command.

ROM inputs are analyzed locally. For directories, FlashROM inspects the top level plus a conventional `images/` directory and reports discovered artifacts such as `.img`, `payload.bin`, `super.img`, and flash scripts. No ROM partition writes are enabled yet.

## Clean Data / Factory Reset

FlashROM exposes Factory Reset as a separate destructive operation:

```text
fastboot -s <serial> -w
```

Safety rules:

- Device must be in classic Bootloader/Fastboot.
- The backend requires the exact confirmation value `WIPE`.
- The UI previews the exact command before enabling the action.
- A/B layout does not create `userdata_a` or `userdata_b`; userdata is treated independently from the boot slot model.

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
        +-- serial-scoped actions
        +-- guarded TWRP boot
        +-- guarded factory reset
        +-- ROM analyzer
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
- [x] Serial-scoped commands
- [ ] Realtime process streaming

### v0.2 - Safe manual operations

- [x] TWRP image validation
- [x] Temporary TWRP boot with command preview
- [x] Clean Data / Factory Reset with explicit confirmation
- [x] Inspect dropped ROM inputs
- [x] Detect common local ROM layouts and artifacts
- [ ] Read broader partition/device metadata
- [ ] Validate target partition
- [ ] Choose active/both slots for A/B devices
- [ ] Manual image flash command preview and confirmation
- [ ] Flash progress and result handling

### v0.3 - ROM flash wizard

- [x] Scan extracted ROM folder and conventional `images/` directory
- [x] Detect common image layouts
- [x] Classify recovery ZIP vs fastboot ROM vs `payload.bin` input
- [ ] Generate a flash plan
- [ ] Validate device codename / A-B slot / dynamic partitions
- [ ] Execute and verify the plan

### Later

- Inspect ZIP contents without extraction
- `payload.bin` extraction workflow
- `super.img` / dynamic partition tooling
- ADB sideload
- Backup/restore helpers
- Device profiles
- Release builds and auto-update

## License

A license has not been selected yet.
