import "./fonts";
import "./styles.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { useSession } from "./state/session";

// A paired screen ignores pairing links; keep the unused code out of the address bar and history.
function dropPairingLink(): void {
  if (/[#&]pair=/.test(window.location.hash) && useSession.getState().session) {
    window.history.replaceState(null, "", window.location.pathname + window.location.search);
  }
}
dropPairingLink();
window.addEventListener("hashchange", dropPairingLink);

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
