import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./styles.css";

if (/Mac/.test(navigator.userAgent)) {
  document.documentElement.classList.add("mac");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App initialSection="settings" />
  </React.StrictMode>,
);
