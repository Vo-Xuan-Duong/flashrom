# FlashROM

FlashROM is a Windows-first Android flashing utility built with **Rust + Tauri 2 + React + TypeScript**. It replaces repetitive ADB/Fastboot command-line work with a GUI that keeps device state, validation, commands, and realtime process output visible.

> [!WARNING]
> Flashing Android partitions and wiping userdata can permanently erase data or make a device unbootable. FlashROM applies backend validation and explicit confirmations before destructive operations are allowed.

## Current capabilities

- Detect devices through ADB or Fastboot.
- Distinguish Android, Recovery, ADB Sideload, classic Fastboot, and FastbootD when possible.
- Detect single-slot vs A/B boot layouts.
- Map single-slot boot to `boot`; A/B boot to `boot_a` / `boot_b`.
- Manual boot-layout override: Auto / 1 partition / 2 partitions (A/B).
- Read current slot and product metadata.
- Drag/drop TWRP `.img` and ROM file/folder inputs.
- Temporarily boot TWRP using guarded `fastboot boot`.
- Clean Data / Factory Reset using guarded `fastboot -w` and exact `WIPE` confirmation.
- Analyze ROM input and classify ZIP, `payload.bin`, `super.img`, image files/folders, and common Fastboot ROM layouts.
- Generate a non-executing Flash Plan Preview from known image filenames.
- Choose active slot or both slots for A/B images.
- Probe partition metadata read-only: slot support, size, logical/physical state, partition type, and recommended Fastboot mode.
- Parse trusted local ROM product/codename metadata and compare it with the connected Fastboot product.
- Resolve physical/logical and A/B partition targets into a device-validated Final Flash Plan.
- Generate a full-ROM Execution Dry Run containing preflight checks, Fastboot/FastbootD mode transitions, future write previews, and post-write state checks.
- ADB Sideload recovery ZIPs with realtime stdout/stderr logging after verifying the device is actually in `sideload` state.
- Manually flash a fully resolved `.img` step with realtime output and backend safety checks.
- Scope ADB/Fastboot actions to the detected serial.

Automatic whole-ROM execution is **not enabled yet**. `payload.bin`, `super.img`, unresolved targets, product mismatches, unknown Fastboot mode, and any incomplete preflight remain blocked.

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

In Fastboot, FlashROM prefers:

```text
fastboot getvar has-slot:boot
fastboot getvar current-slot
```

In Android/Recovery through ADB it checks properties such as `ro.boot.slot_suffix`, `ro.boot.slot`, and `ro.build.ab_update`.

```text
Single-slot
└── boot

A/B
├── boot_a
└── boot_b
```

## TWRP

Drop a TWRP `.img` into the TWRP input zone. Temporary boot uses:

```text
fastboot -s <serial> boot "<twrp.img>"
```

Backend requirements:

- selected path exists and is an `.img` file;
- device is available through classic Bootloader/Fastboot;
- the detected serial is explicitly targeted.

## ROM Analyzer and Flash Plan Preview

ROM inputs are analyzed locally. Extracted directories are inspected at the top level plus a conventional `images/` directory.

Known image mappings currently include:

```text
boot.img
init_boot.img
vendor_boot.img
vendor_kernel_boot.img
dtbo.img
vbmeta.img
vbmeta_system.img
vbmeta_vendor.img
recovery.img
super.img
system.img
system_ext.img
product.img
vendor.img
odm.img
system_dlkm.img
vendor_dlkm.img
odm_dlkm.img
```

Unknown image filenames are deliberately not auto-mapped.

The Flash Plan Preview shows:

- image → candidate partition;
- active-slot / both-slot strategy;
- required Fastboot or FastbootD mode;
- command preview;
- unresolved/blocked/compatibility-check state.

## Partition Probe

Before a resolved image step can be manually flashed, the UI requires a read-only partition probe. It uses Fastboot variables such as:

```text
has-slot:<partition>
partition-size:<target>
is-logical:<target>
partition-type:<target>
```

The probe identifies A/B targets, physical vs logical partitions, target size, and the recommended mode.

## ROM product / codename validation

Final validation reads explicit local metadata when available, including:

```text
android-info.txt
metadata
images/android-info.txt
images/metadata
META-INF/com/android/metadata
```

Recognized device identity keys include `product`, `device`, `pre-device`, `post-device`, and `ro.product.device`. `board` is retained as supporting evidence but is not treated as equivalent to Fastboot `product` for automatic compatibility approval.

The ROM is considered automatically compatible only when trusted identity metadata contains an exact normalized match for:

```text
fastboot getvar product
```

If ROM identity metadata is absent, Fastboot product is unavailable, or the values differ, automatic whole-ROM execution remains blocked.

## Final Flash Plan

The Final Flash Plan re-reads live device state instead of trusting an earlier UI preview. It validates:

- ROM product/codename compatibility;
- `unlocked: yes`;
- current slot;
- `snapshot-update-status` when reported;
- `is-userspace` so Fastboot vs FastbootD is explicitly known;
- target A/B layout;
- physical vs logical state;
- partition size;
- image size vs target size.

Resolved physical partitions become **Fastboot / phase 1** steps. Logical partitions become **FastbootD / phase 2** steps. If both classes are present, the plan records that a mode transition is required.

`super.img` remains `manual_only` and prevents the Final Flash Plan from becoming automatically executable.

## Full-ROM Execution Dry Run

FlashROM can transform a ready Final Flash Plan into a non-executing dry run. The dry run contains:

```text
preflight
↓
mode transition if required
↓
revalidate step
↓
flash command preview
↓
post-write device-state check
↓
next step
```

The backend re-resolves the Final Flash Plan when building the dry run. It does **not** invoke `fastboot flash` for the full-ROM sequence. Automatic execution remains explicitly disabled until conservative partition ordering and stronger post-write verification rules are finalized.

## Guarded Manual Image Flash

A resolved step can only be enabled after partition metadata has been confirmed and the user types exactly:

```text
FLASH
```

The Rust backend then validates again before running `fastboot flash`:

- expected serial is currently visible through Fastboot;
- partition name contains only safe characters;
- selected file exists and is `.img`;
- bootloader reports `unlocked: yes`;
- snapshot update status is not active/merging;
- target partition reports a usable size;
- image size does not exceed target partition size;
- logical partition requires FastbootD;
- physical partition requires classic Fastboot.

If any check fails, the write is blocked. Realtime Fastboot output is streamed to the application log. The `FLASH` confirmation resets after each successful partition write.

## ADB Sideload

For a ROM ZIP, FlashROM provides a Recovery ZIP flow:

```text
adb -s <serial> sideload "<rom.zip>"
```

The backend refuses to start unless `adb devices` reports the exact serial in state:

```text
sideload
```

stdout/stderr are streamed to the runtime log while the ZIP is transferred.

## Clean Data / Factory Reset

Factory Reset executes:

```text
fastboot -s <serial> -w
```

Safety rules:

- classic Bootloader/Fastboot is required;
- backend confirmation must exactly equal `WIPE`;
- exact command is previewed in the UI;
- A/B boot layout does not imply `userdata_a` / `userdata_b`; userdata is handled separately.

## Architecture

```text
React / TypeScript UI
        |
        | Tauri invoke + native drag/drop + events
        v
Rust backend
        |
        +-- device / slot detection
        +-- serial-scoped process runner
        +-- realtime stdout/stderr streaming
        +-- guarded TWRP boot
        +-- guarded factory reset
        +-- ROM analyzer
        +-- Flash Plan Preview
        +-- read-only partition probe
        +-- ROM product compatibility validator
        +-- device-validated Final Flash Plan
        +-- full-ROM Execution Dry Run
        +-- guarded manual image flash
        +-- guarded ADB sideload
        |
        v
adb / fastboot
```

## Roadmap

### v0.1 - Device foundation

- [x] Project bootstrap
- [x] Device detection
- [x] ADB/Fastboot/FastbootD/Sideload detection
- [x] Single-slot / A-B boot detection
- [x] Manual boot-layout override
- [x] Native TWRP/ROM drag-and-drop
- [x] Reboot actions
- [x] Serial-scoped commands
- [x] Realtime process streaming

### v0.2 - Safe manual operations

- [x] TWRP image validation and temporary boot
- [x] Clean Data / Factory Reset with explicit confirmation
- [x] ROM analyzer
- [x] Flash Plan Preview
- [x] Active/both boot-slot selection
- [x] Read-only partition metadata probe
- [x] Resolve non-boot A/B targets from live Fastboot metadata
- [x] Validate ROM product/codename against the connected device
- [x] Manual resolved-image flash with backend preflight
- [x] Realtime flash output
- [x] ADB Sideload ZIP flow

### v0.3 - ROM Flash Wizard

- [x] Scan extracted Fastboot ROM folders
- [x] Classify common ROM input types
- [x] Generate preliminary Flash Plan Preview
- [x] Parse trusted ROM product/codename requirements
- [x] Finalize physical/logical and A/B partition targets
- [x] Build a device-validated Final Flash Plan
- [x] Build a non-executing multi-phase Execution Dry Run
- [ ] Define conservative partition ordering policy
- [ ] Safe ordered multi-step plan execution
- [ ] Revalidate device state before every write
- [ ] Verify each completed step and final device state
- [ ] Optional Clean Data after a successful validated plan

### Later

- Inspect ZIP contents without manual extraction
- `payload.bin` extraction workflow
- advanced `super.img` / dynamic partition tooling
- stronger post-write verification where device support allows it
- backup/restore helpers
- device profiles
- release builds and auto-update
- commit reproducible `pnpm-lock.yaml` and `src-tauri/Cargo.lock`

## License

A license has not been selected yet.
