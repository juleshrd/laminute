import { useState } from "react";

import { APP_NAME } from "./lib/app";
import { AiProviderSettings } from "./components/AiProviderSettings";
import { MeetingHistory } from "./components/MeetingHistory";
import { MeetingWorkspace } from "./components/MeetingWorkspace";
import "./App.css";

type AppTab = "new" | "history";

function App() {
  const [activeTab, setActiveTab] = useState<AppTab>("new");

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
    </main>
  );
}

export default App;
