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

type AppTab = "new" | "history" | "settings";

function App() {
  const [activeTab, setActiveTab] = useState<AppTab>("new");
  const [visitedTabs, setVisitedTabs] = useState<Set<AppTab>>(() => new Set(["new"]));
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

  const selectTab = (tab: AppTab) => {
    setActiveTab(tab);
    setVisitedTabs((current) => {
      if (current.has(tab)) {
        return current;
      }
      const next = new Set(current);
      next.add(tab);
      return next;
    });
  };

  return (
    <div className="app-shell">
      <header className="app-header">
        <h1 className="app-header__brand">{APP_NAME}</h1>
        <p className="app-header__tagline">Enregistrez, importez, compte-rendu.</p>
        <nav className="app-nav" aria-label="Navigation principale">
          <button
            type="button"
            className={activeTab === "new" ? "app-nav__tab app-nav__tab--active" : "app-nav__tab"}
            aria-current={activeTab === "new" ? "page" : undefined}
            aria-controls="app-view-new"
            onClick={() => selectTab("new")}
          >
            Réunion
          </button>
          <button
            type="button"
            className={
              activeTab === "history" ? "app-nav__tab app-nav__tab--active" : "app-nav__tab"
            }
            aria-current={activeTab === "history" ? "page" : undefined}
            aria-controls="app-view-history"
            onClick={() => selectTab("history")}
          >
            Historique
          </button>
          <button
            type="button"
            className={
              activeTab === "settings" ? "app-nav__tab app-nav__tab--active" : "app-nav__tab"
            }
            aria-current={activeTab === "settings" ? "page" : undefined}
            aria-controls="app-view-settings"
            onClick={() => selectTab("settings")}
          >
            Réglages
          </button>
        </nav>
      </header>

      <main className="app-main">
        <section id="app-view-new" className="app-view" hidden={activeTab !== "new"}>
          <MeetingWorkspace />
        </section>
        {visitedTabs.has("history") ? (
          <section id="app-view-history" className="app-view" hidden={activeTab !== "history"}>
            <MeetingHistory />
          </section>
        ) : null}
        {visitedTabs.has("settings") ? (
          <section
            id="app-view-settings"
            className="app-view app-settings"
            hidden={activeTab !== "settings"}
          >
            <AiProviderSettings />
            <PrivacySettings />
          </section>
        ) : null}
      </main>

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
    </div>
  );
}

export default App;
