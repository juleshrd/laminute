import { APP_NAME } from "./lib/app";
import { AiProviderSettings } from "./components/AiProviderSettings";
import { MicrophoneRecorder } from "./components/MicrophoneRecorder";
import { StructuredSummaryPanel } from "./components/StructuredSummaryPanel";
import "./App.css";

function App() {
  return (
    <main className="container">
      <h1>{APP_NAME}</h1>
      <MicrophoneRecorder />
      <AiProviderSettings />
      <StructuredSummaryPanel />
    </main>
  );
}

export default App;
