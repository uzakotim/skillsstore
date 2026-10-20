import { useState, useEffect, useCallback, useMemo } from "react";
import "./App.css";
import UploadButton from "@/components/custom/UploadButton";
import BookShelf from "@/components/custom/BookShelf";
import ModelManager from "@/components/custom/ModelManager";
import SearchMode from "@/components/custom/SearchMode";
import LearnMode from "@/components/custom/LearnMode";
import Sidebar from "@/components/custom/Sidebar";
import Header from "@/components/custom/Header";
import { useAtom } from "jotai";
import { consoleMsgAtom } from "@/store/atoms";
import { invoke } from "@tauri-apps/api/core";
import { confirm, save, open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import type {
  Chunk,
  Book,
  ModelConfig,
  ConceptItem,
  AnalysisProgress,
  LearnTab,
} from "@/types";

export type {
  Chunk,
  Book,
  ModelConfig,
  ConceptItem,
  AnalysisProgress,
  LearnTab,
};

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
  const [learnTab, setLearnTab] = useState<LearnTab>("concepts");
  const [lesson, setLesson] = useState("");

  const [learningPath, setLearningPath] = useState("");
  const [relatedExcerpts, setRelatedExcerpts] = useState<string[]>([]);
  const [selectedConcept, setSelectedConcept] = useState("");

  const [isUploading, setIsUploading] = useState(false);
  const [newBookId, setNewBookId] = useState<string | null>(null);
  const [modelManagerOpen, setModelManagerOpen] = useState(false);
  const [modelConfig, setModelConfig] = useState<ModelConfig>({ llm_model: "gemma2:2b", embed_model: "nomic-embed-text" });
  const [showChunksPanel, setShowChunksPanel] = useState(false);
  const [analysisProgress, setAnalysisProgress] = useState<AnalysisProgress | null>(null);
  const [isExporting, setIsExporting] = useState(false);

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
    setLearningPath("");
    setRelatedExcerpts([]);
    setSelectedConcept("");
    setLesson("");

    if (!selectedBookId) {
      return;
    }

    let cancelled = false;

    const loadStoredData = async () => {
      try {
        const storedPath = await invoke<string | null>("get_stored_learning_path", { bookId: selectedBookId });
        if (cancelled) return;
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
        if (!cancelled) {
          console.error("Error loading stored data:", error);
        }
      }
    };
    loadStoredData();

    return () => {
      cancelled = true;
    };
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

  const handleExportNotes = async () => {
    if (!selectedBookId || parsedConcepts.length === 0) {
      setConsoleMsg("No extracted concepts to export for this book.");
      return;
    }

    const currentBook = books.find((b) => b.id === selectedBookId);
    const bookTitle = currentBook?.title || "Book";

    try {
      const chosenPath = await open({
        directory: true,
        multiple: false,
        title: `Select folder to export notes for "${bookTitle}"`,
      });

      if (!chosenPath || typeof chosenPath !== "string") {
        return;
      }

      setIsExporting(true);
      setConsoleMsg(`Exporting notes for "${bookTitle}"...`);

      // Retrieve any cached/stored lessons for this book to enrich the notes if available
      const lessonMap = new Map<string, string>();
      try {
        const storedLessons = await invoke<[string, string][]>("get_stored_lessons", {
          bookId: selectedBookId,
        });
        if (Array.isArray(storedLessons)) {
          for (const [cConcept, cLesson] of storedLessons) {
            if (cConcept && cLesson) {
              lessonMap.set(cConcept.trim().toLowerCase(), cLesson.trim());
            }
          }
        }
      } catch (err) {
        console.warn("Could not load stored lessons for export:", err);
      }

      // Format each concept as a .md file note suitable for Notes app
      const notes = parsedConcepts.map((concept) => {
        let md = `# ${concept.title}\n\n`;

        if (concept.description) {
          md += `${concept.description}\n\n`;
        }

        if (concept.excerpts && concept.excerpts.length > 0) {
          md += `### Excerpts\n\n`;
          md += concept.excerpts.map((e) => `* "${e}"`).join("\n\n");
          md += `\n\n`;
        }

        const storedLesson = lessonMap.get(concept.title.trim().toLowerCase());
        if (storedLesson) {
          md += `### Detailed Lesson\n\n${storedLesson}\n\n`;
        }

        md += `---\n*Source: ${bookTitle}*\n`;

        return {
          title: concept.title,
          content: md,
        };
      });

      const result = await invoke<{ count: number; folder_path: string }>("export_notes", {
        destinationDir: chosenPath,
        bookTitle,
        notes,
      });

      setConsoleMsg(
        `Successfully exported ${result.count} note${result.count === 1 ? "" : "s"} to "${result.folder_path}". Ready for Notes app!`
      );
    } catch (error) {
      console.error("Export notes error:", error);
      setConsoleMsg(`Error exporting notes: ${String(error)}`);
    } finally {
      setIsExporting(false);
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
      <Sidebar
        mode={mode}
        onModeChange={setMode}
        modelConfig={modelConfig}
        ollamaStatus={ollamaStatus}
        onOpenModelManager={() => setModelManagerOpen(true)}
      />

      {/* Main area */}
      <main className="app-main">
        <Header
          mode={mode}
          selectedBook={selectedBook}
          consoleMsg={consoleMsg}
          onSaveDatabase={handleSaveDatabase}
          onLoadDatabase={handleLoadDatabase}
          onDebugChunks={handleGetChunks}
        />
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
              hasExtractedConcepts={Boolean(selectedBookId && parsedConcepts.length > 0)}
              onExportNotes={handleExportNotes}
              isExporting={isExporting}
            />
            <UploadButton onUploadStart={handleUploadStart} onUpload={handleUploadDone} />
          </div>
        </div>

        {/* Content area */}
        <div className="content-area">
          {mode === "search" ? (
            <SearchMode
              selectedBook={selectedBook}
              searchQuery={searchQuery}
              onSearchQueryChange={setSearchQuery}
              onSearch={handleSearch}
              onGenerate={handleGenerate}
              isGenerating={isGenerating}
              searchResults={searchResults}
              aiResponse={aiResponse}
              showChunksPanel={showChunksPanel}
              onCloseChunksPanel={() => setShowChunksPanel(false)}
              chunks={chunks}
            />
          ) : (
            <LearnMode
              selectedBookId={selectedBookId}
              isGenerating={isGenerating}
              analysisProgress={analysisProgress}
              learningPath={learningPath}
              parsedConcepts={parsedConcepts}
              learnTab={learnTab}
              onTabChange={setLearnTab}
              selectedConcept={selectedConcept}
              onSelectConcept={setSelectedConcept}
              relatedExcerpts={relatedExcerpts}
              onSetRelatedExcerpts={setRelatedExcerpts}
              lesson={lesson}
              onGenerateLearningPath={handleGenerateLearningPath}
              onGetRelatedExcerpts={handleGetRelatedExcerpts}
              onGenerateLesson={handleGenerateExplanation}
            />
          )}
        </div>
      </main>

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