import { useState, useEffect, useCallback } from "react";
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
import { Loader2Icon } from "lucide-react";
import { confirm } from "@tauri-apps/plugin-dialog"

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
  const [learningPath, setLearningPath] = useState("");
  const [lesson, setLesson] = useState("");
  const [selectedConcept, setSelectedConcept] = useState("");
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
      } catch (error) {
        console.error("Error loading stored data:", error);
      }
    };
    loadStoredData();
  }, [selectedBookId]);

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
      setConsoleMsg("Concepts ready!");
    } catch (error) {
      setConsoleMsg(`Error: ${error}`);
    } finally {
      setIsGenerating(false);
    }
  };

  const handleGetLesson = async (concept: string) => {
    if (!selectedBookId) return;
    setSelectedConcept(concept);
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

  const handleUploadStart = () => {
    setIsUploading(true);
    setNewBookId(null);
  };

  const handleUploadDone = async () => {
    setIsUploading(false);
    // Get the freshly uploaded book (last one)
    try {
      const data = await invoke<Book[]>("get_books");
      setBooks(data);
      if (data.length > 0) {
        const lastBook = data[data.length - 1];
        setNewBookId(lastBook.id);
        // Clear the "new" glow after 3 seconds
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
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M22 10v6M2 10l10-5 10 5-10 5z" />
              <path d="M6 12v5c3 3 9 3 12 0v-5" />
            </svg>
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

        {/* Model manager button */}
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
              {/* Search bar */}
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
                {/* AI response */}
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

                {/* Context chunks */}
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

                {/* Debug chunks panel */}
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
            <div className="learn-mode animate-fadein">
              {!learningPath && !isGenerating && (
                <div className="learn-start">
                  <div className="learn-icon">
                    <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20" />
                      <line x1="8" y1="7" x2="14" y2="7" /><line x1="8" y1="11" x2="16" y2="11" /><line x1="8" y1="15" x2="14" y2="15" />
                    </svg>
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
                <div className="learn-grid">
                  <div className="concepts-panel">
                    <div className="panel-header">
                      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
                      </svg>
                      Key Concepts
                    </div>
                    <div className="concepts-content prose prose-sm dark:prose-invert max-w-none">
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>{learningPath}</ReactMarkdown>
                    </div>
                    <div className="concept-input-row">
                      <input
                        type="text"
                        placeholder="Type a concept to study..."
                        className="concept-input"
                        value={selectedConcept}
                        onChange={(e) => setSelectedConcept(e.target.value)}
                        onKeyDown={(e) => e.key === "Enter" && handleGetLesson(selectedConcept)}
                      />
                      <Button
                        onClick={() => handleGetLesson(selectedConcept)}
                        disabled={isGenerating || !selectedConcept}
                        size="sm"
                      >
                        Study
                      </Button>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => { setLearningPath(""); setLesson(""); setSelectedConcept(""); }}
                      className="w-full mt-2"
                    >
                      Regenerate Concepts
                    </Button>
                  </div>

                  <div className="lesson-panel">
                    <div className="panel-header">
                      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
                        <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
                      </svg>
                      {selectedConcept ? `Lesson: ${selectedConcept}` : "Lesson"}
                    </div>
                    {lesson ? (
                      <div className="lesson-content prose prose-neutral dark:prose-invert max-w-none">
                        <ReactMarkdown remarkPlugins={[remarkGfm]}>{lesson}</ReactMarkdown>
                      </div>
                    ) : (
                      <div className="panel-empty">
                        {isGenerating ? (
                          <><Loader2Icon className="w-8 h-8 animate-spin text-primary mb-3" /><p>Preparing your lesson...</p></>
                        ) : (
                          <>
                            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="text-muted-foreground mb-3">
                              <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
                              <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
                            </svg>
                            <p>Select a concept from the list or type one above to generate a lesson.</p>
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
