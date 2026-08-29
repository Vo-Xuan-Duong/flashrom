import { useCallback, useEffect, useMemo, useState } from "react";
import {
  detectDevice,
  rebootDevice,
  type DeviceSnapshot,
  type RebootTarget,
} from "./lib/tauri";

const emptyDevice: DeviceSnapshot = {
  connected: false,
  serial: null,
  mode: "Disconnected",
  slot: null,
  product: null,
  tool: null,
  diagnostic: "Not checked yet.",
};

function now() {
  return new Date().toLocaleTimeString([], { hour12: false });
}

function App() {
  const [device, setDevice] = useState<DeviceSnapshot>(emptyDevice);
  const [busy, setBusy] = useState(false);
  const [logs, setLogs] = useState<string[]>([]);

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
          ? `Detected ${snapshot.serial ?? "device"} via ${snapshot.tool ?? "unknown"} (${snapshot.mode}).`
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

  const statusClass = useMemo(() => {
    if (!device.connected) return "status-dot status-offline";
    if (device.mode.includes("Unauthorized") || device.mode.includes("Offline")) {
      return "status-dot status-warning";
    }
    return "status-dot status-online";
  }, [device]);

  async function handleReboot(target: RebootTarget) {
    if (!device.connected || busy) return;

    setBusy(true);
    try {
      const result = await rebootDevice(target);
      appendLog(`$ ${result.command}`);
      if (result.output.trim()) appendLog(result.output.trim());
      appendLog(result.success ? `Reboot to ${target} requested.` : `Command failed with exit code ${result.status}.`);

      if (result.success) {
        setDevice({ ...emptyDevice, diagnostic: "Device is changing mode. Refresh detection shortly." });
      }
    } catch (error) {
      appendLog(`Reboot failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  const actionsDisabled = busy || !device.connected || device.mode.includes("Unauthorized") || device.mode.includes("Offline");

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
            <dt>Transport</dt>
            <dd>{device.tool?.toUpperCase() ?? "—"}</dd>
          </div>
        </dl>

        <p className="diagnostic">{device.diagnostic}</p>
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
          Partition flashing is intentionally disabled in the bootstrap. The next layer will add image selection,
          partition validation, command preview, device compatibility checks, and explicit confirmation before any
          destructive command is exposed.
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
