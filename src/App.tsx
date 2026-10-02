import { useState, useEffect, useCallback, useMemo } from "react";
import "./App.css";
import UploadButton from "@/components/custom/UploadButton";
import BookShelf from "@/components/custom/BookShelf";
import ModelManager from "@/components/custom/ModelManager";
import { useAtom } from "jotai";
import { consoleMsgAtom } from "@/store/atoms";
import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Loader2Icon, Sparkles, BookOpen, GraduationCap, ArrowRight, DownloadIcon, UploadIcon } from "lucide-react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { Modal, ModalHeader, ModalBody, ModalTitle } from "@/components/custom/Modal";
import { listen } from "@tauri-apps/api/event";
import { save, open } from "@tauri-apps/plugin-dialog";

interface Chunk {
  id: number;
  book_id: string;
  chunk_index: number;
  content: string;
}

interface Book {
  id: string;
  title: string;
}

interface ModelConfig {
  llm_model: string;
  embed_model: string;
}

interface ConceptItem {
  id: string;
  title: string;
  description: string;
  excerpts: string[];
};
interface AnalysisProgress {
  status: string;
  processed?: number;
  total?: number;
  percent?: number | null;
}

function App() {
  const [consoleMsg, setConsoleMsg] = useAtom(consoleMsgAtom);
  const [books, setBooks] = useState<Book[]>([]);
  const [selectedBookId, setSelectedBookId] = useState<string>("");
  const [chunks, setChunks] = useState<Chunk[]>([]);
  const [searchResults, setSearchResults] = useState<string[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [aiResponse, setAiResponse] = useState("");
  const [isGenerating, setIsGenerating] = useState(false);
  const [mode, setMode] = useState<"search" | "learn">("search");
  const [ollamaStatus, setOllamaStatus] = useState<"checking" | "online" | "offline">("checking");
  // Learn mode state
  const [learnTab, setLearnTab] = useState<"concepts" | "excerpts" | "lesson">("concepts");
  const [lesson, setLesson] = useState("");

  const [learningPath, setLearningPath] = useState("");
  const [relatedExcerpts, setRelatedExcerpts] = useState<string[]>([]);
  const [selectedConcept, setSelectedConcept] = useState("");
  const [customConcept, setCustomConcept] = useState("");
  const [conceptDescModalOpen, setConceptDescModalOpen] = useState(false);
  const [selectedConceptDescription, setConceptDescription] = useState("");

  const [isUploading, setIsUploading] = useState(false);
  const [newBookId, setNewBookId] = useState<string | null>(null);
  const [modelManagerOpen, setModelManagerOpen] = useState(false);
  const [modelConfig, setModelConfig] = useState<ModelConfig>({ llm_model: "gemma2:2b", embed_model: "nomic-embed-text" });
  const [showChunksPanel, setShowChunksPanel] = useState(false);
  const [analysisProgress, setAnalysisProgress] = useState<AnalysisProgress | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;

    const setup = async () => {
      unlisten = await listen<AnalysisProgress>(
        "analysis-progress",
        (event) => {
          if (!mounted) return;

          setAnalysisProgress(event.payload);
        }
      );
    };

    setup();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  const fetchBooks = useCallback(async () => {
    try {
      const data = await invoke<Book[]>("get_books");
      setBooks(data);
    } catch (error) {
      console.error("Error fetching books:", error);
    }
  }, []);


  const fetchModelConfig = useCallback(async () => {
    try {
      const cfg = await invoke<ModelConfig>("get_model_config");
      setModelConfig(cfg);
    } catch { }
  }, []);

  interface OllamaModel {
    name: string;
    size?: number;
    digest?: string;
  }

  const checkOllamaStatus = async () => {
    try {
      await invoke<OllamaModel[]>("list_local_models");
      setOllamaStatus("online");
    } catch {
      setOllamaStatus("offline");
    }
  };

  const handleSaveDatabase = async () => {
    try {
      const path = await save({
        title: "Save Library Backup",
        defaultPath: "library-backup.zip",
        filters: [
          {
            name: "Library Backup",
            extensions: ["zip"],
          },
        ],
      });

      if (!path) {
        return;
      }

      await invoke("save_backup", {
        path,
      });

      setConsoleMsg("Library backup saved successfully!");
    } catch (error) {
      console.error("Save backup error:", error);

      setConsoleMsg(
        `Error saving backup: ${String(error)}`
      );
    }
  };
  const handleLoadDatabase = async () => {
    try {
      const path = await open({
        title: "Load Library Backup",
        multiple: false,
        directory: false,
        filters: [
          {
            name: "Library Backup",
            extensions: ["zip"],
          },
        ],
      });

      if (!path || Array.isArray(path)) {
        return;
      }

      await invoke("load_backup", {
        path,
      });

      setConsoleMsg("Library backup restored successfully!");
    } catch (error) {
      console.error("Load backup error:", error);

      setConsoleMsg(
        `Error loading backup: ${String(error)}`
      );
    }
  };
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await listen("backup-restored", async () => {
        console.log("Backup restored - refreshing application data");
        await fetchBooks();
      });
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  useEffect(() => {
    checkOllamaStatus();
    const interval = setInterval(checkOllamaStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    fetchBooks();
    fetchModelConfig();
  }, [fetchBooks, fetchModelConfig]);

  useEffect(() => {
    const loadStoredData = async () => {
      if (!selectedBookId) {
        setLearningPath("");
        setRelatedExcerpts([]);
        setSelectedConcept("");
        setLesson("");
        return;
      }
      try {
        const storedPath = await invoke<string | null>("get_stored_learning_path", { bookId: selectedBookId });
        if (storedPath) {
          setLearningPath(storedPath);
        } else {
          setLearningPath("");
        }
        setRelatedExcerpts([]);
        setSelectedConcept("");
        setLesson("");
        setLearnTab("concepts");
      } catch (error) {
        console.error("Error loading stored data:", error);
      }
    };
    loadStoredData();
  }, [selectedBookId]);

  useEffect(() => {
    if (!selectedBookId || !selectedConcept.trim()) {
      setLesson("");
      return;
    }

    let cancelled = false;

    const loadLesson = async () => {
      setLesson("");

      try {
        const cachedLesson = await invoke<string | null>("get_lesson", {
          concept: selectedConcept,
          bookId: selectedBookId,
        });

        if (!cancelled) {
          setLesson(cachedLesson ?? "");
        }
      } catch (error) {
        if (!cancelled) {
          console.error("Error loading lesson:", error);
          setLesson("");
        }
      }
    };

    loadLesson();

    return () => {
      cancelled = true;
    };
  }, [selectedBookId, selectedConcept]);
  const parsedConcepts = useMemo<ConceptItem[]>(() => {
    if (!learningPath) return [];

    try {
      const cleaned = learningPath
        .trim()
        .replace(/^```(?: markdown) ?\s */i, "")
        .replace(/\s*```$/i, "")
        .trim();

      // Find every ## Concept heading and everything until the next ## heading.
      const sections = cleaned
        .split(/^##\s+/gm)
        .map((section) => section.trim())
        .filter(Boolean);

      return sections
        .map((section) => {
          const lines = section.split("\n");

          // First line is the concept title
          const title = lines[0]
            .replace(/^Concept\s+\d+\s*:\s*/i, "")
            .trim();

          if (!title || title.length < 3) {
            return null;
          }

          // Find the "### Excerpts" heading
          const excerptsIndex = lines.findIndex((line) =>
            /^###\s+Excerpts\s*$/i.test(line.trim())
          );

          let description = "";
          let excerpts: string[] = [];

          if (excerptsIndex !== -1) {
            // Everything between title and ### Excerpts is the description
            description = lines
              .slice(1, excerptsIndex)
              .join("\n")
              .trim();

            // Everything after ### Excerpts is the excerpts section
            const excerptLines = lines.slice(excerptsIndex + 1);

            excerpts = excerptLines
              .map((line) => line.trim())
              .filter((line) => /^[-*•]\s+/.test(line))
              .map((line) =>
                line
                  .replace(/^[-*•]\s+/, "")
                  .trim()
                  .replace(/^["“”]+|["“”]+$/g, "")
                  .trim()
              )
              .filter(Boolean);
          } else {
            // If there is no Excerpts heading, treat everything after
            // the title as the description.
            description = lines
              .slice(1)
              .join("\n")
              .trim();
          }

          return {
            id: title
              .toLowerCase()
              .replace(/[^a-z0-9]+/g, "-")
              .replace(/^-|-$/g, ""),

            title,

            description:
              description ||
              "Master this key core concept from the selected book.",

            excerpts,
          };
        })
        .filter((item): item is ConceptItem => item !== null);
    } catch (error) {
      console.error("Failed to parse learning path Markdown:", error);
      console.error("Raw learning path:", learningPath);
      return [];
    }
  }, [learningPath]);

  const handleDeleteBook = async () => {
    if (!selectedBookId) return;
    const confirmDelete = await confirm("Remove this book from your shelf? This cannot be undone.");
    if (!confirmDelete) return;
    try {
      await invoke("delete_book", { bookId: selectedBookId });
      setConsoleMsg("Book removed from shelf.");
      setSelectedBookId("");
      setChunks([]);
      setSearchResults([]);
      setLearningPath("");
      setRelatedExcerpts([]);
      setSelectedConcept("");
      fetchBooks();
    } catch (error) {
      setConsoleMsg(`Error removing book: ${error}`);
    }
  };

  const handleGetChunks = async () => {
    try {
      const data = await invoke<Chunk[]>("get_chunks", { bookId: selectedBookId || null });
      setChunks(data);
      setShowChunksPanel(true);
      setConsoleMsg(`Fetched ${data.length} chunks${selectedBookId ? " for selected book" : ""}`);
    } catch (error) {
      setConsoleMsg(`Error fetching chunks: ${error}`);
    }
  };

  const handleSearch = async () => {
    try {
      if (!searchQuery) {
        setConsoleMsg("Please enter a search query");
        return;
      }
      const results = await invoke<string[]>("search_context", {
        query: searchQuery,
        bookId: selectedBookId || null,
      });
      setSearchResults(results);
      setConsoleMsg(`Found ${results.length} relevant passages${selectedBookId ? " (filtered by book)" : ""}`);
    } catch (error) {
      setConsoleMsg(`Search error: ${error}`);
    }
  };

  const handleGenerate = async () => {
    if (searchResults.length === 0) {
      setConsoleMsg("Search first to provide context for the AI");
      return;
    }
    setIsGenerating(true);
    setConsoleMsg("Generating response...");
    const response = await invoke<string>("generate_response", {
      query: searchQuery,
      bookId: selectedBookId || null,
    });
    setAiResponse(response);
    setIsGenerating(false);
    setAnalysisProgress(null);
    setConsoleMsg("AI generation complete!");
  };

  const handleGenerateLearningPath = async () => {
    if (!selectedBookId) {
      setConsoleMsg("Please select a book first");
      return;
    }

    setIsGenerating(true);
    setConsoleMsg("Analysing book concepts...");
    try {
      const path = await invoke<string>("generate_learning_path", { bookId: selectedBookId });
      setLearningPath(path);
      setLearnTab("concepts");
      setConsoleMsg("Concepts ready!");
    } catch (error) {
      setConsoleMsg(`Error: ${error}`);

    } finally {
      setAnalysisProgress(null);
      setIsGenerating(false);
    }
  };

  const handleGenerateExplanation = async () => {
    if (!selectedBookId || !selectedConcept) {
      setConsoleMsg("Please select a book and concept first.");
      return;
    }

    setLearnTab("lesson");
    setIsGenerating(true);
    setConsoleMsg(`Generating lesson for "${selectedConcept}"...`);

    try {
      const explanation = await invoke<string>("generate_lesson", {
        concept: selectedConcept,
        bookId: selectedBookId,
      });

      setLesson(explanation);
      setAiResponse(explanation);
      setConsoleMsg("Lesson generated successfully!");
    } catch (error) {
      console.error("Error generating lesson:", error);
      setConsoleMsg(`Error generating lesson: ${error}`);
    } finally {
      setIsGenerating(false);
    }
  };
  const handleGetRelatedExcerpts = async (concept: string) => {
    if (!selectedBookId || !concept.trim()) return;

    setSelectedConcept(concept);
    setLesson("");
    setLearnTab("excerpts");
    setIsGenerating(true);
    setRelatedExcerpts([]);
    setConsoleMsg(`Finding passages related to "${concept}"...`);

    try {
      const results = await invoke<string[]>("search_context", {
        query: concept,
        bookId: selectedBookId,
      });

      setRelatedExcerpts(results);

      setConsoleMsg(
        results.length > 0
          ? `Found ${results.length} related passage${results.length === 1 ? "" : "s"
          } from the book.`
          : "No related passages found in the book."
      );
    } catch (error) {
      setConsoleMsg(`Error finding related passages: ${error}`);
    } finally {
      setIsGenerating(false);
      setAnalysisProgress(null);
    }
  };

  const handleUploadStart = () => {
    setIsUploading(true);
    setNewBookId(null);
  };

  const handleUploadDone = async () => {
    setIsUploading(false);
    try {
      const data = await invoke<Book[]>("get_books");
      setBooks(data);
      if (data.length > 0) {
        const lastBook = data[data.length - 1];
        setNewBookId(lastBook.id);
        setTimeout(() => setNewBookId(null), 3000);
      }
    } catch { }
  };

  const selectedBook = books.find(b => b.id === selectedBookId);

  return (
    <div className="app-root">
      {/* Sidebar */}
      <aside className="app-sidebar">
        <div className="sidebar-logo">
          <div className="logo-icon">
            <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20" />
            </svg>
          </div>
          <span className="logo-text">SkillsStore</span>
        </div>

        {/* Mode switcher */}
        <nav className="sidebar-nav">
          <button
            className={`nav-item ${mode === "search" ? "active" : ""}`}
            onClick={() => setMode("search")}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
            </svg>
            Search & Ask
          </button>
          <button
            className={`nav-item ${mode === "learn" ? "active" : ""}`}
            onClick={() => setMode("learn")}
          >
            <GraduationCap className="w-4 h-4" />
            Learn Mode
          </button>
        </nav>

        <div className="sidebar-divider" />

        {/* Model config summary */}
        <div className="model-config-summary" onClick={() => setModelManagerOpen(true)}>
          <div className="model-config-row">
            <span className="model-config-label">LLM</span>
            <span className="model-config-val">{modelConfig.llm_model}</span>
          </div>
          <div className="model-config-row">
            <span className="model-config-label">Embed</span>
            <span className="model-config-val">{modelConfig.embed_model}</span>
          </div>
          <div className="model-config-hint">
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="10" /><line x1="12" y1="8" x2="12" y2="12" /><line x1="12" y1="16" x2="12.01" y2="16" />
            </svg>
            Click to manage models
          </div>
        </div>

        <div className="sidebar-spacer" />

        <div className="flex flex-col gap-2">
          <div className={`ollama-status ${ollamaStatus}`}>
            <span className="status-dot" />
            <span>Ollama {ollamaStatus === "online" ? "Online" : ollamaStatus === "offline" ? "Offline" : "Checking..."}</span>
          </div>

          <button className="sidebar-bottom-btn" onClick={() => setModelManagerOpen(true)}>
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="3" />
              <path d="M12 2v3M12 19v3M4.22 4.22l2.12 2.12M17.66 17.66l2.12 2.12M2 12h3M19 12h3M4.22 19.78l2.12-2.12M17.66 6.34l2.12-2.12" />
            </svg>
            AI Models
          </button>
        </div>
      </aside>

      {/* Main area */}
      <main className="app-main">
        {/* Top bar */}
        <header className="app-topbar">
          <div className="topbar-left">
            <h1 className="topbar-title">
              {mode === "search" ? "Search & Ask" : "Learn Mode"}
              {selectedBook && <span className="topbar-book"> · {selectedBook.title}</span>}
            </h1>
          </div>
          <div className="topbar-right">
            <div className="status-chip-wrapper">
              <span className="status-chip" title={consoleMsg}>
                {consoleMsg || "Ready"}
              </span>
            </div>
            <button
              className="debug-btn"
              onClick={handleSaveDatabase}
              title="Save database"
            >
              <UploadIcon className="w-4 h-4" />
              Save database
            </button>
            <button
              className="debug-btn"
              onClick={handleLoadDatabase}
              title="Load database"
            >
              <DownloadIcon className="w-4 h-4" />
              Load database
            </button>
            <button
              className="debug-btn"
              onClick={handleGetChunks}
              title="Inspect database chunks"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
                <line x1="8" y1="11" x2="14" y2="11" /><line x1="11" y1="8" x2="11" y2="14" />
              </svg>
              Debug Chunks
            </button>
          </div>
        </header>
        {/* Bookshelf area */}
        <div className="bookshelf-section">
          <div className="bookshelf-section-top">
            <BookShelf
              books={books}
              selectedBookId={selectedBookId}
              onSelectBook={setSelectedBookId}
              onDeleteBook={handleDeleteBook}
              isUploading={isUploading}
              newBookId={newBookId}
            />
            <UploadButton onUploadStart={handleUploadStart} onUpload={handleUploadDone} />
          </div>
        </div>

        {/* Content area */}
        <div className="content-area">
          {mode === "search" ? (
            <div className="search-mode animate-fadein">
              <div className="search-bar-row">
                <div className="search-input-wrap">
                  <svg className="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
                  </svg>
                  <input
                    type="text"
                    placeholder={selectedBook ? `Search in "${selectedBook.title}"...` : "Search across all books..."}
                    className="search-input"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                  />
                </div>
                <Button onClick={handleSearch} className="search-btn">Search</Button>
                <Button
                  variant="secondary"
                  onClick={handleGenerate}
                  disabled={isGenerating || searchResults.length === 0}
                  className="ask-btn"
                >
                  {isGenerating ? (
                    <><Loader2Icon className="w-4 h-4 animate-spin mr-1" /> Thinking...</>
                  ) : "Ask AI"}
                </Button>
              </div>

              <div className="search-results-grid">
                <div className="ai-response-panel">
                  <div className="panel-header">
                    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M12 2a10 10 0 1 0 10 10" />
                      <path d="M12 6v6l4 2" />
                    </svg>
                    AI Perspective
                  </div>
                  {aiResponse ? (
                    <div className="ai-response-content prose prose-sm dark:prose-invert max-w-none">
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>{aiResponse}</ReactMarkdown>
                    </div>
                  ) : (
                    <div className="panel-empty">
                      {isGenerating ? (
                        <><Loader2Icon className="w-8 h-8 animate-spin text-primary mb-3" /><p>Synthesizing answer from your books...</p></>
                      ) : (
                        <>
                          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="text-muted-foreground mb-3">
                            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
                          </svg>
                          <p>Search for something, then click <strong>Ask AI</strong> to get an answer from your books.</p>
                        </>
                      )}
                    </div>
                  )}
                </div>

                <div className="context-panel">
                  <div className="panel-header">
                    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                      <polyline points="14 2 14 8 20 8" />
                    </svg>
                    Source Passages
                    {searchResults.length > 0 && <span className="panel-count">{searchResults.length}</span>}
                  </div>
                  {searchResults.length > 0 ? (
                    <div className="context-list">
                      {searchResults.map((res, i) => (
                        <div key={i} className="context-item">
                          <span className="context-num">{i + 1}</span>
                          <p className="context-text">"{res}"</p>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="panel-empty">
                      <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="text-muted-foreground mb-3">
                        <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
                      </svg>
                      <p>Search results will appear here.</p>
                    </div>
                  )}
                </div>

                {showChunksPanel && chunks.length > 0 && (
                  <div className="chunks-panel">
                    <div className="panel-header">
                      <div className="flex items-center gap-2">
                        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                          <rect x="3" y="3" width="18" height="18" rx="2" /><path d="M3 9h18M9 21V9" />
                        </svg>
                        Knowledge Grains
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="panel-count">{chunks.length}</span>
                        <button className="panel-close" onClick={() => setShowChunksPanel(false)}>×</button>
                      </div>
                    </div>
                    <div className="chunks-list">
                      {chunks.map((chunk) => (
                        <div key={chunk.id} className="chunk-item">
                          <div className="chunk-meta">
                            <span>Segment {chunk.chunk_index}</span>
                            <span className="chunk-id">{chunk.book_id.slice(0, 8)}</span>
                          </div>
                          <p className="chunk-content">{chunk.content}</p>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>
          ) : (
            /* LEARN MODE */
            <div className="learn-mode animate-fadein">
              {!learningPath && !isGenerating && (
                <div className="learn-start">
                  <div className="learn-icon">
                    <GraduationCap className="w-8 h-8" />
                  </div>
                  <h2>Start Learning</h2>
                  <p>Select a book from your shelf, then let AI identify the key concepts and show the relevant passages from the book.</p>
                  <Button
                    onClick={handleGenerateLearningPath}
                    size="lg"
                    disabled={!selectedBookId}
                    className="learn-start-btn"
                  >
                    {selectedBookId ? "Find Key Concepts" : "Select a book first"}
                  </Button>
                  {!selectedBookId && (
                    <p className="learn-hint">↑ Click a book on the shelf above to get started</p>
                  )}
                </div>
              )}

              {isGenerating && !learningPath && (
                <div className="learn-loading">
                  <Loader2Icon className="w-12 h-12 text-primary animate-spin" />

                  <p>
                    {analysisProgress?.status ||
                      "Analysing your book's key concepts..."}
                  </p>

                  {analysisProgress?.percent != null && (
                    <div className="pull-progress-bar rounded-xl w-[80%]">
                      <div className="pull-info flex gap-10 ">
                        <span className="pull-status">
                          Progress
                        </span>
                        <span className="pull-percent">
                          {analysisProgress.percent.toFixed(0)}%
                        </span>
                      </div>

                      <div className="pull-track">
                        <div
                          className="pull-fill"
                          style={{
                            width: `${analysisProgress.percent}%`,
                            transition: "width 0.3s ease",
                          }}
                        />
                      </div>

                      {analysisProgress.processed != null &&
                        analysisProgress.total != null && (
                          <div className="pull-info">
                            <span className="pull-status">
                              {analysisProgress.processed.toLocaleString()} /{" "}
                              {analysisProgress.total.toLocaleString()} tokens
                            </span>
                          </div>
                        )}
                    </div>
                  )}
                </div>
              )}

              {learningPath && (
                <div className="learn-container">
                  {/* Learn Mode Navigation Header */}
                  <div className="learn-nav-header">
                    <div className="learn-tabs">
                      <button
                        className={`learn-tab-btn ${learnTab === "concepts" ? "active" : ""
                          }`}
                        onClick={() => setLearnTab("concepts")}
                      >
                        <Sparkles className="w-4 h-4" />
                        Key Concepts

                        {parsedConcepts.length > 0 && (
                          <span className="learn-tab-badge">
                            {parsedConcepts.length}
                          </span>
                        )}
                      </button>

                      <button
                        className={`learn-tab-btn ${learnTab === "excerpts" ? "active" : ""
                          }`}
                        onClick={() => setLearnTab("excerpts")}
                      >
                        <BookOpen className="w-4 h-4" />
                        Excerpts

                        {selectedConcept && (
                          <span className="learn-tab-dot" />
                        )}
                      </button>

                      <button
                        className={`learn-tab-btn ${learnTab === "lesson" ? "active" : ""
                          }`}
                        onClick={() => setLearnTab("lesson")}
                      >
                        <GraduationCap className="w-4 h-4" />
                        Lesson

                        {lesson && (
                          <span className="learn-tab-dot" />
                        )}
                      </button>
                    </div>

                    {/* <div className="learn-header-actions">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={handleGenerateLearningPath}
                        disabled={isGenerating}
                        className="text-muted-foreground hover:text-foreground"
                      >
                        <RefreshCw className={`w-3.5 h-3.5 mr-1.5 ${isGenerating ? "animate-spin" : ""}`} />
                        Regenerate
                      </Button>
                    </div> */}
                  </div>

                  {/* TAB 1: CONCEPTS */}
                  {learnTab === "concepts" && (
                    <div className="concepts-tab-content animate-fadein">
                      {/* Concept search & custom prompt box */}
                      <div className="custom-concept-bar">
                        <input
                          type="text"
                          placeholder="Or type a specific concept/topic to find in the book..."
                          className="custom-concept-input"
                          value={customConcept}
                          onChange={(e) => setCustomConcept(e.target.value)}
                          onKeyDown={(e) => e.key === "Enter" && handleGetRelatedExcerpts(customConcept)}
                        />
                        <Button
                          onClick={() => handleGetRelatedExcerpts(customConcept)}
                          disabled={isGenerating || !customConcept.trim()}
                          size="sm"
                        >
                          Find Related Excerpts
                        </Button>
                      </div>

                      {parsedConcepts.length > 0 ? (
                        <div className="concepts-grid">
                          {parsedConcepts.map((item, idx) => (
                            <div
                              key={item.id || idx}
                              className={`concept-card ${selectedConcept === item.title ? "active" : ""}`}
                              onClick={() => {
                                setSelectedConcept(item.title);
                                setRelatedExcerpts(item.excerpts || []);
                                setLearnTab("excerpts");
                              }}
                            >
                              <div className="concept-card-header">
                                <span className="concept-number">{idx + 1}</span>
                                <h3 className="concept-card-title">{item.title}</h3>
                              </div>
                              {item.description && (
                                <p className="concept-card-desc">{item.description}</p>
                              )}
                              <div className="concept-card-footer">
                                <span className="concept-tag concept-tag-clickable" onClick={(e) => {
                                  e.stopPropagation();
                                  setSelectedConcept(item.title);
                                  setConceptDescription(item.description || "");
                                  setConceptDescModalOpen(true);
                                }}>Concept</span>
                                <span className="concept-action">
                                  View Excerpts <ArrowRight className="w-3.5 h-3.5 ml-1 inline" />
                                </span>
                              </div>
                            </div>
                          ))}
                        </div>
                      ) : (
                        /* Fallback Markdown View if concept parsing finds no clear items */
                        <div className="concepts-fallback-panel">
                          <div className="prose prose-sm dark:prose-invert max-w-none">
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>{learningPath}</ReactMarkdown>
                          </div>
                        </div>
                      )}
                    </div>
                  )}

                  {/* TAB 2: RELATED EXCERPTS */}
                  {learnTab === "excerpts" && (
                    <div className="lesson-tab-content animate-fadein">
                      <div className="lesson-workspace">
                        <div className="lesson-workspace-header">
                          <div className="lesson-topic-title">
                            {selectedConcept ? (
                              <>
                                <span className="topic-subtitle">RELATED EXCERPTS</span>
                                <h2>{selectedConcept}</h2>
                              </>
                            ) : (
                              <h2>Related Book Excerpts</h2>
                            )}
                          </div>

                          <Button
                            onClick={handleGenerateExplanation}
                            disabled={
                              isGenerating ||
                              !selectedBookId ||
                              !selectedConcept.trim()
                            }
                          >
                            {isGenerating ? (
                              <>
                                <Loader2Icon className="w-4 h-4 mr-2 animate-spin" />
                                Generating...
                              </>
                            ) : (
                              <>
                                <GraduationCap className="w-4 h-4 mr-2" />
                                Generate Lesson
                              </>
                            )}
                          </Button>
                        </div>

                        {relatedExcerpts.length > 0 ? (
                          <div className="context-list">
                            {relatedExcerpts.map((excerpt, i) => (
                              <div key={i} className="context-item">
                                <span className="context-num">{i + 1}</span>
                                <p className="context-text">"{excerpt}"</p>
                              </div>
                            ))}
                          </div>
                        ) : (
                          <div className="panel-empty lesson-empty-state">
                            {isGenerating ? (
                              <>
                                <Loader2Icon className="w-10 h-10 animate-spin text-primary mb-3" />
                                <h3>Finding Related Excerpts...</h3>
                                <p>Searching the selected book for passages related to this concept.</p>
                              </>
                            ) : (
                              <>
                                <BookOpen className="w-10 h-10 text-muted-foreground mb-3 opacity-60" />
                                <h3>No Related Excerpts</h3>
                                <p>Select a concept from the <strong>Key Concepts</strong> tab or type a topic to find matching passages in the book.</p>
                                <Button
                                  variant="secondary"
                                  className="mt-4"
                                  onClick={() => setLearnTab("concepts")}
                                >
                                  Browse Key Concepts
                                </Button>
                              </>
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                  )}
                  {/* TAB 3: LESSON */}
                  {learnTab === "lesson" && (
                    <div className="lesson-tab-content animate-fadein">
                      <div className="lesson-workspace">
                        <div className="lesson-workspace-header">
                          <div className="lesson-topic-title">
                            <span className="topic-subtitle">AI LESSON</span>

                            <h2>
                              {selectedConcept || "Select a concept"}
                            </h2>
                          </div>

                          {/* <Button
                            onClick={handleGenerateExplanation}
                            disabled={
                              isGenerating ||
                              !selectedBookId ||
                              !selectedConcept.trim()
                            }
                            variant="secondary"
                          >
                            {isGenerating ? (
                              <>
                                <Loader2Icon className="w-4 h-4 mr-2 animate-spin" />
                                Generating...
                              </>
                            ) : (
                              <>
                                <RefreshCw className="w-4 h-4 mr-2" />
                                Regenerate
                              </>
                            )}
                          </Button> */}
                        </div>

                        {!selectedConcept ? (
                          <div className="panel-empty lesson-empty-state">
                            <GraduationCap className="w-10 h-10 text-muted-foreground mb-3 opacity-60" />

                            <h3>Select a Concept</h3>

                            <p>
                              Choose a concept from the{" "}
                              <strong>Key Concepts</strong> tab to generate a lesson.
                            </p>

                            <Button
                              variant="secondary"
                              className="mt-4"
                              onClick={() => setLearnTab("concepts")}
                            >
                              Browse Key Concepts
                            </Button>
                          </div>
                        ) : isGenerating && !lesson ? (
                          <div className="panel-empty lesson-empty-state">
                            <Loader2Icon className="w-10 h-10 animate-spin text-primary mb-3" />

                            <h3>Generating Lesson...</h3>

                            <p>
                              Building a lesson about{" "}
                              <strong>{selectedConcept}</strong> from your book.
                            </p>
                          </div>
                        ) : lesson ? (
                          <div className="ai-response-content prose prose-sm dark:prose-invert max-w-none">
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>
                              {lesson}
                            </ReactMarkdown>
                          </div>
                        ) : (
                          <div className="panel-empty lesson-empty-state">
                            <GraduationCap className="w-10 h-10 text-muted-foreground mb-3 opacity-60" />

                            <h3>Ready to Learn</h3>

                            <p>
                              Generate a lesson about{" "}
                              <strong>{selectedConcept}</strong> using the selected book.
                            </p>

                            <Button
                              className="mt-4"
                              onClick={handleGenerateExplanation}
                              disabled={!selectedBookId}
                            >
                              <Sparkles className="w-4 h-4 mr-2" />
                              Generate Lesson
                            </Button>
                          </div>
                        )}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
        {/* Modal for viewing concept description */}
        <Modal
          isOpen={conceptDescModalOpen}
          onClose={() => setConceptDescModalOpen(false)}
        >
          <ModalHeader onClose={() => setConceptDescModalOpen(false)}>
            <ModalTitle>{selectedConcept}</ModalTitle>
          </ModalHeader>
          <ModalBody>
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{selectedConceptDescription}</ReactMarkdown>
          </ModalBody>
        </Modal>
      </main >

      {/* Model Manager Overlay */}
      < ModelManager
        isOpen={modelManagerOpen}
        onClose={() => setModelManagerOpen(false)
        }
        onConfigChange={(cfg) => setModelConfig(cfg)}
      />
    </div >
  );
}

export default App;