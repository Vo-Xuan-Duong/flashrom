import { useEffect, useState } from "react";
import {
  buildExecutionPreview,
  resolveFinalFlashPlan,
  type ExecutionPreview,
  type FinalFlashPlan,
  type SlotStrategy,
} from "../lib/tauri";

interface FinalPlanPanelProps {
  romPath: string | null;
  serial: string | null;
  deviceProduct: string | null;
  onLog: (message: string) => void;
}

function formatBytes(bytes: number | null) {
  if (bytes === null) return "Unknown";
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** index;
  return `${value >= 10 || index === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[index]}`;
}

function statusLabel(status: string) {
  if (status === "matched") return "Matched";
  if (status === "mismatch") return "Mismatch";
  return "Unknown";
}

function stepStateLabel(state: string) {
  if (state === "ready") return "Ready";
  if (state === "manual_only") return "Manual only";
  if (state === "blocked") return "Blocked";
  return state;
}

function actionLabel(kind: string) {
  const labels: Record<string, string> = {
    preflight: "Preflight",
    mode_transition: "Mode transition",
    revalidate_step: "Revalidate",
    flash_preview: "Flash preview",
    post_write_check: "Post-write check",
    finish: "Finish",
  };
  return labels[kind] ?? kind;
}

function FinalPlanPanel({ romPath, serial, deviceProduct, onLog }: FinalPlanPanelProps) {
  const [slotStrategy, setSlotStrategy] = useState<SlotStrategy>("active");
  const [plan, setPlan] = useState<FinalFlashPlan | null>(null);
  const [dryRun, setDryRun] = useState<ExecutionPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [dryRunBusy, setDryRunBusy] = useState(false);

  useEffect(() => {
    setPlan(null);
    setDryRun(null);
  }, [romPath, serial, deviceProduct, slotStrategy]);

  async function resolvePlan() {
    if (!romPath || !serial || busy) return;

    setBusy(true);
    setDryRun(null);
    try {
      const result = await resolveFinalFlashPlan({
        path: romPath,
        serial,
        slotStrategy,
      });
      setPlan(result);
      onLog(
        `Final flash plan resolved: ${result.steps.length} step(s), compatibility=${result.compatibility.status}, ready=${result.readyForExecution}.`,
      );
    } catch (error) {
      setPlan(null);
      onLog(`Final plan resolution failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function buildDryRun() {
    if (!romPath || !serial || dryRunBusy) return;

    setDryRunBusy(true);
    try {
      const result = await buildExecutionPreview({
        path: romPath,
        serial,
        slotStrategy,
      });
      setPlan(result.finalPlan);
      setDryRun(result);
      onLog(
        `Full-ROM dry run generated: ${result.actions.length} action(s), automaticExecutionEnabled=${result.automaticExecutionEnabled}.`,
      );
    } catch (error) {
      setDryRun(null);
      onLog(`Execution dry run failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setDryRunBusy(false);
    }
  }

  return (
    <section className="panel final-plan-panel">
      <div className="section-heading">
        <div>
          <p className="eyebrow">Device-validated plan</p>
          <h2>Final Flash Plan</h2>
        </div>
        <p>
          Re-reads Fastboot metadata and ROM codename requirements. Full-ROM writes remain disabled.
        </p>
      </div>

      <div className="final-plan-controls">
        <div className="plan-source">
          <span>ROM input</span>
          <strong>{romPath ?? "Drop a ROM first"}</strong>
        </div>

        <div className="slot-strategy">
          <button
            type="button"
            className={`layout-option ${slotStrategy === "active" ? "layout-option-active" : ""}`}
            onClick={() => setSlotStrategy("active")}
            disabled={!romPath}
          >
            <strong>Active slot</strong>
            <span>Use current Fastboot slot</span>
          </button>
          <button
            type="button"
            className={`layout-option ${slotStrategy === "both" ? "layout-option-active" : ""}`}
            onClick={() => setSlotStrategy("both")}
            disabled={!romPath}
          >
            <strong>Both slots</strong>
            <span>Resolve every A/B target</span>
          </button>
        </div>

        <button
          type="button"
          className="button button-primary"
          disabled={!romPath || !serial || busy}
          onClick={() => void resolvePlan()}
        >
          {busy ? "Resolving…" : "Validate & Resolve"}
        </button>
      </div>

      {!serial && romPath && (
        <div className="plan-empty">Connect the device through Fastboot/FastbootD before final validation.</div>
      )}

      {plan && (
        <div className="final-plan-result">
          <div className={`compatibility-card compatibility-${plan.compatibility.status}`}>
            <div>
              <span>Compatibility</span>
              <strong>{statusLabel(plan.compatibility.status)}</strong>
            </div>
            <div>
              <span>Device product</span>
              <code>{plan.compatibility.deviceProduct ?? deviceProduct ?? "Unknown"}</code>
            </div>
            <div>
              <span>ROM products</span>
              <code>
                {plan.compatibility.romProducts.length
                  ? plan.compatibility.romProducts.join(", ")
                  : "No trusted metadata"}
              </code>
            </div>
          </div>
          <p className="final-diagnostic">{plan.compatibility.diagnostic}</p>

          <div className="final-preflight-grid">
            <div>
              <span>Bootloader</span>
              <strong>
                {plan.bootloaderUnlocked === true
                  ? "Unlocked"
                  : plan.bootloaderUnlocked === false
                    ? "Locked"
                    : "Unknown"}
              </strong>
            </div>
            <div>
              <span>Active slot</span>
              <strong>{plan.activeSlot?.toUpperCase() ?? "Unknown"}</strong>
            </div>
            <div>
              <span>Current mode</span>
              <strong>{plan.currentMode}</strong>
            </div>
            <div>
              <span>Snapshot</span>
              <strong>{plan.snapshotUpdateStatus ?? "Unknown / not reported"}</strong>
            </div>
            <div>
              <span>Mode phases</span>
              <strong>{plan.requiresModeSwitch ? "Fastboot → FastbootD" : "Single mode"}</strong>
            </div>
            <div>
              <span>Final state</span>
              <strong className={plan.readyForExecution ? "final-ready" : "final-blocked"}>
                {plan.readyForExecution ? "Ready" : "Blocked"}
              </strong>
            </div>
          </div>

          {plan.compatibility.evidence.length > 0 && (
            <div className="compatibility-evidence">
              <span>ROM product evidence</span>
              {plan.compatibility.evidence.map((item, index) => (
                <div key={`${item.source}-${item.key}-${item.product}-${index}`}>
                  <code>{item.product}</code>
                  <small>{item.key}</small>
                  <small title={item.source}>{item.source}</small>
                </div>
              ))}
            </div>
          )}

          {plan.warnings.length > 0 && (
            <div className="plan-warnings">
              {plan.warnings.map((warning, index) => (
                <p key={`${warning}-${index}`}>{warning}</p>
              ))}
            </div>
          )}

          <div className="final-step-list">
            {plan.steps.map((step, index) => (
              <article className="final-step" key={`${step.imagePath}-${step.partition}-${index}`}>
                <div className="final-step-heading">
                  <div>
                    <span className="plan-index">{String(index + 1).padStart(2, "0")}</span>
                    <code>{step.image}</code>
                    <span className="plan-arrow">→</span>
                    <code>{step.partition}</code>
                  </div>
                  <span className={`final-step-state final-step-${step.state}`}>
                    {stepStateLabel(step.state)}
                  </span>
                </div>

                <div className="final-step-meta">
                  <span>Phase {step.phase || "—"}</span>
                  <span>{step.requiredMode}</span>
                  <span>{step.logical === true ? "Logical" : step.logical === false ? "Physical" : "Unknown"}</span>
                  <span>{formatBytes(step.imageSize)} image</span>
                  <span>{formatBytes(step.partitionSize)} partition</span>
                </div>

                <div className="command-preview">
                  <span>Validated command preview</span>
                  <code>{step.commandPreview}</code>
                </div>

                {step.warning && <p className="plan-step-warning">{step.warning}</p>}
              </article>
            ))}
          </div>

          <div className={`final-execution-gate ${plan.readyForExecution ? "gate-ready" : "gate-blocked"}`}>
            <strong>
              {plan.readyForExecution
                ? "All current safety checks passed."
                : "Full-ROM execution remains blocked."}
            </strong>
            <span>
              The dry run below re-resolves the plan and previews mode transitions and every future guarded write.
              It never executes `fastboot flash`.
            </span>
            <button
              type="button"
              className="button button-secondary"
              disabled={!serial || !romPath || dryRunBusy}
              onClick={() => void buildDryRun()}
            >
              {dryRunBusy ? "Building dry run…" : "Build Execution Dry Run"}
            </button>
          </div>

          {dryRun && (
            <div className="execution-dry-run">
              <div className="partition-probe-heading">
                <div>
                  <span>Non-executing sequence</span>
                  <strong>Full-ROM Execution Dry Run</strong>
                </div>
                <span className={dryRun.blockedReason ? "final-blocked" : "final-ready"}>
                  {dryRun.blockedReason ? "Blocked" : `${dryRun.actions.length} actions`}
                </span>
              </div>

              <p>{dryRun.diagnostic}</p>
              {dryRun.blockedReason && <p className="plan-step-warning">{dryRun.blockedReason}</p>}

              <div className="execution-action-list">
                {dryRun.actions.map((action) => (
                  <article className="execution-action" key={`${action.index}-${action.kind}`}>
                    <div className="execution-action-heading">
                      <span className="plan-index">{String(action.index).padStart(2, "0")}</span>
                      <strong>{actionLabel(action.kind)}</strong>
                      {action.mode && <span>{action.mode}</span>}
                      {action.partition && <code>{action.partition}</code>}
                    </div>
                    <p>{action.description}</p>
                    {action.commandPreview && (
                      <div className="command-preview">
                        <span>Dry-run command</span>
                        <code>{action.commandPreview}</code>
                      </div>
                    )}
                  </article>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </section>
  );
}

export default FinalPlanPanel;
