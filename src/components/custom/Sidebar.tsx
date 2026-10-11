import React from "react";
import { GraduationCap } from "lucide-react";
import { AppMode, ModelConfig, OllamaStatus } from "@/types";

export interface SidebarProps {
  mode: AppMode;
  onModeChange: (mode: AppMode) => void;
  modelConfig: ModelConfig;
  ollamaStatus: OllamaStatus;
  onOpenModelManager: () => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  mode,
  onModeChange,
  modelConfig,
  ollamaStatus,
  onOpenModelManager,
}) => {
  return (
    <aside className="app-sidebar">
      <div className="sidebar-logo">
        <div className="logo-icon">
          <svg
            width="22"
            height="22"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20" />
          </svg>
        </div>
        <span className="logo-text">SkillsStore</span>
      </div>

      {/* Mode switcher */}
      <nav className="sidebar-nav">
        <button
          className={`nav-item ${mode === "search" ? "active" : ""}`}
          onClick={() => onModeChange("search")}
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
          Search & Ask
        </button>
        <button
          className={`nav-item ${mode === "learn" ? "active" : ""}`}
          onClick={() => onModeChange("learn")}
        >
          <GraduationCap className="w-4 h-4" />
          Learn Mode
        </button>
      </nav>

      <div className="sidebar-divider" />

      {/* Model config summary */}
      <div
        className="model-config-summary"
        onClick={onOpenModelManager}
      >
        <div className="model-config-row">
          <span className="model-config-label">LLM</span>
          <span className="model-config-val">{modelConfig.llm_model}</span>
        </div>
        <div className="model-config-row">
          <span className="model-config-label">Embed</span>
          <span className="model-config-val">{modelConfig.embed_model}</span>
        </div>
        <div className="model-config-hint">
          <svg
            width="11"
            height="11"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          Click to manage models
        </div>
      </div>

      <div className="sidebar-spacer" />

      <div className="flex flex-col gap-2">
        <div className={`ollama-status ${ollamaStatus}`}>
          <span className="status-dot" />
          <span>
            Ollama{" "}
            {ollamaStatus === "online"
              ? "Online"
              : ollamaStatus === "offline"
              ? "Offline"
              : "Checking..."}
          </span>
        </div>

        <button
          className="sidebar-bottom-btn"
          onClick={onOpenModelManager}
        >
          <svg
            width="15"
            height="15"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="12" cy="12" r="3" />
            <path d="M12 2v3M12 19v3M4.22 4.22l2.12 2.12M17.66 17.66l2.12 2.12M2 12h3M19 12h3M4.22 19.78l2.12-2.12M17.66 6.34l2.12-2.12" />
          </svg>
          AI Models
        </button>
      </div>
    </aside>
  );
};

export default Sidebar;
