import { APP_NAME } from "./lib/app";
import { AiProviderSettings } from "./components/AiProviderSettings";
import { MicrophoneRecorder } from "./components/MicrophoneRecorder";
import { Mp3Importer } from "./components/Mp3Importer";
import { StructuredSummaryPanel } from "./components/StructuredSummaryPanel";
import "./App.css";

function App() {
  return (
    <main className="container">
      <h1>{APP_NAME}</h1>
      <Mp3Importer />
      <MicrophoneRecorder />
      <AiProviderSettings />
      <StructuredSummaryPanel />
    </main>
  );
}

export default App;
