import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSetAtom } from "jotai";
import { consoleMsgAtom } from "@/store/atoms";
import { confirm } from "@tauri-apps/plugin-dialog";

interface OllamaModel {
  name: string;
  size?: number;
  digest?: string;
}

interface ModelConfig {
  llm_model: string;
  embed_model: string;
}

interface PullProgress {
  status: string;
  completed?: number;
  total?: number;
  percent?: number | null; // Allow null from Rust serde
}

interface ModelManagerProps {
  isOpen: boolean;
  onClose: () => void;
  onConfigChange: (config: ModelConfig) => void;
}

// Curated list of popular Ollama models
const POPULAR_LLM_MODELS = [
  {
    name: "gemma4:e4b-mlx", desc: "Google Gemma 4 · 4B · Fast, lightweight, made for Apple Silicon", size: "8.2 GB"
  },
  {
    name: "gemma4:e4b", desc: "Google Gemma 4 · 4B · Suitable for laptops", size: "6.6GB - 9.5 GB"
  },
  { name: "llama3.2:3b", desc: "Meta Llama 3.2 · 3B · Fast & capable", size: "2.0 GB" },
  { name: "llama3.1:8b", desc: "Meta Llama 3.1 · 8B · Great reasoning", size: "4.7 GB" },
  { name: "mistral:7b", desc: "Mistral AI · 7B · Excellent instruction following", size: "4.1 GB" },
  { name: "phi3:mini", desc: "Microsoft Phi-3 · 3.8B · Small but smart", size: "2.2 GB" },
];

const POPULAR_EMBED_MODELS = [

  { name: "nomic-embed-text-v2-moe:latest", desc: "Nomic MoE · 768-dim · Best for RAG", size: "913 MB" },
  { name: "mxbai-embed-large", desc: "MixedBread · 1024-dim · Higher accuracy", size: "670 MB" },
]
function normalizeModelName(name?: string): string {
  if (!name) return "";
  return name.endsWith(":latest") ? name.slice(0, -7) : name;
}

function formatSize(bytes?: number): string {
  if (!bytes) return "Unknown";
  const gb = bytes / 1_073_741_824;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  const mb = bytes / 1_048_576;
  return `${mb.toFixed(0)} MB`;
}

export default function ModelManager({ isOpen, onClose, onConfigChange }: ModelManagerProps) {
  const setConsoleMsg = useSetAtom(consoleMsgAtom);
  const [localModels, setLocalModels] = useState<OllamaModel[]>([]);
  const [config, setConfig] = useState<ModelConfig>({ llm_model: "", embed_model: "" });
  const [activeTab, setActiveTab] = useState<"llm" | "embed">("llm");
  const [pullingModel, setPullingModel] = useState<string | null>(null);
  const [pullProgress, setPullProgress] = useState<PullProgress | null>(null);
  const [ollamaStatus, setOllamaStatus] = useState<"checking" | "online" | "offline">("checking");

  const isInstalled = useCallback(
    (name: string) => {
      if (!name) return false;
      const norm = normalizeModelName(name);
      return localModels.some((m) => m.name === name || normalizeModelName(m.name) === norm);
    },
    [localModels]
  );

  const refreshData = useCallback(async () => {
    try {
      const [models, cfg] = await Promise.all([
        invoke<OllamaModel[]>("list_local_models").catch(() => null),
        invoke<ModelConfig>("get_model_config").catch(() => null),
      ]);

      if (models !== null) {
        setLocalModels(models);
        setOllamaStatus("online");

        if (cfg) {
          const norm = (n: string) => (n.endsWith(":latest") ? n.slice(0, -7) : n);
          const isModelInList = (name: string) => {
            if (!name) return false;
            const n = norm(name);
            return models.some((m) => m.name === name || norm(m.name) === n);
          };

          const validLlm = isModelInList(cfg.llm_model) ? cfg.llm_model : "";
          const validEmbed = isModelInList(cfg.embed_model) ? cfg.embed_model : "";

          const validatedConfig = { llm_model: validLlm, embed_model: validEmbed };
          setConfig(validatedConfig);

          if (validLlm !== cfg.llm_model || validEmbed !== cfg.embed_model) {
            invoke("set_model_config", { llmModel: validLlm, embedModel: validEmbed }).catch(() => {});
            onConfigChange(validatedConfig);
          }
        }
      } else {
        setOllamaStatus("offline");
        if (cfg) {
          setConfig(cfg);
        }
      }
    } catch {
      setOllamaStatus("offline");
    }
  }, [onConfigChange]);

  useEffect(() => {
    if (isOpen) {
      refreshData();
    }
  }, [isOpen, refreshData]);

  // Listen for pull progress / complete / error events.
  // We collect unlisten functions in an array so cleanup works even if the
  // component unmounts before the async setup resolves.
  useEffect(() => {
    let unlistenFns: Array<() => void> = [];
    let isMounted = true;

    const setupListeners = async () => {
      const unlisteners = await Promise.all([
        listen<PullProgress>("model-pull-progress", (e) => {
          if (isMounted) setPullProgress(e.payload);
        }),
        listen<string>("model-pull-complete", async (e) => {
          if (!isMounted) return;
          const downloadedModel = e.payload;
          setPullingModel(null);
          setPullProgress(null);
          setConsoleMsg(`✓ "${downloadedModel}" downloaded successfully!`);

          try {
            const models = await invoke<OllamaModel[]>("list_local_models");
            if (!isMounted) return;
            setLocalModels(models);
            setOllamaStatus("online");

            const isEmbed =
              POPULAR_EMBED_MODELS.some(
                (m) =>
                  m.name === downloadedModel ||
                  normalizeModelName(m.name) === normalizeModelName(downloadedModel)
              ) || downloadedModel.toLowerCase().includes("embed");

            setConfig((prev) => {
              let updated = { ...prev };
              let changed = false;
              if (isEmbed) {
                if (!prev.embed_model) {
                  updated.embed_model = downloadedModel;
                  changed = true;
                }
              } else {
                if (!prev.llm_model) {
                  updated.llm_model = downloadedModel;
                  changed = true;
                }
              }
              if (changed) {
                invoke("set_model_config", {
                  llmModel: updated.llm_model,
                  embedModel: updated.embed_model,
                }).catch(() => {});
                onConfigChange(updated);
              }
              return updated;
            });
          } catch {
            refreshData();
          }
        }),
        listen<string>("model-pull-error", (e) => {
          if (!isMounted) return;
          setPullingModel(null);
          setPullProgress(null);
          setConsoleMsg(`Download failed: ${e.payload}`);
        }),
      ]);

      if (isMounted) {
        unlistenFns = unlisteners;
      } else {
        unlisteners.forEach((fn) => fn());
      }
    };

    setupListeners().catch(console.error);

    return () => {
      isMounted = false;
      unlistenFns.forEach((fn) => fn());
    };
  }, [refreshData, onConfigChange, setConsoleMsg]);

  const handlePull = async (modelName: string) => {
    if (pullingModel) return;
    setPullingModel(modelName);
    setPullProgress({ status: "Connecting to Ollama..." });
    setConsoleMsg(`Starting download of ${modelName}...`);
    try {
      // invoke returns immediately — the backend spawns the download
      // in a background task and emits progress events.
      await invoke("pull_model", { name: modelName });
      setPullProgress({ status: "Downloading..." });
    } catch (e) {
      // This catches early failures (e.g. Ollama offline)
      setConsoleMsg(`Cannot start download: ${e}`);
      setPullingModel(null);
      setPullProgress(null);
    }
  };

  const handleDelete = async (modelName: string) => {
    const confirmed = await confirm(`Delete model "${modelName}"? This cannot be undone.`);
    if (!confirmed) return;
    try {
      await invoke("delete_ollama_model", { name: modelName });
      setConsoleMsg(`Deleted ${modelName}`);

      const normDeleted = normalizeModelName(modelName);
      setConfig((prev) => {
        let updated = { ...prev };
        let changed = false;
        if (prev.llm_model === modelName || normalizeModelName(prev.llm_model) === normDeleted) {
          updated.llm_model = "";
          changed = true;
        }
        if (prev.embed_model === modelName || normalizeModelName(prev.embed_model) === normDeleted) {
          updated.embed_model = "";
          changed = true;
        }
        if (changed) {
          invoke("set_model_config", {
            llmModel: updated.llm_model,
            embedModel: updated.embed_model,
          }).catch(() => {});
          onConfigChange(updated);
        }
        return updated;
      });

      refreshData();
    } catch (e) {
      setConsoleMsg(`Delete failed: ${e}`);
    }
  };

  const handleSetLLM = async (modelName: string) => {
    const newConfig = { ...config, llm_model: modelName };
    try {
      await invoke("set_model_config", { llmModel: newConfig.llm_model, embedModel: newConfig.embed_model });
      setConfig(newConfig);
      onConfigChange(newConfig);
      setConsoleMsg(`LLM set to ${modelName}`);
    } catch (e) {
      setConsoleMsg(`Error: ${e}`);
    }
  };

  const handleSetEmbed = async (modelName: string) => {
    const newConfig = { ...config, embed_model: modelName };
    try {
      await invoke("set_model_config", { llmModel: newConfig.llm_model, embedModel: newConfig.embed_model });
      setConfig(newConfig);
      onConfigChange(newConfig);
      setConsoleMsg(`Embedding model set to ${modelName}`);
    } catch (e) {
      setConsoleMsg(`Error: ${e}`);
    }
  };

  if (!isOpen) return null;

  const catalogModels = activeTab === "llm" ? POPULAR_LLM_MODELS : POPULAR_EMBED_MODELS;
  const activeModel = activeTab === "llm" ? config.llm_model : config.embed_model;
  const setActive = activeTab === "llm" ? handleSetLLM : handleSetEmbed;

  return (
    <div className="model-manager-overlay" onClick={onClose}>
      <div className="model-manager-panel" onClick={e => e.stopPropagation()}>
        {/* Header */}
        <div className="model-manager-header">
          <div className="flex items-center gap-3">
            <div className="model-manager-icon">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="3" />
                <path d="M12 2v3M12 19v3M4.22 4.22l2.12 2.12M17.66 17.66l2.12 2.12M2 12h3M19 12h3M4.22 19.78l2.12-2.12M17.66 6.34l2.12-2.12" />
              </svg>
            </div>
            <div>
              <h2 className="model-manager-title">AI Model Manager</h2>
              <p className="model-manager-subtitle">Select and download Ollama models</p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <div className={`ollama-status ${ollamaStatus}`}>
              <span className="status-dot" />
              <span>Ollama {ollamaStatus === "online" ? "Online" : ollamaStatus === "offline" ? "Offline" : "Checking..."}</span>
            </div>
            <button className="model-manager-close" onClick={onClose}>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
        </div>

        {/* Active Config Banner */}
        <div className="active-config-banner">
          <div className="config-item">
            <span className="config-label">🤖 LLM</span>
            <span className="config-value">
              {isInstalled(config.llm_model) ? config.llm_model : "None selected"}
            </span>
          </div>
          <div className="config-divider" />
          <div className="config-item">
            <span className="config-label">🔢 Embeddings</span>
            <span className="config-value">
              {isInstalled(config.embed_model) ? config.embed_model : "None selected"}
            </span>
          </div>
        </div>

        {/* Pull Progress */}
        {pullingModel && (
          <div className="pull-progress-bar">
            <div className="pull-info">
              <span className="pull-model-name">Downloading {pullingModel}</span>
              <span className="pull-status">{pullProgress?.status}</span>
            </div>
            <div className="pull-track">
              <div
                className="pull-fill"
                style={{ width: `${pullProgress?.percent ?? 0}%`, transition: "width 0.3s ease" }}
              />
            </div>
            {/* Check for both null and undefined */}
            {pullProgress?.percent != null && (
              <span className="pull-percent">{pullProgress.percent.toFixed(0)}%</span>
            )}
          </div>
        )}

        {/* Tabs */}
        <div className="model-tabs">
          <button
            className={`model-tab ${activeTab === "llm" ? "active" : ""}`}
            onClick={() => setActiveTab("llm")}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            </svg>
            Language Models
          </button>
          <button
            className={`model-tab ${activeTab === "embed" ? "active" : ""}`}
            onClick={() => setActiveTab("embed")}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="10" /><path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
            </svg>
            Embedding Models
          </button>
        </div>

        <div className="model-manager-content">
          {/* Installed models section */}
          {localModels.length > 0 && (
            <div className="model-section">
              <h3 className="model-section-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
                  <polyline points="22 4 12 14.01 9 11.01" />
                </svg>
                Installed Locally
              </h3>
              <div className="model-grid">
                {localModels.map(m => {
                  const isMatch = activeModel === m.name || (activeModel !== "" && normalizeModelName(activeModel) === normalizeModelName(m.name));
                  return (
                    <div key={m.name} className={`model-card installed ${isMatch ? "is-active" : ""}`}>
                      <div className="model-card-info">
                        <span className="model-card-name">{m.name}</span>
                        <span className="model-card-size">{formatSize(m.size)}</span>
                      </div>
                      <div className="model-card-actions">
                        {isMatch ? (
                          <span className="active-badge">✓ Active</span>
                        ) : (
                          <button className="model-btn use-btn" onClick={() => setActive(m.name)}>
                            Use
                          </button>
                        )}
                        <button className="model-btn delete-btn" onClick={() => handleDelete(m.name)} title="Delete">
                          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M3 6h18" /><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" /><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
                          </svg>
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Catalog */}
          <div className="model-section">
            <h3 className="model-section-title">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
              </svg>
              {activeTab === "llm" ? "Language Model Catalog" : "Embedding Model Catalog"}
            </h3>
            <div className="catalog-grid">
              {catalogModels.map(m => {
                const installed = isInstalled(m.name);
                const isMatch = activeModel === m.name || (activeModel !== "" && normalizeModelName(activeModel) === normalizeModelName(m.name));
                const isActive = installed && isMatch;
                const isPulling = pullingModel === m.name;
                return (
                  <div key={m.name} className={`catalog-card ${installed ? "installed" : ""} ${isActive ? "is-active" : ""}`}>
                    <div className="catalog-card-header">
                      <span className="catalog-model-name">{m.name}</span>
                      <span className="catalog-size-badge">{m.size}</span>
                    </div>
                    <p className="catalog-model-desc">{m.desc}</p>
                    <div className="catalog-card-footer">
                      {isActive ? (
                        <span className="active-badge">✓ Active</span>
                      ) : installed ? (
                        <button className="model-btn use-btn" onClick={() => setActive(m.name)}>Use</button>
                      ) : isPulling ? (
                        <span className="pulling-badge">Downloading...</span>
                      ) : (
                        <button
                          className="model-btn download-btn"
                          onClick={() => handlePull(m.name)}
                          disabled={!!pullingModel || ollamaStatus !== "online"}
                        >
                          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" />
                          </svg>
                          Download
                        </button>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        {ollamaStatus === "offline" && (
          <div className="ollama-offline-banner">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
              <line x1="12" y1="9" x2="12" y2="13" /><line x1="12" y1="17" x2="12.01" y2="17" />
            </svg>
            Ollama is not available. Please restart the application.
          </div>
        )}
      </div>
    </div>
  );
}
