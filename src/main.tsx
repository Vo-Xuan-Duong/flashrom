import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
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

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
