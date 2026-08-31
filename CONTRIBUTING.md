# Contributing to FlashROM

FlashROM is a destructive-capability Android utility. Contributions should optimize for correctness, explicit targeting and fail-closed behavior rather than convenience shortcuts.

## Development checks

Before opening a pull request, run:

```powershell
pnpm install
pnpm check
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

## Safety rules

- Never invoke a destructive ADB/Fastboot command without `-s <serial>`.
- Do not infer `_a` / `_b` for every partition; probe slot metadata first.
- Do not treat classic Fastboot and FastbootD as interchangeable.
- Do not auto-map unknown image filenames to partitions.
- Do not bypass product/codename checks for automatic Full-ROM execution.
- Do not weaken SHA-256 or partition-size checks to make a ROM pass validation.
- Do not run multiple protected write/reboot/wipe operations concurrently.
- Do not silently execute scripts found inside ROM ZIPs.
- Do not blindly resume an interrupted flash from the next partition; rebuild the current Guard first.
- Keep `super.img`, bootloader firmware and other high-risk targets fail-closed unless a specific validated workflow exists.

## Hardware testing

When testing on real hardware, include the device product/codename, slot model, Android Platform Tools version, Fastboot/FastbootD mode and whether dynamic/virtual A/B partitions are present. Prefer sacrificial/test devices for new write paths.

## Pull requests

Keep safety-sensitive changes small enough to review. Explain which backend validation protects the new action and include unit tests for parsers/decision logic where practical.
