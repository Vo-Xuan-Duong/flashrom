import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import RecoveryCenter from "./components/RecoveryCenter";
import "./styles.css";
import "./rom.css";
import "./plan.css";
import "./sideload.css";
import "./flash.css";
import "./final-plan.css";
import "./guard.css";
import "./restore.css";
import "./restore-profile.css";
import "./executor.css";
import "./recovery-center.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
    <RecoveryCenter />
  </StrictMode>,
);
