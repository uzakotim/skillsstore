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
import { Loader2Icon, Sparkles, BookOpen, GraduationCap, ArrowRight, RefreshCw, Copy, Check } from "lucide-react";
import { confirm } from "@tauri-apps/plugin-dialog";

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

  // Learn mode state
  const [learnTab, setLearnTab] = useState<"concepts" | "lesson">("concepts");
  const [learningPath, setLearningPath] = useState("");
  const [lesson, setLesson] = useState("");
  const [selectedConcept, setSelectedConcept] = useState("");
  const [customConcept, setCustomConcept] = useState("");
  const [copiedLesson, setCopiedLesson] = useState(false);

  const [isUploading, setIsUploading] = useState(false);
  const [newBookId, setNewBookId] = useState<string | null>(null);
  const [modelManagerOpen, setModelManagerOpen] = useState(false);
  const [modelConfig, setModelConfig] = useState<ModelConfig>({ llm_model: "gemma2:2b", embed_model: "nomic-embed-text" });
  const [showChunksPanel, setShowChunksPanel] = useState(false);

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

  useEffect(() => {
    fetchBooks();
    fetchModelConfig();
  }, [fetchBooks, fetchModelConfig]);

  useEffect(() => {
    const loadStoredData = async () => {
      if (!selectedBookId) {
        setLearningPath("");
        setLesson("");
        setSelectedConcept("");
        return;
      }
      try {
        const storedPath = await invoke<string | null>("get_stored_learning_path", { bookId: selectedBookId });
        if (storedPath) {
          setLearningPath(storedPath);
        } else {
          setLearningPath("");
        }
        setLesson("");
        setSelectedConcept("");
        setLearnTab("concepts");
      } catch (error) {
        console.error("Error loading stored data:", error);
      }
    };
    loadStoredData();
  }, [selectedBookId]);

  // Parse markdown content from learning path into structured concept cards
  const parsedConcepts = useMemo<ConceptItem[]>(() => {
    if (!learningPath) return [];

    const lines = learningPath.split("\n");
    const concepts: ConceptItem[] = [];
    let currentTitle = "";
    let currentDesc: string[] = [];

    const saveCurrent = () => {
      if (currentTitle) {
        concepts.push({
          id: currentTitle.toLowerCase().replace(/[^a-z0-9]+/g, "-"),
          title: currentTitle,
          description: currentDesc.join(" ").trim() || "Master this key core concept from the selected book."
        });
      }
    };

    lines.forEach((line) => {
      const trimmed = line.trim();
      // Match headings or bullet items with titles
      const headingMatch = trimmed.match(/^(?:#{1,4}|\d+\.|\*|-)\s+\*?\*?([^:*#]+)\*?\*?:?(.*)$/);
      if (headingMatch && headingMatch[1].trim().length > 2) {
        saveCurrent();
        currentTitle = headingMatch[1].replace(/\*\*/g, "").trim();
        currentDesc = headingMatch[2] ? [headingMatch[2].trim()] : [];
      } else if (trimmed && currentTitle) {
        currentDesc.push(trimmed.replace(/^[-*]\s+/, ""));
      }
    });
    saveCurrent();

    return concepts;
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
      setLesson("");
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
      setIsGenerating(false);
    }
  };

  const handleGetLesson = async (concept: string) => {
    if (!selectedBookId || !concept.trim()) return;
    setSelectedConcept(concept);
    setLearnTab("lesson");
    setIsGenerating(true);
    setConsoleMsg(`Generating lesson on "${concept}"...`);
    try {
      const result = await invoke<string>("generate_lesson", { concept, bookId: selectedBookId });
      setLesson(result);
      setConsoleMsg("Lesson generated!");
    } catch (error) {
      setConsoleMsg(`Error: ${error}`);
    } finally {
      setIsGenerating(false);
    }
  };

  const handleCopyLesson = () => {
    if (!lesson) return;
    navigator.clipboard.writeText(lesson);
    setCopiedLesson(true);
    setTimeout(() => setCopiedLesson(false), 2000);
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

        <button className="sidebar-bottom-btn" onClick={() => setModelManagerOpen(true)}>
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="12" cy="12" r="3" />
            <path d="M12 2v3M12 19v3M4.22 4.22l2.12 2.12M17.66 17.66l2.12 2.12M2 12h3M19 12h3M4.22 19.78l2.12-2.12M17.66 6.34l2.12-2.12" />
          </svg>
          AI Models
        </button>
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
            <span className="status-chip">{consoleMsg || "Ready"}</span>
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
                      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <rect x="3" y="3" width="18" height="18" rx="2" /><path d="M3 9h18M9 21V9" />
                      </svg>
                      Knowledge Grains
                      <span className="panel-count">{chunks.length}</span>
                      <button className="panel-close" onClick={() => setShowChunksPanel(false)}>×</button>
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
                  <p>Select a book from your shelf, then let AI identify the key concepts and create structured lessons for you.</p>
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
                  <p>Analysing your book's key concepts...</p>
                </div>
              )}

              {learningPath && (
                <div className="learn-container">
                  {/* Learn Mode Navigation Header */}
                  <div className="learn-nav-header">
                    <div className="learn-tabs">
                      <button
                        className={`learn-tab-btn ${learnTab === "concepts" ? "active" : ""}`}
                        onClick={() => setLearnTab("concepts")}
                      >
                        <Sparkles className="w-4 h-4" />
                        Key Concepts
                        {parsedConcepts.length > 0 && (
                          <span className="learn-tab-badge">{parsedConcepts.length}</span>
                        )}
                      </button>
                      <button
                        className={`learn-tab-btn ${learnTab === "lesson" ? "active" : ""}`}
                        onClick={() => setLearnTab("lesson")}
                      >
                        <BookOpen className="w-4 h-4" />
                        Lesson
                        {selectedConcept && <span className="learn-tab-dot" />}
                      </button>
                    </div>

                    <div className="learn-header-actions">
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
                    </div>
                  </div>

                  {/* TAB 1: CONCEPTS */}
                  {learnTab === "concepts" && (
                    <div className="concepts-tab-content animate-fadein">
                      {/* Concept search & custom prompt box */}
                      <div className="custom-concept-bar">
                        <input
                          type="text"
                          placeholder="Or type a specific concept/topic to study..."
                          className="custom-concept-input"
                          value={customConcept}
                          onChange={(e) => setCustomConcept(e.target.value)}
                          onKeyDown={(e) => e.key === "Enter" && handleGetLesson(customConcept)}
                        />
                        <Button
                          onClick={() => handleGetLesson(customConcept)}
                          disabled={isGenerating || !customConcept.trim()}
                          size="sm"
                        >
                          Study Custom Topic
                        </Button>
                      </div>

                      {parsedConcepts.length > 0 ? (
                        <div className="concepts-grid">
                          {parsedConcepts.map((item, idx) => (
                            <div
                              key={item.id || idx}
                              className={`concept-card ${selectedConcept === item.title ? "active" : ""}`}
                              onClick={() => handleGetLesson(item.title)}
                            >
                              <div className="concept-card-header">
                                <span className="concept-number">{idx + 1}</span>
                                <h3 className="concept-card-title">{item.title}</h3>
                              </div>
                              {item.description && (
                                <p className="concept-card-desc">{item.description}</p>
                              )}
                              <div className="concept-card-footer">
                                <span className="concept-tag">Concept</span>
                                <span className="concept-action">
                                  Study Lesson <ArrowRight className="w-3.5 h-3.5 ml-1 inline" />
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

                  {/* TAB 2: LESSON */}
                  {learnTab === "lesson" && (
                    <div className="lesson-tab-content animate-fadein">
                      <div className="lesson-workspace">
                        <div className="lesson-workspace-header">
                          <div className="lesson-topic-title">
                            {selectedConcept ? (
                              <>
                                <span className="topic-subtitle">CURRENT LESSON</span>
                                <h2>{selectedConcept}</h2>
                              </>
                            ) : (
                              <h2>Lesson Workspace</h2>
                            )}
                          </div>
                          {lesson && (
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={handleCopyLesson}
                              className="copy-lesson-btn"
                            >
                              {copiedLesson ? (
                                <><Check className="w-3.5 h-3.5 mr-1 text-green-500" /> Copied</>
                              ) : (
                                <><Copy className="w-3.5 h-3.5 mr-1" /> Copy Lesson</>
                              )}
                            </Button>
                          )}
                        </div>

                        {lesson ? (
                          <div className="lesson-body prose prose-neutral dark:prose-invert max-w-none">
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>{lesson}</ReactMarkdown>
                          </div>
                        ) : (
                          <div className="panel-empty lesson-empty-state">
                            {isGenerating ? (
                              <>
                                <Loader2Icon className="w-10 h-10 animate-spin text-primary mb-3" />
                                <h3>Generating Comprehensive Lesson...</h3>
                                <p>Synthesizing key insights from your selected book.</p>
                              </>
                            ) : (
                              <>
                                <BookOpen className="w-10 h-10 text-muted-foreground mb-3 opacity-60" />
                                <h3>No Lesson Active</h3>
                                <p>Select a concept from the <strong>Key Concepts</strong> tab or type a topic to generate a detailed lesson.</p>
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
                </div>
              )}
            </div>
          )}
        </div>
      </main>

      {/* Model Manager Overlay */}
      <ModelManager
        isOpen={modelManagerOpen}
        onClose={() => setModelManagerOpen(false)}
        onConfigChange={(cfg) => setModelConfig(cfg)}
      />
    </div>
  );
}

export default App;