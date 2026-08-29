import { useEffect, useState } from "react";
import {
  generateFlashPlan,
  type BootLayout,
  type FlashPlan,
  type SlotStrategy,
} from "../lib/tauri";

interface FlashPlanPanelProps {
  romPath: string | null;
  bootLayout: BootLayout;
  activeSlot: string | null;
  serial: string | null;
  onLog: (message: string) => void;
}

function stateLabel(state: string) {
  const labels: Record<string, string> = {
    resolved: "Resolved",
    blocked: "Blocked",
    needs_partition_metadata: "Needs partition check",
    needs_compatibility_check: "Needs compatibility check",
    unsupported: "Unsupported",
  };
  return labels[state] ?? state;
}

function FlashPlanPanel({
  romPath,
  bootLayout,
  activeSlot,
  serial,
  onLog,
}: FlashPlanPanelProps) {
  const [slotStrategy, setSlotStrategy] = useState<SlotStrategy>("active");
  const [plan, setPlan] = useState<FlashPlan | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setPlan(null);
    if (bootLayout !== "ab") setSlotStrategy("active");
  }, [romPath, bootLayout, activeSlot, serial]);

  async function buildPlan() {
    if (!romPath || busy) return;

    setBusy(true);
    try {
      const result = await generateFlashPlan({
        path: romPath,
        bootLayout,
        activeSlot,
        slotStrategy,
        serial,
      });
      setPlan(result);
      onLog(
        `Flash plan preview generated: ${result.steps.length} step(s), ${result.warnings.length} warning(s).`,
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setPlan(null);
      onLog(`Flash plan generation failed: ${message}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel">
      <div className="section-heading">
        <div>
          <p className="eyebrow">Validation stage</p>
          <h2>Flash Plan Preview</h2>
        </div>
        <p>No command in this section is executable yet. It only resolves expected targets and required modes.</p>
      </div>

      <div className="plan-controls">
        <div className="plan-source">
          <span>ROM input</span>
          <strong>{romPath ?? "Drop a ROM above first"}</strong>
        </div>

        <div className="slot-strategy" aria-label="A/B slot strategy">
          <button
            type="button"
            className={`layout-option ${slotStrategy === "active" ? "layout-option-active" : ""}`}
            onClick={() => {
              setSlotStrategy("active");
              setPlan(null);
            }}
            disabled={!romPath}
          >
            <strong>Active slot</strong>
            <span>{activeSlot ? `Current: ${activeSlot.toUpperCase()}` : "Use detected active slot"}</span>
          </button>
          <button
            type="button"
            className={`layout-option ${slotStrategy === "both" ? "layout-option-active" : ""}`}
            onClick={() => {
              setSlotStrategy("both");
              setPlan(null);
            }}
            disabled={!romPath || bootLayout !== "ab"}
          >
            <strong>Both slots</strong>
            <span>Resolve boot_a + boot_b</span>
          </button>
        </div>

        <button
          type="button"
          className="button button-primary"
          onClick={() => void buildPlan()}
          disabled={!romPath || busy}
        >
          {busy ? "Building…" : "Build Flash Plan"}
        </button>
      </div>

      {!plan && (
        <div className="plan-empty">
          {romPath
            ? "Build the plan to resolve image → partition mappings."
            : "No ROM selected. Drop a ROM file or extracted ROM directory first."}
        </div>
      )}

      {plan && (
        <div className="plan-result">
          <div className="plan-summary">
            <div>
              <span>ROM type</span>
              <strong>{plan.romKind}</strong>
            </div>
            <div>
              <span>Boot layout</span>
              <strong>{plan.bootLayout === "ab" ? "A/B" : plan.bootLayout}</strong>
            </div>
            <div>
              <span>Strategy</span>
              <strong>{plan.slotStrategy === "both" ? "Both slots" : "Active slot"}</strong>
            </div>
            <div>
              <span>Plan state</span>
              <strong>{plan.readyForValidation ? "Ready for metadata validation" : "More checks required"}</strong>
            </div>
          </div>

          {plan.warnings.length > 0 && (
            <div className="plan-warnings">
              {plan.warnings.map((warning, index) => (
                <p key={`${warning}-${index}`}>{warning}</p>
              ))}
            </div>
          )}

          <div className="plan-step-list">
            {plan.steps.length === 0 ? (
              <div className="plan-empty">No direct partition flash steps were generated.</div>
            ) : (
              plan.steps.map((step, index) => (
                <article className="plan-step" key={`${step.imagePath}-${step.partition}-${index}`}>
                  <div className="plan-step-heading">
                    <div>
                      <span className="plan-index">{String(index + 1).padStart(2, "0")}</span>
                      <code>{step.image}</code>
                      <span className="plan-arrow">→</span>
                      <code>{step.partition}</code>
                    </div>
                    <span className={`plan-state plan-state-${step.state}`}>{stateLabel(step.state)}</span>
                  </div>

                  <div className="plan-meta">
                    <span>Required mode: {step.requiredMode}</span>
                  </div>

                  <div className="command-preview">
                    <span>Command preview</span>
                    <code>{step.commandPreview}</code>
                  </div>

                  {step.warning && <p className="plan-step-warning">{step.warning}</p>}
                </article>
              ))
            )}
          </div>
        </div>
      )}
    </section>
  );
}

export default FlashPlanPanel;
