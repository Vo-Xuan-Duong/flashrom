# FlashROM

FlashROM is a Windows-first Android flashing utility built with **Rust + Tauri 2 + React + TypeScript**. It replaces repetitive ADB/Fastboot command-line work with a GUI that keeps the selected device, ROM analysis, validation, command previews, realtime process output and recovery state visible.

> [!WARNING]
> Flashing Android partitions and wiping userdata can permanently erase data or make a device unbootable. FlashROM reduces avoidable mistakes with backend validation and explicit confirmations, but it cannot make an incompatible or defective ROM safe.

## Current state

The project is preparing for its first **0.1.0 beta**. CI type-checks/builds the frontend and formats/tests/checks the Rust backend on every push to `main`.

### Device and transport

- Discover devices through ADB and Fastboot.
- Explicitly select a serial when more than one device is connected.
- Distinguish Android, Recovery, ADB Sideload, classic Fastboot, FastbootD and unknown Fastboot mode.
- Detect current slot and single-slot vs A/B boot layouts.
- Scope destructive actions to the selected serial.
- Serialize protected operations globally so flash/wipe/reboot/sideload actions cannot overlap.
- Diagnose the resolved Android Platform Tools source and run `adb version` / `fastboot --version`.

### TWRP and recovery ZIP

- Drag/drop a TWRP `.img`.
- Temporarily boot recovery using guarded `fastboot boot` in classic Fastboot.
- ADB Sideload `.zip` packages only when the selected serial is actually reported in `sideload` state.
- Stream stdout/stderr to the runtime log.

### ROM analysis and planning

- Drag/drop ROM files or folders.
- Classify image folders, Fastboot ROMs, ZIPs, `payload.bin` and `super.img`.
- Generate a preliminary Flash Plan from an allowlist of known Android image names.
- Probe live partition metadata: slot support, size, logical/physical state and partition type.
- Parse trusted ROM product/codename metadata (`android-info.txt`, OTA metadata and related files).
- Compare ROM identity with `fastboot getvar product`.
- Resolve active-slot/both-slot targets into a device-validated Final Flash Plan.
- Apply conservative partition ordering: boot-chain → system payload → AVB metadata.
- Generate a non-writing Execution Dry Run.
- Build an immutable SHA-256 Execution Guard.

### Guarded Full-ROM Executor

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

Every operation creates a persistent journal under the FlashROM application data directory. If an operation is interrupted, **Recovery Center never blind-resumes the next partition**. It can inspect the previous journal and retry from the beginning only after the backend rebuilds compatibility, partition metadata, ordering and SHA-256 Guard from the current device/ROM state.

Optional post-plan actions:

- Clean Data (`fastboot -w`) only after all partition writes succeed;
- reboot Android after a successful plan.

### Post-flash boot verification

Recovery Center can wait for the selected serial to return through ADB and verifies:

```text
sys.boot_completed = 1
ro.product.device
ro.build.version.release
ro.build.fingerprint
```

This distinguishes **partition writes completed** from **Android boot verified**.

### ROM ZIP Inspector

FlashROM can inspect a ZIP central directory without executing embedded scripts. It identifies payload/recovery/fastboot-style packages and surfaces relevant metadata/images.

Safe extraction only allows known ROM inputs such as:

- `payload.bin`;
- `metadata` / `android-info.txt`;
- `META-INF/com/android/metadata`;
- `.img` files.

Extraction rejects symbolic links and unsafe paths using ZIP path confinement, applies entry/file/total-size limits and never executes `flash_all`, updater scripts or other archive programs.

`payload.bin` can currently be extracted to a workspace, but **update_engine payload → partition image extraction is intentionally not automatic yet**. `super.img` also remains manual-only. These two high-risk formats stay fail-closed until their dedicated parsers/workflows are validated.

### App Restore Profile

Before a clean flash, FlashROM can scan third-party packages and record their installer/source strategy.

Current restore helpers include:

- Google Play apps delegated to Android/Google restore and verified afterwards;
- detection of Obtainium/F-Droid/Aurora/external-store sources;
- local and sideloaded `base.apk` + split APK backup;
- SHA-256 for backed-up APK files;
- `adb install` / `adb install-multiple` restore;
- package verification after restore;
- persistent `flashrom-restore-profile.json` configuration.

Private `/data/data` application data is deliberately not copied directly. Seedvault/Neo Backup integration is a future extension because Android UID/SELinux/Keystore/encryption semantics make raw private-data copying unsafe.

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
No ZIP script execution
No blind journal resume
```

Backend confirmation phrases are required for high-risk operations:

```text
Manual image flash     FLASH
Factory reset          WIPE
Full ROM               FLASH ROM
Full ROM + wipe        FLASH ROM WIPE
```

## Android Platform Tools resolution

FlashROM resolves `adb` and `fastboot` in this order:

1. `FLASHROM_PLATFORM_TOOLS` environment variable.
2. `tools/platform-tools/` inside the working directory.
3. System `PATH`.

Example:

```powershell
$env:FLASHROM_PLATFORM_TOOLS="D:\Android\platform-tools"
pnpm tauri dev
```

The UI includes a diagnostics panel showing the resolved source and tool version output.

## Development

### Prerequisites

- Windows 10/11 for the primary target.
- Microsoft C++ / Visual Studio Build Tools with Desktop development with C++.
- Rust stable MSVC toolchain.
- Node.js 24 (CI) or a compatible Node version meeting `package.json` engines.
- pnpm 10.15.0.
- Android SDK Platform Tools.

```powershell
rustup default stable-msvc
corepack enable
pnpm install
pnpm tauri dev
```

Validation commands:

```powershell
pnpm check
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
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
        +-- Platform Tools diagnostics
        +-- global OperationManager
        +-- ADB/Fastboot process runner + realtime output
        +-- TWRP / recovery sideload / clean data
        +-- ROM analyzer + ZIP inspector
        +-- product/codename compatibility
        +-- partition probe
        +-- Final Flash Plan
        +-- conservative ordering
        +-- SHA-256 Execution Guard
        +-- guarded Full-ROM Executor
        +-- persistent execution journals
        +-- Android boot verification
        +-- App Restore Profile
        |
        v
adb / fastboot
```

## CI and release

`.github/workflows/ci.yml` runs frontend and Rust checks and temporarily uploads generated dependency lockfiles so the repository can pin its build graph.

`.github/workflows/release.yml` is **manual-only** and builds Windows bundles through Tauri. Releases are created as draft/prerelease first so artifacts can be reviewed before publication.

Tauri is configured with a production CSP and Windows NSIS/MSI bundle targets.

## Roadmap

### Completed for the 0.1 beta core

- [x] Device discovery and explicit multi-device selection
- [x] ADB / Recovery / Sideload / Fastboot / FastbootD detection
- [x] Single-slot and A/B detection
- [x] TWRP temporary boot
- [x] Factory Reset confirmation
- [x] ROM Analyzer
- [x] Partition Probe
- [x] ROM product/codename validation
- [x] Final Flash Plan
- [x] Conservative partition ordering
- [x] SHA-256 Execution Guard
- [x] Global protected-operation lock
- [x] Guarded ordered Full-ROM Executor
- [x] Per-step live revalidation
- [x] Realtime process output
- [x] Persistent operation journals
- [x] Recovery Center safe retry-from-beginning
- [x] Android boot verification
- [x] ZIP central-directory inspection and safe extraction
- [x] App Restore Profile / local APK backup and restore
- [x] Platform Tools diagnostics
- [x] Production CSP and Windows bundle configuration
- [x] Manual GitHub Release workflow

### Remaining specialized / validation work

- [ ] Fully parse Android update_engine `payload.bin` and extract partition images with manifest validation
- [ ] Advanced `super.img` / logical partition metadata tooling
- [ ] Obtainium/F-Droid configuration import/export automation
- [ ] Real-device hardware test matrix across single-slot, A/B, dynamic and virtual A/B devices
- [ ] Frontend automated tests / Tauri integration tests
- [ ] Commit generated `pnpm-lock.yaml` and `src-tauri/Cargo.lock`, then enforce frozen/locked CI installs
- [ ] Replace fallback build icon with final product artwork
- [ ] Optional updater/code-signing after the beta distribution flow is proven

## Responsible testing

Do not validate new write paths first on a primary phone. Use test hardware and keep a known recovery method available. New automatic write support for bootloader firmware, `super.img` or unknown partitions should remain disabled until a device-specific validation strategy exists.

## License

FlashROM is licensed under the [MIT License](LICENSE).

See [SECURITY.md](SECURITY.md), [CONTRIBUTING.md](CONTRIBUTING.md) and [CHANGELOG.md](CHANGELOG.md) for project policies and release notes.
