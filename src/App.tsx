import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  detectDevice,
  rebootDevice,
  type BootLayout,
  type DeviceSnapshot,
  type RebootTarget,
} from "./lib/tauri";

type BootLayoutSelection = "auto" | "single" | "ab";
type DropTarget = "twrp" | "rom" | null;

const emptyDevice: DeviceSnapshot = {
  connected: false,
  serial: null,
  mode: "Disconnected",
  slot: null,
  product: null,
  tool: null,
  bootLayout: "unknown",
  bootPartitions: [],
  diagnostic: "Not checked yet.",
};

function now() {
  return new Date().toLocaleTimeString([], { hour12: false });
}

function fileName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

function partitionsFor(layout: BootLayout): string[] {
  if (layout === "single") return ["boot"];
  if (layout === "ab") return ["boot_a", "boot_b"];
  return [];
}

function dropTargetAt(position: { x: number; y: number }): DropTarget {
  const ratio = window.devicePixelRatio || 1;
  const candidates = [
    [position.x / ratio, position.y / ratio],
    [position.x, position.y],
  ];

  for (const [x, y] of candidates) {
    const element = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-drop-target]");
    const target = element?.dataset.dropTarget;
    if (target === "twrp" || target === "rom") return target;
  }

  return null;
}

function App() {
  const [device, setDevice] = useState<DeviceSnapshot>(emptyDevice);
  const [busy, setBusy] = useState(false);
  const [logs, setLogs] = useState<string[]>([]);
  const [layoutSelection, setLayoutSelection] = useState<BootLayoutSelection>("auto");
  const [twrpPath, setTwrpPath] = useState<string | null>(null);
  const [romPath, setRomPath] = useState<string | null>(null);
  const [dragTarget, setDragTarget] = useState<DropTarget>(null);

  const appendLog = useCallback((message: string) => {
    setLogs((current) => [...current.slice(-199), `[${now()}] ${message}`]);
  }, []);

  const refresh = useCallback(async () => {
    setBusy(true);
    try {
      const snapshot = await detectDevice();
      setDevice(snapshot);
      appendLog(
        snapshot.connected
          ? `Detected ${snapshot.serial ?? "device"} via ${snapshot.tool ?? "unknown"} (${snapshot.mode}); boot layout: ${snapshot.bootLayout}.`
          : `No device detected. ${snapshot.diagnostic}`,
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      appendLog(`Device detection failed: ${message}`);
      setDevice({ ...emptyDevice, diagnostic: message });
    } finally {
      setBusy(false);
    }
  }, [appendLog]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (disposed) return;

        if (event.payload.type === "leave") {
          setDragTarget(null);
          return;
        }

        if (event.payload.type === "enter" || event.payload.type === "over") {
          setDragTarget(dropTargetAt(event.payload.position));
          return;
        }

        const target = dropTargetAt(event.payload.position);
        setDragTarget(null);

        if (!target || event.payload.paths.length === 0) return;
        const path = event.payload.paths[0];

        if (target === "twrp") {
          if (!path.toLowerCase().endsWith(".img")) {
            appendLog(`Rejected TWRP input: ${fileName(path)}. Expected an .img file.`);
            return;
          }
          setTwrpPath(path);
          appendLog(`TWRP image selected: ${path}`);
          return;
        }

        setRomPath(path);
        appendLog(`ROM input selected: ${path}`);
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((error) => appendLog(`Unable to enable native file drop: ${String(error)}`));

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [appendLog]);

  const statusClass = useMemo(() => {
    if (!device.connected) return "status-dot status-offline";
    if (device.mode.includes("Unauthorized") || device.mode.includes("Offline")) {
      return "status-dot status-warning";
    }
    return "status-dot status-online";
  }, [device]);

  const effectiveBootLayout: BootLayout =
    layoutSelection === "auto" ? device.bootLayout : layoutSelection;
  const effectiveBootPartitions = partitionsFor(effectiveBootLayout);

  async function handleReboot(target: RebootTarget) {
    if (!device.connected || busy) return;

    setBusy(true);
    try {
      const result = await rebootDevice(target);
      appendLog(`$ ${result.command}`);
      if (result.output.trim()) appendLog(result.output.trim());
      appendLog(
        result.success
          ? `Reboot to ${target} requested.`
          : `Command failed with exit code ${result.status}.`,
      );

      if (result.success) {
        setDevice({ ...emptyDevice, diagnostic: "Device is changing mode. Refresh detection shortly." });
      }
    } catch (error) {
      appendLog(`Reboot failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  const actionsDisabled =
    busy ||
    !device.connected ||
    device.mode.includes("Unauthorized") ||
    device.mode.includes("Offline");

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <p className="eyebrow">Android device utility</p>
          <h1>FlashROM</h1>
        </div>
        <button className="button button-secondary" onClick={() => void refresh()} disabled={busy}>
          {busy ? "Working…" : "Refresh device"}
        </button>
      </header>

      <section className="device-card" aria-live="polite">
        <div className="device-heading">
          <div>
            <span className={statusClass} aria-hidden="true" />
            <span className="device-state">{device.connected ? "Device connected" : "No device"}</span>
          </div>
          <span className="mode-pill">{device.mode}</span>
        </div>

        <dl className="device-grid">
          <div>
            <dt>Serial</dt>
            <dd>{device.serial ?? "—"}</dd>
          </div>
          <div>
            <dt>Product</dt>
            <dd>{device.product ?? "—"}</dd>
          </div>
          <div>
            <dt>Active slot</dt>
            <dd>{device.slot?.toUpperCase() ?? "—"}</dd>
          </div>
          <div>
            <dt>Boot layout</dt>
            <dd>{device.bootLayout === "ab" ? "A/B" : device.bootLayout === "single" ? "Single" : "Unknown"}</dd>
          </div>
          <div>
            <dt>Boot partitions</dt>
            <dd>{device.bootPartitions.length ? device.bootPartitions.join(", ") : "—"}</dd>
          </div>
          <div>
            <dt>Transport</dt>
            <dd>{device.tool?.toUpperCase() ?? "—"}</dd>
          </div>
        </dl>

        <p className="diagnostic">{device.diagnostic}</p>
      </section>

      <section className="panel">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Partition model</p>
            <h2>Boot partition layout</h2>
          </div>
          <p>Auto uses device metadata. Override only when the device reports an incorrect layout.</p>
        </div>

        <div className="layout-options" role="radiogroup" aria-label="Boot partition layout">
          <button
            type="button"
            className={`layout-option ${layoutSelection === "auto" ? "layout-option-active" : ""}`}
            onClick={() => setLayoutSelection("auto")}
          >
            <strong>Auto detect</strong>
            <span>{device.bootLayout === "unknown" ? "Not detected" : device.bootLayout === "ab" ? "Detected A/B" : "Detected single"}</span>
          </button>
          <button
            type="button"
            className={`layout-option ${layoutSelection === "single" ? "layout-option-active" : ""}`}
            onClick={() => setLayoutSelection("single")}
          >
            <strong>1 partition</strong>
            <span>boot</span>
          </button>
          <button
            type="button"
            className={`layout-option ${layoutSelection === "ab" ? "layout-option-active" : ""}`}
            onClick={() => setLayoutSelection("ab")}
          >
            <strong>2 partitions (A/B)</strong>
            <span>boot_a + boot_b</span>
          </button>
        </div>

        <div className="partition-preview">
          <span>Effective targets</span>
          <div className="partition-pills">
            {effectiveBootPartitions.length ? (
              effectiveBootPartitions.map((partition) => (
                <span
                  key={partition}
                  className={`partition-pill ${device.slot && partition.endsWith(`_${device.slot}`) ? "partition-pill-active" : ""}`}
                >
                  {partition}
                </span>
              ))
            ) : (
              <span className="partition-pill partition-pill-muted">Unknown — select a layout manually</span>
            )}
          </div>
        </div>
      </section>

      <section className="panel">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Flash inputs</p>
            <h2>Drop TWRP and ROM</h2>
          </div>
          <p>Drop local files directly from Explorer. No flash command is executed at this stage.</p>
        </div>

        <div className="drop-grid">
          <div
            className={`drop-zone ${dragTarget === "twrp" ? "drop-zone-active" : ""} ${twrpPath ? "drop-zone-filled" : ""}`}
            data-drop-target="twrp"
          >
            <div className="drop-icon" aria-hidden="true">IMG</div>
            <div className="drop-copy">
              <strong>TWRP image</strong>
              <span>{twrpPath ? fileName(twrpPath) : "Drop twrp.img here"}</span>
              <small>{twrpPath ?? "Accepts .img"}</small>
            </div>
            {twrpPath && (
              <button type="button" className="text-button" onClick={() => setTwrpPath(null)}>
                Clear
              </button>
            )}
          </div>

          <div
            className={`drop-zone ${dragTarget === "rom" ? "drop-zone-active" : ""} ${romPath ? "drop-zone-filled" : ""}`}
            data-drop-target="rom"
          >
            <div className="drop-icon" aria-hidden="true">ROM</div>
            <div className="drop-copy">
              <strong>ROM package</strong>
              <span>{romPath ? fileName(romPath) : "Drop ROM file or folder here"}</span>
              <small>{romPath ?? "ZIP / fastboot ROM / payload package"}</small>
            </div>
            {romPath && (
              <button type="button" className="text-button" onClick={() => setRomPath(null)}>
                Clear
              </button>
            )}
          </div>
        </div>
      </section>

      <section className="panel">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Device actions</p>
            <h2>Reboot mode</h2>
          </div>
          <p>These actions do not flash or erase partitions.</p>
        </div>

        <div className="action-grid">
          <button className="action-button" disabled={actionsDisabled} onClick={() => void handleReboot("android")}>
            <strong>Android</strong>
            <span>Normal system boot</span>
          </button>
          <button className="action-button" disabled={actionsDisabled} onClick={() => void handleReboot("bootloader")}>
            <strong>Bootloader</strong>
            <span>Classic Fastboot mode</span>
          </button>
          <button className="action-button" disabled={actionsDisabled} onClick={() => void handleReboot("fastbootd")}>
            <strong>FastbootD</strong>
            <span>Userspace Fastboot</span>
          </button>
          <button className="action-button" disabled={actionsDisabled} onClick={() => void handleReboot("recovery")}>
            <strong>Recovery</strong>
            <span>Recovery environment</span>
          </button>
        </div>
      </section>

      <section className="panel upcoming-panel">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Next milestone</p>
            <h2>Safe flashing</h2>
          </div>
        </div>
        <p>
          TWRP and ROM inputs are captured now, but destructive operations remain disabled. The next layer will inspect
          these inputs, validate the selected partition layout, preview the exact fastboot command, and require explicit
          confirmation before flashing.
        </p>
      </section>

      <section className="console-panel">
        <div className="console-header">
          <div>
            <p className="eyebrow">Runtime</p>
            <h2>Command log</h2>
          </div>
          <button className="text-button" type="button" onClick={() => setLogs([])} disabled={logs.length === 0}>
            Clear
          </button>
        </div>
        <pre className="console" aria-live="polite">
          {logs.length ? logs.join("\n") : "No commands executed yet."}
        </pre>
      </section>
    </main>
  );
}

export default App;
