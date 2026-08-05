import { APP_NAME } from "./lib/app";
import { AiProviderSettings } from "./components/AiProviderSettings";
import { MeetingWorkspace } from "./components/MeetingWorkspace";
import "./App.css";

function App() {
  return (
    <main className="container">
      <h1>{APP_NAME}</h1>
      <MeetingWorkspace />
      <details className="ai-settings-collapsible">
        <summary>Réglages IA</summary>
        <AiProviderSettings />
      </details>
    </main>
  );
}

export default App;
