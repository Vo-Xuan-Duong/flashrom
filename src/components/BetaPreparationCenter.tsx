import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import {
  backupSourceManagerConfig,
  inspectSourceManagerVault,
  inspectSpecialTools,
  listBetaDevices,
  preparePayloadInput,
  prepareSuperInput,
  stageSourceManagerConfig,
  type PreparedRomInput,
  type SourceManagerId,
  type SourceManagerManifest,
  type SourceManagerStageResult,
  type SpecialToolsStatus,
} from "../lib/beta";
import type { DeviceSnapshot, ProcessOutputEvent } from "../lib/tauri";

function formatBytes(bytes: number) {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** index;
  return `${value >= 10 || index === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[index]}`;
}

function BetaPreparationCenter() {
  const [tools, setTools] = useState<SpecialToolsStatus | null>(null);
  const [toolBusy, setToolBusy] = useState(false);
  const [sourcePath, setSourcePath] = useState("");
  const [prepWorkspace, setPrepWorkspace] = useState("");
  const [prepBusy, setPrepBusy] = useState(false);
  const [prepared, setPrepared] = useState<PreparedRomInput | null>(null);
  const [prepLog, setPrepLog] = useState<string[]>([]);

  const [restoreWorkspace, setRestoreWorkspace] = useState("");
  const [manager, setManager] = useState<SourceManagerId>("obtainium");
  const [configPath, setConfigPath] = useState("");
  const [vault, setVault] = useState<SourceManagerManifest | null>(null);
  const [vaultBusy, setVaultBusy] = useState(false);
  const [devices, setDevices] = useState<DeviceSnapshot[]>([]);
  const [serial, setSerial] = useState("");
  const [stageConfirmation, setStageConfirmation] = useState("");
  const [stageResult, setStageResult] = useState<SourceManagerStageResult | null>(null);
  const [statusMessage, setStatusMessage] = useState("");

  const selectedEntry = useMemo(
    () => vault?.entries.find((entry) => entry.manager === manager) ?? null,
    [manager, vault],
  );

  async function refreshTools() {
    if (toolBusy) return;
    setToolBusy(true);
    try {
      const result = await inspectSpecialTools();
      setTools(result);
      setStatusMessage(result.diagnostic);
    } catch (error) {
      setTools(null);
      setStatusMessage(`Special tool check failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setToolBusy(false);
    }
  }

  async function refreshDevices() {
    try {
      const result = await listBetaDevices();
      const adbDevices = result.filter((device) => device.tool === "adb" && device.serial);
      setDevices(adbDevices);
      setSerial((current) => {
        if (current && adbDevices.some((device) => device.serial === current)) return current;
        return adbDevices.length === 1 ? adbDevices[0].serial ?? "" : "";
      });
    } catch {
      setDevices([]);
      setSerial("");
    }
  }

  useEffect(() => {
    void refreshTools();
    void refreshDevices();
  }, []);

  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void listen<ProcessOutputEvent>("flashrom-process-output", (event) => {
      if (disposed || !event.payload.operationId.startsWith("prepare-")) return;
      const data = event.payload.data.trim();
      if (!data) return;
      setPrepLog((current) => [
        ...current.slice(-149),
        `[${event.payload.stream}] ${data}`,
      ]);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    });
    return () => {
      disposed = true;
      stop?.();
    };
  }, []);

  async function prepare(kind: "payload" | "super") {
    if (!sourcePath.trim() || !prepWorkspace.trim() || prepBusy) return;
    setPrepBusy(true);
    setPrepared(null);
    setPrepLog([]);
    try {
      const result =
        kind === "payload"
          ? await preparePayloadInput(sourcePath.trim(), prepWorkspace.trim())
          : await prepareSuperInput(sourcePath.trim(), prepWorkspace.trim());
      setPrepared(result);
      setStatusMessage(result.diagnostic);
    } catch (error) {
      setStatusMessage(`ROM preparation failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setPrepBusy(false);
    }
  }

  async function refreshVault() {
    if (!restoreWorkspace.trim() || vaultBusy) return;
    setVaultBusy(true);
    try {
      const result = await inspectSourceManagerVault(restoreWorkspace.trim());
      setVault(result);
      setStatusMessage(`Source-manager vault loaded: ${result.entries.length} config(s).`);
    } catch (error) {
      setVault(null);
      setStatusMessage(`Vault read failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setVaultBusy(false);
    }
  }

  async function saveConfig() {
    if (!restoreWorkspace.trim() || !configPath.trim() || vaultBusy) return;
    setVaultBusy(true);
    try {
      const result = await backupSourceManagerConfig(
        restoreWorkspace.trim(),
        manager,
        configPath.trim(),
      );
      setStatusMessage(result.diagnostic);
      setConfigPath("");
      const current = await inspectSourceManagerVault(restoreWorkspace.trim());
      setVault(current);
    } catch (error) {
      setStatusMessage(`Config vault backup failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setVaultBusy(false);
    }
  }

  async function stageConfig() {
    if (
      !serial ||
      !restoreWorkspace.trim() ||
      !selectedEntry ||
      stageConfirmation !== "STAGE CONFIG" ||
      vaultBusy
    ) {
      return;
    }
    setVaultBusy(true);
    setStageResult(null);
    try {
      const result = await stageSourceManagerConfig({
        serial,
        workspace: restoreWorkspace.trim(),
        manager,
        confirmation: stageConfirmation,
      });
      setStageResult(result);
      setStageConfirmation("");
      setStatusMessage(result.diagnostic);
    } catch (error) {
      setStatusMessage(`Config staging failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setVaultBusy(false);
    }
  }

  return (
    <main className="app-shell beta-prep-shell">
      <section className="panel beta-prep-panel">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Beta ROM coverage</p>
            <h2>Payload & Dynamic Partition Preparation</h2>
          </div>
          <p>
            Convert specialized ROM containers into normal allowlisted image inputs. Preparation never writes a device partition.
          </p>
        </div>

        <div className="beta-tool-grid">
          {[tools?.payloadDumper, tools?.lpunpack, tools?.simg2img].filter(Boolean).map((tool) => (
            <div className="beta-tool-card" key={tool!.name}>
              <span>{tool!.name}</span>
              <strong className={tool!.available ? "final-ready" : "final-blocked"}>
                {tool!.available ? "Available" : "Missing"}
              </strong>
              <code>{tool!.path}</code>
              <small>{tool!.source}</small>
            </div>
          ))}
          <button type="button" className="button button-secondary" onClick={() => void refreshTools()} disabled={toolBusy}>
            {toolBusy ? "Checking…" : "Recheck Tools"}
          </button>
        </div>

        <div className="beta-path-grid">
          <label>
            <span>Source</span>
            <input
              value={sourcePath}
              onChange={(event) => setSourcePath(event.target.value)}
              placeholder="D:\\ROM\\payload.bin  or  D:\\ROM\\super.img"
              spellCheck={false}
            />
          </label>
          <label>
            <span>Empty preparation workspace</span>
            <input
              value={prepWorkspace}
              onChange={(event) => setPrepWorkspace(event.target.value)}
              placeholder="D:\\FlashROM-Work\\prepared-rom"
              spellCheck={false}
            />
          </label>
        </div>

        <div className="beta-action-row">
          <button
            type="button"
            className="button button-primary"
            disabled={prepBusy || !sourcePath.trim() || !prepWorkspace.trim() || tools?.payloadReady === false}
            onClick={() => void prepare("payload")}
          >
            {prepBusy ? "Preparing…" : "Prepare payload.bin / OTA ZIP"}
          </button>
          <button
            type="button"
            className="button button-primary"
            disabled={prepBusy || !sourcePath.trim() || !prepWorkspace.trim() || tools?.superReady === false}
            onClick={() => void prepare("super")}
          >
            {prepBusy ? "Preparing…" : "Prepare super.img"}
          </button>
        </div>

        <p className="operation-note">
          Payload extraction keeps verification enabled and only extracts FlashROM's partition allowlist. Raw super.img remains non-flashable; use this preparation path to unpack logical images first.
        </p>

        {prepared && (
          <div className="beta-prepared-result guard-ready">
            <div>
              <span>Prepared input</span>
              <strong>{prepared.artifacts.length} image(s) · {formatBytes(prepared.totalBytes)}</strong>
              <code>{prepared.destination}</code>
            </div>
            <p>{prepared.diagnostic}</p>
            {prepared.ignoredImageCount > 0 && (
              <small>{prepared.ignoredImageCount} unsupported image(s) quarantined under _ignored_partitions.</small>
            )}
            <div className="beta-artifact-list">
              {prepared.artifacts.slice(0, 24).map((artifact) => (
                <div key={artifact.path}>
                  <code>{artifact.name}</code>
                  <span>{formatBytes(artifact.size)}</span>
                </div>
              ))}
            </div>
            <strong>Next: drag the prepared directory into the main ROM drop zone and rebuild Final Flash Plan.</strong>
          </div>
        )}

        {prepLog.length > 0 && <pre className="beta-log">{prepLog.join("\n")}</pre>}
      </section>

      <section className="panel beta-vault-panel">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Post-flash restore</p>
            <h2>Obtainium / F-Droid Config Vault</h2>
          </div>
          <p>
            Preserve an exported manager configuration before a clean flash, pin it by SHA-256, then stage the verified file back to Downloads after Android boots.
          </p>
        </div>

        <div className="beta-path-grid">
          <label>
            <span>Restore workspace</span>
            <input
              value={restoreWorkspace}
              onChange={(event) => {
                setRestoreWorkspace(event.target.value);
                setVault(null);
                setStageResult(null);
              }}
              placeholder="D:\\FlashROM-Backup\\apps"
              spellCheck={false}
            />
          </label>
          <label>
            <span>Manager</span>
            <select value={manager} onChange={(event) => setManager(event.target.value as SourceManagerId)}>
              <option value="obtainium">Obtainium</option>
              <option value="fdroid">F-Droid</option>
            </select>
          </label>
          <button type="button" className="button button-secondary" disabled={!restoreWorkspace.trim() || vaultBusy} onClick={() => void refreshVault()}>
            Load Vault
          </button>
        </div>

        <div className="beta-path-grid">
          <label>
            <span>Exported config file</span>
            <input
              value={configPath}
              onChange={(event) => setConfigPath(event.target.value)}
              placeholder="D:\\Backups\\obtainium-export.json"
              spellCheck={false}
            />
          </label>
          <button type="button" className="button button-secondary" disabled={!restoreWorkspace.trim() || !configPath.trim() || vaultBusy} onClick={() => void saveConfig()}>
            Save Export to Vault
          </button>
        </div>

        {vault && (
          <div className="beta-vault-list">
            {vault.entries.length === 0 ? (
              <p className="operation-note">Vault is empty.</p>
            ) : (
              vault.entries.map((entry) => (
                <div className="beta-vault-entry" key={entry.manager}>
                  <strong>{entry.label}</strong>
                  <code>{entry.localPath}</code>
                  <small>{formatBytes(entry.size)} · sha256 {entry.sha256.slice(0, 20)}…</small>
                </div>
              ))
            )}
          </div>
        )}

        <div className="beta-stage-card">
          <div className="beta-path-grid">
            <label>
              <span>ADB device</span>
              <select value={serial} onChange={(event) => setSerial(event.target.value)} disabled={vaultBusy}>
                <option value="">Select normal Android device</option>
                {devices.map((device) => (
                  <option key={`${device.tool}-${device.serial}`} value={device.serial ?? ""}>
                    {device.serial} · {device.product ?? "unknown"} · {device.mode}
                  </option>
                ))}
              </select>
            </label>
            <button type="button" className="button button-secondary" onClick={() => void refreshDevices()} disabled={vaultBusy}>
              Refresh Devices
            </button>
          </div>
          <label>
            Type <strong>STAGE CONFIG</strong> to copy the pinned export to the device
            <input
              className="confirm-input"
              value={stageConfirmation}
              onChange={(event) => setStageConfirmation(event.target.value)}
              placeholder="STAGE CONFIG"
              spellCheck={false}
            />
          </label>
          <button
            type="button"
            className="button button-primary"
            disabled={!selectedEntry || !serial || stageConfirmation !== "STAGE CONFIG" || vaultBusy}
            onClick={() => void stageConfig()}
          >
            Stage Config to Device
          </button>
        </div>

        {stageResult && (
          <div className="beta-prepared-result guard-ready">
            <strong>{stageResult.managerInstalled ? "Manager installed" : "Manager APK still missing"}</strong>
            <code>{stageResult.remotePath}</code>
            <p>{stageResult.diagnostic}</p>
            <p>{stageResult.importHint}</p>
          </div>
        )}

        {statusMessage && <p className="restore-diagnostic">{statusMessage}</p>}
      </section>
    </main>
  );
}

export default BetaPreparationCenter;
