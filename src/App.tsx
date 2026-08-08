import { useEffect, useState } from "react";
import type { Update } from "@tauri-apps/plugin-updater";

import { applyAppUpdate, probeAppUpdate, type UpdateProgress } from "./lib/updater";
import {
  applyReduceMotionToDocument,
  isOnboardingDone,
  setOnboardingDone,
} from "./lib/preferences";
import { useMeetingFlow } from "./hooks/useMeetingFlow";
import { LmShell, type AppScreen } from "./components/LmShell";
import { MeetingHistory } from "./components/MeetingHistory";
import { MeetingWorkspace } from "./components/MeetingWorkspace";
import { OnboardingIA } from "./components/OnboardingIA";
import { SettingsScreen } from "./components/SettingsScreen";
import { UpdateAvailableModal } from "./components/UpdateAvailableModal";
import "./App.css";

function App() {
  const [activeScreen, setActiveScreen] = useState<AppScreen>("meeting");
  const [historyMeetingId, setHistoryMeetingId] = useState<string | null>(null);
  const [showOnboarding, setShowOnboarding] = useState(() => !isOnboardingDone());
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<UpdateProgress | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [updateCheckNotice, setUpdateCheckNotice] = useState<string | null>(null);
  const meetingFlow = useMeetingFlow();

  useEffect(() => {
    applyReduceMotionToDocument();
  }, []);

  useEffect(() => {
    let cancelled = false;

    void (async () => {
      const result = await probeAppUpdate();
      if (cancelled) {
        return;
      }
      if (result.status === "available") {
        setPendingUpdate(result.update);
        setUpdateCheckNotice(null);
      } else if (result.status === "error") {
        setUpdateCheckNotice(result.message);
      } else {
        setUpdateCheckNotice(null);
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

  const updateModal =
    pendingUpdate != null ? (
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
    ) : null;

  const updateCheckBanner =
    updateCheckNotice != null ? (
      <div className="status-banner status-banner--error update-check-banner" role="status">
        <p>{updateCheckNotice}</p>
        <button type="button" className="lm-btn" onClick={() => setUpdateCheckNotice(null)}>
          Fermer
        </button>
      </div>
    ) : null;

  if (showOnboarding) {
    return (
      <div className="lm-root">
        {updateCheckBanner}
        <OnboardingIA onComplete={finishOnboarding} onSkip={finishOnboarding} />
        {updateModal}
      </div>
    );
  }

  return (
    <div className="lm-root">
      {updateCheckBanner}
      <LmShell
        active={activeScreen}
        onNavigate={(screen) => {
          if (screen !== "history") {
            setHistoryMeetingId(null);
          }
          setActiveScreen(screen);
        }}
        onOpenMeeting={(meetingId) => {
          setHistoryMeetingId(meetingId);
          setActiveScreen("history");
        }}
        isRecording={meetingFlow.isRecording}
        recordingDurationSecs={meetingFlow.recordingStatus?.durationSecs ?? null}
        onStopRecording={meetingFlow.handleStopRecording}
      >
        {activeScreen === "meeting" ? <MeetingWorkspace flow={meetingFlow} /> : null}
        {activeScreen === "history" ? (
          <MeetingHistory
            initialSelectedId={historyMeetingId}
            onSelectedIdChange={setHistoryMeetingId}
          />
        ) : null}
        {activeScreen === "settings" ? (
          <SettingsScreen
            updateCheckNotice={updateCheckNotice}
            onReplayOnboarding={() => {
              setShowOnboarding(true);
            }}
          />
        ) : null}
      </LmShell>

      {updateModal}
    </div>
  );
}

export default App;
