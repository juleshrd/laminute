import { useEffect, useState } from "react";
import type { Update } from "@tauri-apps/plugin-updater";

import { applyAppUpdate, checkForAppUpdate, type UpdateProgress } from "./lib/updater";
import {
  applyReduceMotionToDocument,
  isOnboardingDone,
  setOnboardingDone,
} from "./lib/preferences";
import { LmShell, type AppScreen } from "./components/LmShell";
import { MeetingHistory } from "./components/MeetingHistory";
import { MeetingWorkspace } from "./components/MeetingWorkspace";
import { OnboardingIA } from "./components/OnboardingIA";
import { SettingsScreen } from "./components/SettingsScreen";
import { UpdateAvailableModal } from "./components/UpdateAvailableModal";
import "./App.css";

function App() {
  const [activeScreen, setActiveScreen] = useState<AppScreen>("meeting");
  const [showOnboarding, setShowOnboarding] = useState(() => !isOnboardingDone());
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<UpdateProgress | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);

  useEffect(() => {
    applyReduceMotionToDocument();
  }, []);

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

  function finishOnboarding() {
    setOnboardingDone(true);
    setShowOnboarding(false);
    setActiveScreen("meeting");
  }

  if (showOnboarding) {
    return (
      <div className="lm-root">
        <OnboardingIA onComplete={finishOnboarding} onSkip={finishOnboarding} />
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

  return (
    <div className="lm-root">
      <LmShell active={activeScreen} onNavigate={setActiveScreen}>
        {activeScreen === "meeting" ? <MeetingWorkspace /> : null}
        {activeScreen === "history" ? <MeetingHistory /> : null}
        {activeScreen === "settings" ? (
          <SettingsScreen
            onReplayOnboarding={() => {
              setShowOnboarding(true);
            }}
          />
        ) : null}
      </LmShell>

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
