interface MeetingProcessingStepProps {
  progressMessage: string;
}

export function MeetingProcessingStep({ progressMessage }: MeetingProcessingStepProps) {
  return <p className="progress-message">{progressMessage}</p>;
}
