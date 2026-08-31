# FlashROM beta validation gate

A public beta installer must not be published until the checklist below has been exercised on disposable/test hardware. CI validates code paths and invariants, but it cannot prove OEM Fastboot behavior or USB stability.

## Release gate

Record the tested device model/product, Android version, bootloader state, Platform Tools version and result for each applicable case. The Windows release workflow requires explicit acknowledgement that this validation was completed.

## Device matrix

At minimum, cover these device layouts when hardware is available:

- [ ] single-slot device
- [ ] A/B device
- [ ] dynamic-partition device
- [ ] FastbootD userspace transition
- [ ] virtual A/B device with snapshot status reporting

A single device can satisfy multiple rows.

## Non-destructive preflight

- [ ] `adb devices` / `fastboot devices` identify the intended serial
- [ ] multiple connected devices require explicit selection
- [ ] product/codename metadata matches the connected Fastboot product
- [ ] product mismatch blocks execution
- [ ] unknown ROM identity blocks Full-ROM execution
- [ ] locked bootloader blocks writes
- [ ] active snapshot update blocks writes
- [ ] partition size and logical/physical status are probed before execution
- [ ] oversized image is blocked
- [ ] unknown image filename is blocked
- [ ] both-slot strategy never invents `_a/_b` when `has-slot` is unknown/false

## Classic Fastboot / FastbootD

- [ ] physical partition step runs only in classic Fastboot
- [ ] logical partition step runs only in FastbootD
- [ ] Fastboot -> FastbootD transition reaches the same selected serial
- [ ] FastbootD -> bootloader transition reaches the same selected serial
- [ ] unexpected serial/product/slot/mode change stops the plan

## Write execution

Use a ROM known to be recoverable for the test hardware.

- [ ] Execution Guard hashes every image before write
- [ ] exact `FLASH ROM` confirmation is required
- [ ] exact `FLASH ROM WIPE` confirmation is required when Clean Data is enabled
- [ ] each step is revalidated immediately before write
- [ ] output is streamed while Fastboot writes
- [ ] first failed step stops later writes
- [ ] operation journal records success/failure accurately
- [ ] app restart can inspect interrupted/failed journal
- [ ] journal retry starts from the beginning after complete revalidation; it never blind-resumes

## First boot

- [ ] optional Android reboot occurs only after all selected steps succeed
- [ ] FlashROM waits for the original serial to return through ADB
- [ ] `sys.boot_completed=1` is required for verified success
- [ ] `ro.product.device` still matches the expected product
- [ ] timeout/failure is reported as operation failure even when partition writes succeeded

## Recovery paths

- [ ] temporary TWRP boot requires classic Fastboot
- [ ] ADB sideload requires device ADB state `sideload`
- [ ] Factory Reset requires exact `WIPE`
- [ ] disconnect/reconnect before a write fails closed
- [ ] USB disconnect during an active write produces a failed journal and does not continue automatically

## Payload / OTA ZIP

- [ ] specialized-tool diagnostics identify the exact resolved `payload-dumper-go` path
- [ ] full OTA/payload partition list is read before extraction
- [ ] only FlashROM allowlisted partitions are extracted
- [ ] extractor verification remains enabled
- [ ] incremental OTA requiring base images stops with an explicit diagnostic
- [ ] trusted product metadata is preserved into the prepared workspace
- [ ] prepared directory must pass the normal compatibility/Final Plan/Guard pipeline before execution

## `super.img`

- [ ] raw `super.img` remains manual-only in Final Plan
- [ ] raw super is unpacked with `lpunpack`
- [ ] sparse super is converted with `simg2img` first
- [ ] unsupported unpacked partitions are quarantined in `_ignored_partitions`
- [ ] slot-qualified logical images target only live-confirmed slot partitions
- [ ] incomplete `_a/_b` coverage blocks the `both` strategy
- [ ] unpacked logical partitions are written only through FastbootD Guard

## Post-ROM app restore

- [ ] restore profile scans third-party packages
- [ ] local APK and split APK backup hashes are recorded
- [ ] APK restore verifies package presence after install
- [ ] restore verification identifies missing packages
- [ ] Obtainium/F-Droid export is copied into the source-manager vault
- [ ] vault file SHA-256 is checked before staging
- [ ] staging targets the selected ADB serial
- [ ] config is copied only to `/sdcard/Download/FlashROM/<manager>`
- [ ] final Obtainium/F-Droid import remains an explicit in-app user action

## Windows installer smoke test

On a clean Windows 10/11 test machine:

- [ ] NSIS installer installs and launches
- [ ] MSI installer installs and launches
- [ ] uninstall succeeds
- [ ] app starts when no Platform Tools are installed and shows a useful diagnostic
- [ ] configured Platform Tools are detected
- [ ] missing payload/super helper tools do not prevent normal ADB/Fastboot workflows
- [ ] no unexpected network access is required for flashing workflows
- [ ] Windows SmartScreen/signing state is documented in release notes

## Beta release decision

A beta may be published when:

1. CI is green on the exact release commit.
2. All applicable high-risk checks above pass on test hardware.
3. Known unsupported cases are fail-closed and documented.
4. Installer signing status is explicit.
5. A recovery method for every test device is available before destructive testing.
