import type { UseMeetingFlowResult } from "../hooks/useMeetingFlow";
import { meetingFlowStatusLabel } from "../lib/meetingFlow";
import { RecordingConsentModal } from "./RecordingConsentModal";
import { MeetingIdleStep } from "./meeting-workspace/MeetingIdleStep";
import { MeetingProcessingStep } from "./meeting-workspace/MeetingProcessingStep";
import { MeetingReadyStep } from "./meeting-workspace/MeetingReadyStep";
import { MeetingRecordingStep } from "./meeting-workspace/MeetingRecordingStep";
import { MeetingResultStep } from "./meeting-workspace/MeetingResultStep";

interface MeetingWorkspaceProps {
  flow: UseMeetingFlowResult;
}

export function MeetingWorkspace({ flow }: MeetingWorkspaceProps) {
  return (
    <div className={`meeting-workspace${flow.isRecording ? " meeting-workspace--recording" : ""}`}>
      {!flow.isRecording && flow.flowPhase !== "idle" && flow.flowPhase !== "done" && (
        <div
          className={`status-banner status-banner--${flow.flowPhase}`}
          role="status"
          aria-live="polite"
        >
          {meetingFlowStatusLabel(flow.flowPhase)}
        </div>
      )}

      {flow.loading ? (
        <p>Chargement…</p>
      ) : (
        <>
          {flow.flowPhase === "idle" && (
            <MeetingIdleStep
              canStartRecording={flow.canStartRecording}
              hasDevices={flow.devices.length > 0}
              importing={flow.importing}
              dragOver={flow.dragOver}
              deviceName={flow.selectedDeviceName}
              devices={flow.devices}
              selectedDeviceId={flow.selectedDeviceId}
              onSelectDevice={(deviceId) => void flow.selectDevice(deviceId)}
              onRequestStartRecording={flow.requestStartRecording}
              onPickMp3={flow.handlePickMp3}
              onDragEnter={() => flow.setDragOver(true)}
              onDragLeave={() => flow.setDragOver(false)}
            />
          )}

          {flow.flowPhase === "recording" && (
            <MeetingRecordingStep
              durationSecs={flow.recordingStatus?.durationSecs ?? 0}
              deviceName={flow.selectedDeviceName}
              onStopRecording={flow.handleStopRecording}
            />
          )}

          {flow.showReadyControls && flow.filePath && (
            <MeetingReadyStep
              flowPhase={flow.flowPhase}
              title={flow.title}
              meetingId={flow.meetingId}
              filePath={flow.filePath}
              durationSecs={flow.durationSecs}
              hasApiKey={flow.hasApiKey}
              providerName={flow.providerName}
              selectedProvider={flow.selectedProvider}
              ollamaBaseUrl={flow.ollamaBaseUrl}
              isSummarizeOnly={flow.isSummarizeOnly}
              isBusy={flow.isBusy}
              pastedText={flow.pastedText}
              onTitleChange={flow.setTitle}
              onTitleBlur={() => void flow.persistTitleIfNeeded(flow.title)}
              onPastedTextChange={flow.setPastedText}
              onProcess={flow.runProcessing}
              onSummarizeFromText={flow.runSummarizeFromText}
            />
          )}

          {flow.flowPhase === "processing" && flow.progressMessage && (
            <MeetingProcessingStep progressMessage={flow.progressMessage} />
          )}

          {flow.flowPhase === "done" && (
            <MeetingResultStep
              title={flow.title}
              transcription={flow.transcription}
              summary={flow.summary}
              audioPath={flow.filePath}
              providerName={flow.providerName}
              selectedProvider={flow.selectedProvider}
            />
          )}
        </>
      )}

      {flow.error &&
      !(flow.flowPhase === "idle" && /(périphérique|micro)/i.test(flow.error)) ? (
        <p className="error" role="alert">
          {flow.error}
        </p>
      ) : null}

      {flow.showRecordingConsent && (
        <RecordingConsentModal
          onConfirm={() => void flow.handleStartRecording()}
          onCancel={() => flow.setShowRecordingConsent(false)}
        />
      )}
    </div>
  );
}
