import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import WindowPicker from "./WindowPicker";
import "./styles.css";

const RootView = getCurrentWindow().label === "window-picker" ? WindowPicker : App;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <RootView />
  </StrictMode>,
);
