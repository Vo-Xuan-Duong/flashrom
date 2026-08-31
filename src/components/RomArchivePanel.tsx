import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";

interface ZipEntryInfo {
  name: string;
  size: number;
  compressedSize: number;
  kind: string;
}

interface ZipInspection {
  path: string;
  kind: string;
  entryCount: number;
  decompressedSize: number | null;
  hasPayload: boolean;
  hasRecoveryMetadata: boolean;
  hasFastbootImages: boolean;
  entries: ZipEntryInfo[];
  diagnostic: string;
}

interface ZipExtractionResult {
  source: string;
  destination: string;
  extractedFiles: string[];
  extractedBytes: number;
  payloadExtracted: boolean;
  imageCount: number;
  diagnostic: string;
}

function formatBytes(bytes: number | null) {
  if (bytes === null) return "Unknown";
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** index;
  return `${value >= 10 || index === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[index]}`;
}

function RomArchivePanel() {
  const [zipPath, setZipPath] = useState("");
  const [destination, setDestination] = useState("");
  const [inspection, setInspection] = useState<ZipInspection | null>(null);
  const [extraction, setExtraction] = useState<ZipExtractionResult | null>(null);
  const [busy, setBusy] = useState(false);

  async function inspect() {
    if (!zipPath.trim() || busy) return;
    setBusy(true);
    setExtraction(null);
    try {
      setInspection(await invoke<ZipInspection>("inspect_rom_zip", { path: zipPath.trim() }));
    } finally {
      setBusy(false);
    }
  }

  async function extract() {
    if (!inspection || !destination.trim() || busy) return;
    setBusy(true);
    try {
      setExtraction(
        await invoke<ZipExtractionResult>("extract_rom_zip_inputs", {
          path: inspection.path,
          destination: destination.trim(),
        }),
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="app-shell archive-panel-shell">
      <section className="panel archive-panel">
        <div className="section-heading">
          <div>
            <p className="eyebrow">ROM coverage</p>
            <h2>ZIP Package Inspector</h2>
          </div>
          <p>Inspect OTA/recovery/fastboot ZIPs without running scripts or flashing partitions.</p>
        </div>

        <div className="archive-inputs">
          <label>
            <span>ROM ZIP path</span>
            <input
              value={zipPath}
              onChange={(event) => {
                setZipPath(event.target.value);
                setInspection(null);
                setExtraction(null);
              }}
              placeholder="D:\\ROM\\rom.zip"
              spellCheck={false}
            />
          </label>
          <button type="button" className="button button-primary" disabled={!zipPath.trim() || busy} onClick={() => void inspect()}>
            {busy ? "Working…" : "Inspect ZIP"}
          </button>
        </div>

        {inspection && (
          <div className="archive-result">
            <div className="restore-summary-grid">
              <div><span>Type</span><strong>{inspection.kind}</strong></div>
              <div><span>Entries</span><strong>{inspection.entryCount}</strong></div>
              <div><span>Expanded size</span><strong>{formatBytes(inspection.decompressedSize)}</strong></div>
              <div><span>payload.bin</span><strong>{inspection.hasPayload ? "Yes" : "No"}</strong></div>
              <div><span>Fastboot images</span><strong>{inspection.hasFastbootImages ? "Yes" : "No"}</strong></div>
              <div><span>Recovery metadata</span><strong>{inspection.hasRecoveryMetadata ? "Yes" : "No"}</strong></div>
            </div>
            <p>{inspection.diagnostic}</p>

            <div className="artifact-list">
              {inspection.entries.map((entry) => (
                <div className="artifact-row" key={entry.name}>
                  <code>{entry.name}</code>
                  <span>{entry.kind}</span>
                  <span>{formatBytes(entry.size)}</span>
                </div>
              ))}
            </div>

            <div className="archive-extract">
              <label>
                <span>Safe extraction destination</span>
                <input value={destination} onChange={(event) => setDestination(event.target.value)} placeholder="D:\\FlashROM-Workspace\\rom" spellCheck={false} />
              </label>
              <button type="button" className="button button-secondary" disabled={!destination.trim() || busy} onClick={() => void extract()}>
                Extract ROM Inputs
              </button>
              <small>
                Only metadata, payload.bin and .img inputs are extracted. ZIP scripts are never executed automatically.
              </small>
            </div>
          </div>
        )}

        {extraction && (
          <div className="archive-result guard-ready">
            <strong>{extraction.diagnostic}</strong>
            <small>{formatBytes(extraction.extractedBytes)} extracted · {extraction.imageCount} image(s)</small>
            {extraction.payloadExtracted && (
              <p className="operation-note">
                payload.bin is available in the workspace, but FlashROM intentionally blocks automatic partition extraction until update_engine payload parsing is implemented and validated.
              </p>
            )}
            <div className="artifact-list">
              {extraction.extractedFiles.slice(0, 64).map((path) => <div className="artifact-row" key={path}><code>{path}</code></div>)}
            </div>
          </div>
        )}
      </section>
    </main>
  );
}

export default RomArchivePanel;
