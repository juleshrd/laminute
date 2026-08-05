import { useEffect, useState } from "react";
import type { Update } from "@tauri-apps/plugin-updater";

import { APP_NAME } from "./lib/app";
import { applyAppUpdate, checkForAppUpdate, type UpdateProgress } from "./lib/updater";
import { AiProviderSettings } from "./components/AiProviderSettings";
import { MeetingHistory } from "./components/MeetingHistory";
import { MeetingWorkspace } from "./components/MeetingWorkspace";
import { PrivacySettings } from "./components/PrivacySettings";
import { UpdateAvailableModal } from "./components/UpdateAvailableModal";
import "./App.css";

type AppTab = "new" | "history";

function App() {
  const [activeTab, setActiveTab] = useState<AppTab>("new");
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<UpdateProgress | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    void (async () => {
      try {
        const update = await checkForAppUpdate();
        if (!cancelled && update) {
          setPendingUpdate(update);
        }
      } catch {
        // Mode dev / réseau indisponible : pas de toast.
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  const handleApplyUpdate = () => {
    if (!pendingUpdate || updateBusy) {
      return;
    }

    setUpdateBusy(true);
    setUpdateError(null);

    void (async () => {
      try {
        await applyAppUpdate(pendingUpdate, setUpdateProgress);
      } catch (error) {
        const message =
          error instanceof Error ? error.message : "La mise à jour a échoué. Réessayez plus tard.";
        setUpdateError(message);
        setUpdateBusy(false);
        setUpdateProgress(null);
      }
    })();
  };

  return (
    <main className="container">
      <h1>{APP_NAME}</h1>

      <nav className="app-tabs" aria-label="Navigation principale">
        <button
          type="button"
          className={activeTab === "new" ? "app-tabs__tab app-tabs__tab--active" : "app-tabs__tab"}
          onClick={() => setActiveTab("new")}
        >
          Nouvelle réunion
        </button>
        <button
          type="button"
          className={
            activeTab === "history" ? "app-tabs__tab app-tabs__tab--active" : "app-tabs__tab"
          }
          onClick={() => setActiveTab("history")}
        >
          Historique
        </button>
      </nav>

      {activeTab === "new" ? <MeetingWorkspace /> : <MeetingHistory />}

      <details className="ai-settings-collapsible">
        <summary>Réglages IA</summary>
        <AiProviderSettings />
      </details>

      <details className="ai-settings-collapsible">
        <summary>Confidentialité</summary>
        <PrivacySettings />
      </details>

      {pendingUpdate ? (
        <UpdateAvailableModal
          currentVersion={pendingUpdate.currentVersion}
          nextVersion={pendingUpdate.version}
          notes={pendingUpdate.body}
          busy={updateBusy}
          progress={updateProgress}
          error={updateError}
          onConfirm={handleApplyUpdate}
          onCancel={() => {
            if (!updateBusy) {
              setPendingUpdate(null);
              setUpdateError(null);
              setUpdateProgress(null);
            }
          }}
        />
      ) : null}
    </main>
  );
}

export default App;
