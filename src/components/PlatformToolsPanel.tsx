import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

interface ToolStatus {
  name: string;
  path: string;
  available: boolean;
  version: string | null;
  diagnostic: string;
}

interface PlatformToolsStatus {
  source: string;
  adb: ToolStatus;
  fastboot: ToolStatus;
  ready: boolean;
  diagnostic: string;
}

function ToolCard({ tool }: { tool: ToolStatus }) {
  return (
    <article className={`tool-card ${tool.available ? "guard-ready" : "guard-blocked"}`}>
      <div>
        <span>{tool.name.toUpperCase()}</span>
        <strong>{tool.available ? "Available" : "Unavailable"}</strong>
      </div>
      <code>{tool.path}</code>
      <small>{tool.version ?? tool.diagnostic}</small>
    </article>
  );
}

function PlatformToolsPanel() {
  const [status, setStatus] = useState<PlatformToolsStatus | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    if (busy) return;
    setBusy(true);
    try {
      setStatus(await invoke<PlatformToolsStatus>("inspect_platform_tools"));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  return (
    <main className="app-shell platform-tools-shell">
      <section className="panel platform-tools-panel">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Environment</p>
            <h2>Android Platform Tools</h2>
          </div>
          <button type="button" className="button button-secondary" disabled={busy} onClick={() => void refresh()}>
            {busy ? "Checking…" : "Test Tools"}
          </button>
        </div>

        {status ? (
          <>
            <div className={`platform-status ${status.ready ? "guard-ready" : "guard-blocked"}`}>
              <strong>{status.ready ? "Platform Tools ready" : "Platform Tools incomplete"}</strong>
              <span>Source: {status.source}</span>
              <p>{status.diagnostic}</p>
            </div>
            <div className="tool-grid">
              <ToolCard tool={status.adb} />
              <ToolCard tool={status.fastboot} />
            </div>
            <small className="operation-note">
              Resolution order: FLASHROM_PLATFORM_TOOLS → tools/platform-tools → system PATH.
            </small>
          </>
        ) : (
          <p className="operation-note">Platform Tools have not been checked yet.</p>
        )}
      </section>
    </main>
  );
}

export default PlatformToolsPanel;
