# Local Android Platform Tools

Optionally copy the official Android SDK Platform Tools files into this directory for local development.

Expected Windows executables:

```text
adb.exe
AdbWinApi.dll
AdbWinUsbApi.dll
fastboot.exe
```

The binaries are intentionally ignored by Git. FlashROM can also use a directory from the `FLASHROM_PLATFORM_TOOLS` environment variable or tools available on the system `PATH`.
