import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/inter";
import { App } from "./App";
import { I18nProvider, useI18n } from "./i18n";
import { TooltipProvider } from "./components/ui/tooltip";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { installBrowserPreviewStub } from "./lib/browserPreviewStub";
import "./index.css";

// Must run before any component touches the Tauri API surface.
installBrowserPreviewStub();

/** Last-resort boundary around the whole app (the per-page boundary in App.tsx
 *  only covers <main>). If even the shell crashes, show a simple localized
 *  fallback telling the user to restart the app. */
function RootBoundary({ children }: { children: React.ReactNode }) {
  const { t } = useI18n();
  return (
    <ErrorBoundary title={t("app.rootCrashed")} retryLabel={t("common.retry")}>
      {children}
    </ErrorBoundary>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <I18nProvider>
      <RootBoundary>
        <TooltipProvider delayDuration={300}>
          <App />
        </TooltipProvider>
      </RootBoundary>
    </I18nProvider>
  </React.StrictMode>
);
