import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/inter";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import { TooltipProvider } from "./components/ui/tooltip";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <I18nProvider>
      <TooltipProvider delayDuration={300}>
        <App />
      </TooltipProvider>
    </I18nProvider>
  </React.StrictMode>
);