import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import {
  executeFullRom,
  listDevices,
  type DeviceSnapshot,
  type FullRomExecutionReport,
} from "../lib/tauri";

interface AndroidBootVerification {
  verified: boolean;
  serial: string;
  product: string | null;
  androidRelease: string | null;
  buildFingerprint: string | null;
  bootCompleted: boolean;
  elapsedMs: number;
  diagnostic: string;
}

interface JournalSummary {
  operationId: string;
  serial: string;
  product: string | null;
  romPath: string;
  status: string;
  startedUnixMs: number;
  updatedUnixMs: number;
  completedSteps: number;
  failedSteps: number;
  totalSteps: number;
  recoverable: boolean;
  path: string;
  diagnostic: string;
}

interface JournalStep {
  index: number;
  image: string;
  partition: string;
  requiredMode: string;
  status: string;
  command: string | null;
  exitCode: number | null;
  diagnostic: string;
}

interface JournalRecord extends JournalSummary {
  version: number;
  slotStrategy: "active" | "both";
  cleanDataRequested: boolean;
  rebootRequested: boolean;
  steps: JournalStep[];
}

function formatDate(value: number) {
  if (!value) return "—";
  return new Date(value).toLocaleString();
}

function RecoveryCenter() {
  const [devices, setDevices] = useState<DeviceSnapshot[]>([]);
  const [serial, setSerial] = useState("");
  const [journals, setJournals] = useState<JournalSummary[]>([]);
  const [selectedJournal, setSelectedJournal] = useState<JournalRecord | null>(null);
  const [bootResult, setBootResult] = useState<AndroidBootVerification | null>(null);
  const [retryResult, setRetryResult] = useState<FullRomExecutionReport | null>(null);
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);

  const selectedDevice = useMemo(
    () => devices.find((device) => device.serial === serial) ?? null,
    [devices, serial],
  );

  const requiredConfirmation = selectedJournal?.cleanDataRequested ? "FLASH ROM WIPE" : "FLASH ROM";
  const retrySerialMatches = !selectedJournal || serial === selectedJournal.serial;

  async function refresh() {
    const [deviceResult, journalResult] = await Promise.all([
      listDevices().catch(() => []),
      invoke<JournalSummary[]>("list_execution_journals").catch(() => []),
    ]);
    setDevices(deviceResult);
    setJournals(journalResult);
    if (!serial && deviceResult.length === 1 && deviceResult[0].serial) {
      setSerial(deviceResult[0].serial);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function verifyBoot() {
    if (!serial || busy) return;
    setBusy(true);
    try {
      const result = await invoke<AndroidBootVerification>("verify_android_boot", {
        serial,
        expectedProduct: selectedDevice?.product ?? null,
        timeoutSeconds: 300,
      });
      setBootResult(result);
    } finally {
      setBusy(false);
    }
  }

  async function inspectJournal(path: string) {
    if (busy) return;
    setBusy(true);
    try {
      const result = await invoke<JournalRecord>("inspect_execution_journal", { path });
      setSelectedJournal(result);
      setRetryResult(null);
      setConfirmation("");
      setSerial(result.serial);
    } finally {
      setBusy(false);
    }
  }

  async function retryFromBeginning() {
    if (
      !selectedJournal ||
      !serial ||
      serial !== selectedJournal.serial ||
      confirmation !== requiredConfirmation ||
      busy
    ) {
      return;
    }
    setBusy(true);
    try {
      const result = await executeFullRom({
        path: selectedJournal.romPath,
        serial,
        slotStrategy: selectedJournal.slotStrategy,
        confirmation,
        cleanDataAfter: selectedJournal.cleanDataRequested,
        rebootAfter: selectedJournal.rebootRequested,
      });
      setRetryResult(result);
      setConfirmation("");
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="app-shell recovery-center-shell">
      <section className="panel recovery-center">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Reliability</p>
            <h2>Recovery Center</h2>
          </div>
          <p>Verify Android first boot and inspect/retry interrupted Full-ROM operations from their journals.</p>
        </div>

        <div className="recovery-toolbar">
          <label>
            <span>Device</span>
            <select value={serial} onChange={(event) => setSerial(event.target.value)} disabled={busy}>
              <option value="">Select device</option>
              {devices.map((device) => (
                <option key={`${device.tool}-${device.serial}`} value={device.serial ?? ""}>
                  {device.serial} · {device.product ?? "unknown"} · {device.mode}
                </option>
              ))}
            </select>
          </label>
          <button type="button" className="button button-secondary" onClick={() => void refresh()} disabled={busy}>
            Refresh
          </button>
          <button type="button" className="button button-primary" onClick={() => void verifyBoot()} disabled={!serial || busy}>
            {busy ? "Working…" : "Verify Android Boot"}
          </button>
        </div>

        {bootResult && (
          <div className={`recovery-result ${bootResult.verified ? "guard-ready" : "guard-blocked"}`}>
            <strong>{bootResult.verified ? "Boot verified" : "Boot not verified"}</strong>
            <p>{bootResult.diagnostic}</p>
            <small>
              product={bootResult.product ?? "?"} · Android {bootResult.androidRelease ?? "?"} · boot_completed={bootResult.bootCompleted ? "1" : "0"}
            </small>
            {bootResult.buildFingerprint && <code>{bootResult.buildFingerprint}</code>}
          </div>
        )}

        <div className="journal-section">
          <div className="partition-probe-heading">
            <div>
              <span>Persistent operation history</span>
              <strong>Full-ROM Journals</strong>
            </div>
            <span>{journals.length} found</span>
          </div>

          {journals.length === 0 ? (
            <p className="operation-note">No Full-ROM journals have been created yet.</p>
          ) : (
            <div className="journal-list">
              {journals.slice(0, 12).map((journal) => (
                <button
                  type="button"
                  className={`journal-row ${selectedJournal?.operationId === journal.operationId ? "journal-row-active" : ""}`}
                  key={journal.operationId}
                  onClick={() => void inspectJournal(journal.path)}
                  disabled={busy}
                >
                  <div>
                    <strong>{journal.operationId}</strong>
                    <code>{journal.serial}</code>
                  </div>
                  <span>{journal.status}</span>
                  <span>{journal.completedSteps}/{journal.totalSteps} completed</span>
                  <small>{formatDate(journal.updatedUnixMs)}</small>
                </button>
              ))}
            </div>
          )}
        </div>

        {selectedJournal && (
          <div className="journal-detail">
            <div>
              <span>ROM</span>
              <code>{selectedJournal.romPath}</code>
            </div>
            <div>
              <span>Original device</span>
              <code>{selectedJournal.serial}</code>
            </div>
            <p>{selectedJournal.diagnostic}</p>
            <div className="guard-step-list">
              {selectedJournal.steps.map((step) => (
                <div className="guard-step" key={`${step.index}-${step.partition}`}>
                  <span>{String(step.index).padStart(2, "0")}</span>
                  <code>{step.partition}</code>
                  <small>{step.requiredMode}</small>
                  <strong className={step.status === "success" ? "final-ready" : step.status === "failed" ? "final-blocked" : ""}>
                    {step.status}
                  </strong>
                </div>
              ))}
            </div>

            {selectedJournal.recoverable && (
              <div className="recovery-retry danger-copy">
                <strong>Retry from beginning — never blind-resume</strong>
                <span>
                  FlashROM will rebuild compatibility, partition metadata, ordering and SHA-256 Guard from the current ROM/device state before writing anything again.
                </span>
                {!retrySerialMatches && (
                  <span className="final-blocked">
                    Retry is locked because the selected serial differs from the original journal serial {selectedJournal.serial}.
                  </span>
                )}
                <label>
                  Type <strong>{requiredConfirmation}</strong>
                  <input
                    className="confirm-input"
                    value={confirmation}
                    onChange={(event) => setConfirmation(event.target.value)}
                    disabled={busy || !retrySerialMatches}
                    placeholder={requiredConfirmation}
                  />
                </label>
                <button
                  type="button"
                  className="button button-danger"
                  disabled={
                    !serial ||
                    !retrySerialMatches ||
                    confirmation !== requiredConfirmation ||
                    busy
                  }
                  onClick={() => void retryFromBeginning()}
                >
                  Retry Full ROM From Beginning
                </button>
              </div>
            )}
          </div>
        )}

        {retryResult && (
          <div className={`recovery-result ${retryResult.success ? "guard-ready" : "guard-blocked"}`}>
            <strong>{retryResult.success ? "Retry completed" : "Retry stopped"}</strong>
            <p>{retryResult.diagnostic}</p>
            <small>{retryResult.journalPath}</small>
          </div>
        )}
      </section>
    </main>
  );
}

export default RecoveryCenter;
