import { APP_NAME } from "./lib/app";
import { AiProviderSettings } from "./components/AiProviderSettings";
import { MicrophoneRecorder } from "./components/MicrophoneRecorder";
import "./App.css";

function App() {
  return (
    <main className="container">
      <h1>{APP_NAME}</h1>
      <MicrophoneRecorder />
      <AiProviderSettings />
    </main>
  );
}

export default App;
