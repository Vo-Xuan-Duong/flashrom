import { useState, type ReactNode } from "react";
import {
  Archive,
  Boxes,
  CircleCheck,
  Gauge,
  RotateCcw,
  ShieldCheck,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import App from "./App";
import BetaPreparationCenter from "./components/BetaPreparationCenter";
import PlatformToolsPanel from "./components/PlatformToolsPanel";
import RecoveryCenter from "./components/RecoveryCenter";
import RomArchivePanel from "./components/RomArchivePanel";

type View = "flash" | "prepare" | "archive" | "recovery" | "tools";

const navigation: Array<{ id: View; label: string; detail: string; icon: LucideIcon }> = [
  { id: "flash", label: "Flash workspace", detail: "Device & flash plan", icon: Gauge },
  { id: "prepare", label: "ROM preparation", detail: "Payload & super images", icon: Boxes },
  { id: "archive", label: "ZIP inspector", detail: "Inspect safely", icon: Archive },
  { id: "recovery", label: "Recovery center", detail: "Journals & retry", icon: RotateCcw },
  { id: "tools", label: "Platform tools", detail: "ADB & Fastboot", icon: Wrench },
];

const viewMeta: Record<View, { eyebrow: string; title: string; description: string }> = {
  flash: { eyebrow: "Operation workspace", title: "Flash with confidence", description: "Verify the device, inspect the ROM, then build an explicit guarded plan." },
  prepare: { eyebrow: "Input preparation", title: "Prepare ROM artifacts", description: "Convert supported containers locally before they enter a flash plan." },
  archive: { eyebrow: "Safe inspection", title: "Review archive contents", description: "Read ROM ZIP contents without running any embedded scripts." },
  recovery: { eyebrow: "Protected recovery", title: "Review and recover operations", description: "Inspect journals and restart interrupted operations safely." },
  tools: { eyebrow: "System readiness", title: "Validate your toolchain", description: "Confirm the exact ADB and Fastboot binaries FlashROM will use." },
};

function Workspace() {
  const [view, setView] = useState<View>("flash");
  const content: Record<View, ReactNode> = {
    flash: <App />,
    prepare: <BetaPreparationCenter />,
    archive: <RomArchivePanel />,
    recovery: <RecoveryCenter />,
    tools: <PlatformToolsPanel />,
  };
  const meta = viewMeta[view];

  return (
    <div className="workspace">
      <aside className="workspace-sidebar">
        <div className="brand">
          <span className="brand-mark">F</span>
          <span>FlashROM</span>
          <small>beta</small>
        </div>
        <nav className="workspace-nav" aria-label="Main navigation">
          <p>Workspace</p>
          {navigation.map((item) => (
            <button
              key={item.id}
              className={`nav-item ${view === item.id ? "nav-item-active" : ""}`}
              type="button"
              onClick={() => setView(item.id)}
            >
              <span className="nav-icon" aria-hidden="true"><item.icon size={17} strokeWidth={1.8} /></span>
              <span><strong>{item.label}</strong><small>{item.detail}</small></span>
            </button>
          ))}
        </nav>
        <div className="sidebar-footer">
          <CircleCheck size={14} /> Local safety checks enabled
        </div>
      </aside>

      <div className="workspace-main">
        <header className="workspace-header">
          <div>
            <p className="eyebrow">{meta.eyebrow}</p>
            <h1>{meta.title}</h1>
            <p className="workspace-description">{meta.description}</p>
          </div>
          <div className="safety-badge"><ShieldCheck size={15} /> Guarded operations</div>
        </header>
        <div className="workspace-content">{content[view]}</div>
      </div>
    </div>
  );
}

export default Workspace;
