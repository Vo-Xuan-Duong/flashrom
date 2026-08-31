# FlashROM

FlashROM is a Windows-first Android flashing utility built with **Rust + Tauri 2 + React + TypeScript**. It replaces repetitive ADB/Fastboot terminal work with a GUI that keeps device identity, ROM analysis, validation, command previews, realtime output and recovery state visible.

> [!WARNING]
> Flashing Android partitions and wiping userdata can permanently erase data or make a device unbootable. FlashROM reduces avoidable mistakes with backend validation and explicit confirmations, but it cannot make an incompatible or defective ROM safe.

## Beta status

The software feature set for the first **0.1.0 beta** is complete. Publication is gated by the real-device checklist in [`docs/BETA-VALIDATION.md`](docs/BETA-VALIDATION.md), because CI cannot validate OEM bootloader behavior, USB stability or actual first-boot behavior on physical phones.

CI uses pinned `pnpm-lock.yaml` / `src-tauri/Cargo.lock` graphs and requires frontend type-check/tests/build plus Rust format/tests/checks on every push to `main`.

## Device and transport

- Discover devices through ADB and Fastboot.
- Explicitly select a serial when more than one device is connected.
- Distinguish Android, Recovery, ADB Sideload, classic Fastboot, FastbootD and unknown Fastboot mode.
- Detect current slot and single-slot vs A/B boot layouts.
- Scope destructive actions to the selected serial.
- Serialize protected operations globally so flash/wipe/reboot/sideload actions cannot overlap.
- Diagnose the resolved Android Platform Tools source and run `adb version` / `fastboot --version`.

## TWRP and recovery ZIP

- Drag/drop a TWRP `.img`.
- Temporarily boot recovery using guarded `fastboot boot` in classic Fastboot.
- ADB Sideload `.zip` packages only when the selected serial is actually reported in `sideload` state.
- Stream stdout/stderr to the runtime log.

## ROM analysis and planning

- Drag/drop ROM files or folders.
- Classify image folders, Fastboot ROMs, ZIPs, `payload.bin` and `super.img`.
- Inspect ZIP central directories without running embedded scripts.
- Probe live partition metadata: slot support, size, logical/physical state and partition type.
- Parse trusted ROM product/codename metadata (`android-info.txt`, OTA metadata and related files).
- Compare ROM identity with `fastboot getvar product`.
- Resolve active-slot/both-slot targets into a device-validated Final Flash Plan.
- Resolve explicit prepared names such as `system_a.img` / `system_b.img` only when live slot metadata confirms the target.
- Apply conservative partition ordering: boot-chain → system payload → AVB metadata.
- Generate a non-writing Execution Dry Run.
- Build an immutable SHA-256 Execution Guard.

## Guarded Full-ROM Executor

Full-ROM execution is enabled only when Final Plan and Execution Guard pass. The Rust backend rebuilds live state instead of trusting an old UI preview.

Before each partition write it revalidates:

- selected serial is still available;
- product/codename identity;
- bootloader reports `unlocked=yes`;
- active slot when applicable;
- snapshot update state;
- Fastboot vs FastbootD mode;
- partition size;
- logical vs physical state;
- image size;
- image SHA-256.

Physical partitions are written in classic Fastboot; logical partitions require FastbootD. Mode transitions are serialized and the executor waits for the selected serial to reappear before continuing.

Every Full-ROM operation creates a persistent journal. Recovery Center never blind-resumes the next partition after interruption. A retry starts from the beginning only after compatibility, partition metadata, ordering and SHA-256 Guard are rebuilt from the current ROM/device state.

Optional post-plan actions:

- Clean Data (`fastboot -w`) only after all selected partition writes succeed;
- reboot Android after successful writes.

If reboot is requested, the operation is not considered fully successful until FlashROM confirms the original serial through ADB, `sys.boot_completed=1`, and consistent `ro.product.device` identity.

## Specialized ROM preparation

FlashROM keeps raw container conversion separate from partition execution. Preparation never writes a device partition.

### `payload.bin` / OTA ZIP

Beta payload support uses a locally provisioned `payload-dumper-go` executable:

1. list payload partitions first;
2. intersect the manifest with FlashROM's partition allowlist;
3. extract only supported partitions;
4. leave payload SHA-256 verification enabled;
5. preserve trusted product/codename metadata;
6. send the prepared image directory back through the normal Final Plan and Execution Guard pipeline.

Incremental/delta OTA packages that require previous/base images stop with an explicit diagnostic in beta; FlashROM does not guess a base build.

### `super.img`

Raw `super.img` remains **non-automatic/manual-only** in the Final Plan. Beta support is instead:

```text
super.img
  ↓
[sparse] simg2img
  ↓
lpunpack
  ↓
allowlisted logical partition images
  ↓
slot validation + FastbootD Final Plan
  ↓
SHA-256 Guard
```

Unsupported unpacked images are quarantined under `_ignored_partitions` and never enter the automatic plan. Incomplete explicit `_a/_b` coverage blocks the `both` strategy.

External helper provisioning and resolution rules are documented in [`tools/README.md`](tools/README.md). FlashROM does not download helper executables at runtime.

## ROM ZIP Inspector

Safe ZIP extraction only allows known ROM inputs such as:

- `payload.bin`;
- `metadata` / `android-info.txt`;
- `META-INF/com/android/metadata`;
- `.img` files.

Extraction rejects symbolic links and unsafe paths, applies entry/file/total-size limits and never executes `flash_all`, updater scripts or archive programs.

## App restore

Before a clean flash, FlashROM can scan third-party packages and record their installer/source strategy.

Current restore helpers include:

- Google Play apps delegated to Android/Google restore and verified afterwards;
- detection of Obtainium/F-Droid/Aurora/external-store sources;
- local and sideloaded `base.apk` + split APK backup;
- SHA-256 for backed-up APK files;
- `adb install` / `adb install-multiple` restore;
- package verification after restore;
- persistent `flashrom-restore-profile.json` configuration.

### Obtainium / F-Droid configuration vault

The Beta Preparation Center can store an **explicitly exported** Obtainium/F-Droid configuration in the restore workspace, pin it by SHA-256 and later stage the verified file to:

```text
/sdcard/Download/FlashROM/obtainium
/sdcard/Download/FlashROM/fdroid
```

The final manager-specific Import/Restore action remains explicit inside Obtainium/F-Droid. FlashROM deliberately does not write app-private `/data/data` files or fabricate an unsupported ADB import API.

## Safety model

FlashROM follows these invariants:

```text
No implicit device
No implicit partition
No unknown Fastboot mode
No product mismatch
No locked bootloader writes
No active snapshot writes
No image larger than partition
No overlapping protected operations
No automatic unknown-image mapping
No raw automatic super.img write
No ZIP script execution
No blind journal resume
No unverified source-manager config staging
```

Backend confirmation phrases are required for high-risk operations:

```text
Manual image flash     FLASH
Factory reset          WIPE
Full ROM               FLASH ROM
Full ROM + wipe        FLASH ROM WIPE
Config staging         STAGE CONFIG
```

## Tool resolution

### Android Platform Tools

Resolution order:

1. `FLASHROM_PLATFORM_TOOLS`
2. `tools/platform-tools/`
3. system `PATH`

### Specialized helpers

Supported overrides:

```text
FLASHROM_PAYLOAD_DUMPER
FLASHROM_LPUNPACK
FLASHROM_SIMG2IMG
```

The Beta Preparation Center displays the exact resolved executable path before use. See [`tools/README.md`](tools/README.md).

## Development

### Prerequisites

- Windows 10/11 for the primary target.
- Microsoft C++ / Visual Studio Build Tools with Desktop development with C++.
- Rust stable MSVC toolchain.
- Node.js 24 in CI or a compatible version meeting `package.json` engines.
- pnpm 10.15.0.
- Android SDK Platform Tools.

```powershell
rustup default stable-msvc
corepack enable
pnpm install --frozen-lockfile
pnpm tauri dev
```

Validation commands:

```powershell
pnpm check
pnpm test
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo check --locked --manifest-path src-tauri/Cargo.toml
```

## Architecture

```text
React / TypeScript UI
        |
        | Tauri invoke + native drag/drop + events
        v
Rust backend
        |
        +-- device discovery / explicit serial selection
        +-- Platform Tools + specialized-tool diagnostics
        +-- global OperationManager
        +-- no-shell process runner + realtime output
        +-- TWRP / recovery sideload / clean data
        +-- ROM analyzer + ZIP inspector
        +-- payload/super preparation
        +-- product/codename compatibility
        +-- partition probe
        +-- Final Flash Plan
        +-- conservative ordering
        +-- SHA-256 Execution Guard
        +-- guarded Full-ROM Executor
        +-- persistent execution journals
        +-- Android boot verification
        +-- App Restore Profile
        +-- Obtainium/F-Droid config vault
        |
        v
adb / fastboot / explicitly provisioned helper tools
```

## CI and release

`.github/workflows/ci.yml` requires:

```text
pnpm install --frozen-lockfile
pnpm check
pnpm test
pnpm build
cargo fmt --check
cargo test --locked
cargo check --locked
```

`.github/workflows/release.yml` is manual-only and creates Windows NSIS/MSI draft prereleases. It requires explicit acknowledgement that the hardware validation checklist was completed and that installer signing status is understood before the release job can run.

Tauri is configured with a production CSP and Windows bundle targets. Code signing is an external release credential requirement; the repository does not contain signing keys or certificates.

## 0.1 beta checklist

### Software implementation

- [x] Device discovery and explicit multi-device selection
- [x] ADB / Recovery / Sideload / Fastboot / FastbootD detection
- [x] Single-slot and A/B detection
- [x] TWRP temporary boot
- [x] Factory Reset confirmation
- [x] ROM Analyzer / ZIP Inspector
- [x] Partition Probe
- [x] ROM product/codename validation
- [x] Final Flash Plan including explicit slot-qualified prepared images
- [x] Conservative partition ordering
- [x] SHA-256 Execution Guard
- [x] Global protected-operation lock
- [x] Guarded ordered Full-ROM Executor
- [x] Per-step live revalidation
- [x] Realtime process output
- [x] Persistent operation journals
- [x] Recovery Center safe retry-from-beginning
- [x] Android first-boot verification
- [x] Payload full-OTA preparation with extractor verification preserved
- [x] `super.img` unpack preparation through `simg2img` / `lpunpack`
- [x] App Restore Profile / local APK backup and restore
- [x] Obtainium/F-Droid configuration vault + verified device staging
- [x] Platform/specialized tool diagnostics
- [x] Frontend IPC tests + Rust unit tests
- [x] Pinned pnpm/Cargo lockfiles and frozen/locked CI
- [x] Production CSP and Windows NSIS/MSI bundle configuration
- [x] Manual, gated GitHub Release workflow
- [x] Beta hardware validation checklist

### External release gates

- [ ] Execute applicable rows in [`docs/BETA-VALIDATION.md`](docs/BETA-VALIDATION.md) on real test hardware.
- [ ] Replace fallback/temporary artwork if a final product icon is desired before public distribution.
- [ ] Configure Windows code signing if signed public installers are required.

## Responsible testing

Do not validate new write paths first on a primary phone. Use test hardware and keep a known recovery method available. Bootloader firmware partitions outside FlashROM's explicit beta allowlist remain unsupported and fail closed.

## License

FlashROM is licensed under the [MIT License](LICENSE).

See [SECURITY.md](SECURITY.md), [CONTRIBUTING.md](CONTRIBUTING.md), [CHANGELOG.md](CHANGELOG.md), [`tools/README.md`](tools/README.md) and [`docs/BETA-VALIDATION.md`](docs/BETA-VALIDATION.md) for project policies and release guidance.
