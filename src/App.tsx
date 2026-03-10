import { useState, useEffect, useCallback } from "react";
import "./App.css";
import UploadButton from "@/components/custom/UploadButton";
import { useAtom } from "jotai";
import { consoleMsgAtom } from "@/store/atoms";
import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Loader2Icon } from "lucide-react";

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

  const fetchBooks = useCallback(async () => {
    try {
      const data = await invoke<Book[]>("get_books");
      setBooks(data);
    } catch (error) {
      console.error("Error fetching books:", error);
    }
  }, []);

  useEffect(() => {
    fetchBooks();
  }, [fetchBooks]);

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
        
        // Reset lesson view when changing books
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
    
    const confirmDelete = await window.confirm("Are you sure you want to delete this book and all its chunks? This action cannot be undone.");
    if (!confirmDelete) return;

    try {
      await invoke("delete_book", { bookId: selectedBookId });
      setConsoleMsg("Book and its chunks deleted successfully.");
      setSelectedBookId("");
      setChunks([]);
      setSearchResults([]);
      setLearningPath("");
      setLesson("");
      setSelectedConcept("");
      fetchBooks();
    } catch (error) {
      setConsoleMsg(`Error deleting book: ${error}`);
    }
  };

  const handleGetChunks = async () => {
    try {
      const data = await invoke<Chunk[]>("get_chunks", { 
        bookId: selectedBookId || null 
      });
      setChunks(data);
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
        bookId: selectedBookId || null
      });
      setSearchResults(results);
      setConsoleMsg(`Found ${results.length} relevant chunks${selectedBookId ? " (filtered by book)" : ""}`);
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
      bookId: selectedBookId || null
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
    setConsoleMsg("Finding concepts from the book...");
    try {
      const path = await invoke<string>("generate_learning_path", { 
        bookId: selectedBookId 
      });
      setLearningPath(path);
      setConsoleMsg("Concepts found!");
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
    setConsoleMsg(`Generating a lesson for ${concept}...`);
    try {
      const result = await invoke<string>("generate_lesson", { 
        concept, 
        bookId: selectedBookId 
      });
      setLesson(result);
      setConsoleMsg("Lesson generated!");
    } catch (error) {
      setConsoleMsg(`Error: ${error}`);
    } finally {
      setIsGenerating(false);
    }
  };

  return (
    <div className="flex flex-col gap-6 p-10 min-h-screen w-screen items-center bg-background text-foreground overflow-auto">
      <header className="flex flex-col items-center gap-2 mt-8">
        <h1 className="text-4xl font-extrabold tracking-tight lg:text-5xl">
          SkillsStore
        </h1>
        <p className="text-muted-foreground">Retrieval-Augmented Generation Dashboard</p>
      </header>

      <div className="flex gap-4 items-center flex-wrap justify-center bg-card p-4 rounded-2xl border shadow-sm w-full max-w-4xl">
        <div className="flex gap-2 p-1 bg-muted rounded-xl">
          <Button 
            variant={mode === "search" ? "default" : "ghost"} 
            size="sm"
            onClick={() => setMode("search")}
            className="rounded-lg px-6"
          >
            Search Mode
          </Button>
          <Button 
            variant={mode === "learn" ? "default" : "ghost"} 
            size="sm"
            onClick={() => setMode("learn")}
            className="rounded-lg px-6"
          >
            Learn Mode
          </Button>
        </div>

        <div className="h-8 w-px bg-border mx-2" />

        <UploadButton onUpload={fetchBooks} />
        
        <div className="flex items-center gap-2 bg-muted/50 p-1 rounded-lg border">
          <label className="text-xs font-medium px-2">Book:</label>
          <select 
            className="bg-transparent text-sm p-1.5 outline-none border-none min-w-[200px]"
            value={selectedBookId}
            onChange={(e) => setSelectedBookId(e.target.value)}
          >
            <option value="">All Uploaded Books</option>
            {books.map((book) => (
              <option key={book.id} value={book.id}>
                {book.title}
              </option>
            ))}
          </select>
          {selectedBookId && (
            <Button 
              variant="destructive" 
              size="icon-xs" 
              onClick={handleDeleteBook}
              title="Delete this book"
              className="mr-1"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/><line x1="10" x2="10" y1="11" y2="17"/><line x1="14" x2="14" y1="11" y2="17"/></svg>
            </Button>
          )}
        </div>

        <Button variant="outline" onClick={handleGetChunks} size="sm">
          {selectedBookId ? "Debug: View Chunks" : "Debug: View All"}
        </Button>
      </div>

      <div className="flex flex-col gap-2 items-center">
         <span className="text-xs font-mono bg-muted px-2 py-1 rounded shadow-sm border">Status: {consoleMsg}</span>
      </div>

      {mode === "search" ? (
        <div className="w-full max-w-6xl space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
          <div className="flex flex-col gap-4 w-full max-w-2xl mx-auto mt-4">
            <div className="flex gap-2">
              <input
                type="text"
                placeholder={selectedBookId ? "Search in this book..." : "Search in all books..."}
                className="flex h-11 w-full rounded-xl border border-input bg-background px-4 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 shadow-sm"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
              />
              <Button onClick={handleSearch} className="rounded-xl h-11 px-6">
                Search
              </Button>
              <Button variant="secondary" onClick={handleGenerate} disabled={isGenerating || searchResults.length === 0} className="rounded-xl h-11 px-6">
                {isGenerating ? "Processing..." : "Ask AI"}
              </Button>
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
            <div className="flex flex-col gap-6">
              <section className="flex flex-col gap-4">
                <h2 className="text-xl font-semibold border-b pb-2 flex justify-between items-center text-primary">
                  AI Perspective 
                </h2>
                {aiResponse ? (
                  <div className="p-6 rounded-2xl bg-primary/[0.03] border border-primary/10 text-sm leading-relaxed shadow-sm prose prose-sm dark:prose-invert max-w-none">
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>
                      {aiResponse}
                    </ReactMarkdown>
                  </div>
                ) : (
                  <div className="h-32 flex items-center justify-center border border-dashed rounded-2xl text-muted-foreground text-sm italic bg-muted/20">
                    {isGenerating ? "Synthesizing answer..." : "Search and click 'Ask AI' to generate a response."}
                  </div>
                )}
              </section>

              <section className="flex flex-col gap-4">
                <h2 className="text-xl font-semibold border-b pb-2">Context Foundations</h2>
                {searchResults.length > 0 ? (
                  <div className="flex flex-col gap-3">
                    {searchResults.map((res, i) => (
                      <div key={i} className="p-4 rounded-xl bg-card border text-sm shadow-sm hover:shadow-md transition-all hover:border-primary/20">
                         <p className="text-card-foreground italic">"{res}"</p>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground italic text-center py-10 border rounded-xl bg-muted/10">No context retrieved yet.</p>
                )}
              </section>
            </div>

            <section className="flex flex-col gap-4">
              <h2 className="text-xl font-semibold border-b pb-2 flex justify-between items-center">
                Knowledge Grains
                <span className="text-xs font-normal text-muted-foreground">{chunks.length} segments</span>
              </h2>
              {chunks.length > 0 ? (
                <div className="flex flex-col gap-3 max-h-[800px] overflow-y-auto pr-2 custom-scrollbar">
                  {chunks.map((chunk) => (
                    <div key={chunk.id} className="p-4 rounded-xl bg-muted/20 border text-xs hover:bg-muted/40 transition-colors">
                      <div className="flex justify-between items-center mb-2 opacity-70">
                        <span className="font-bold">Segment {chunk.chunk_index}</span>
                        <span className="font-mono bg-background px-1.5 py-0.5 rounded border text-[10px]">{chunk.book_id.slice(0,8)}</span>
                      </div>
                      <p className="line-clamp-4 leading-relaxed text-muted-foreground">{chunk.content}</p>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="flex flex-col items-center justify-center py-20 border rounded-2xl bg-muted/10 gap-2">
                  <p className="text-sm text-muted-foreground italic text-center">Click "View Chunks" to inspect database.</p>
                </div>
              )}
            </section>
          </div>
        </div>
      ) : (
        <div className="w-full max-w-6xl space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
          <div className="flex flex-col items-center gap-6 mt-4">
            {!learningPath && !isGenerating && (
              <div className="text-center space-y-4 py-20 px-10 border border-dashed rounded-3xl bg-muted/5 max-w-2xl">
                <div className="w-16 h-16 bg-primary/10 rounded-full flex items-center justify-center mx-auto mb-4">
                  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-primary"><path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 2.5-2.5Z"/><path d="M8 7h6"/><path d="M8 11h8"/><path d="M8 15h6"/></svg>
                </div>
                <h3 className="text-2xl font-bold">Start Your Learning Journey</h3>
                <p className="text-muted-foreground">Select a book and let the AI create a personalized list of key concepts and structured lessons.</p>
                <Button onClick={handleGenerateLearningPath} size="lg" className="rounded-xl px-8 mt-4" disabled={!selectedBookId}>
                  Find Concepts
                </Button>
              </div>
            )}

            {isGenerating && !learningPath && (
              <div className="flex flex-col items-center gap-4 py-20">
                <Loader2Icon className="w-16 h-16 text-primary animate-spin" />
                <p className="text-muted-foreground animate-pulse">Finding concepts...</p>
              </div>
            )}

            {learningPath && (
              <div className="grid grid-cols-1 lg:grid-cols-3 gap-8 w-full">
                <div className="lg:col-span-1 space-y-6">
                  <section className="flex flex-col gap-4">
                    <h2 className="text-xl font-semibold border-b pb-2 flex justify-between items-center text-primary">
                      Concepts
                    </h2>
                    <div className="p-6 rounded-2xl bg-card border shadow-sm prose prose-sm dark:prose-invert max-w-none">
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>
                        {learningPath}
                      </ReactMarkdown>
                    </div>
                  </section>
                  
                  <div className="flex flex-col gap-4">
                    <h3 className="text-sm font-semibold text-muted-foreground uppercase tracking-wider px-1">Study specific concept</h3>
                    <div className="flex gap-2">
                       <input 
                         type="text" 
                         placeholder="Enter concept name..."
                         className="flex h-10 w-full rounded-xl border border-input bg-background px-3 py-2 text-sm"
                         value={selectedConcept}
                         onChange={(e) => setSelectedConcept(e.target.value)}
                         onKeyDown={(e) => e.key === 'Enter' && handleGetLesson(selectedConcept)}
                       />
                       <Button onClick={() => handleGetLesson(selectedConcept)} disabled={isGenerating || !selectedConcept}>
                         Get Lesson
                       </Button>
                    </div>
                  </div>
                </div>

                <div className="lg:col-span-2">
                  <section className="flex flex-col gap-4 h-full">
                    <h2 className="text-xl font-semibold border-b pb-2 text-primary">
                      Current Lesson {selectedConcept && `: ${selectedConcept}`}
                    </h2>
                    {lesson ? (
                      <div className="p-8 rounded-3xl bg-primary/[0.02] border border-primary/10 text-base leading-relaxed shadow-sm prose prose-neutral dark:prose-invert max-w-none min-h-[400px]">
                        <ReactMarkdown remarkPlugins={[remarkGfm]}>
                          {lesson}
                        </ReactMarkdown>
                      </div>
                    ) : (
                      <div className="flex-1 flex flex-col items-center justify-center border border-dashed rounded-3xl text-muted-foreground text-sm italic bg-muted/5 min-h-[400px] p-10 text-center">
                        {isGenerating ? (
                           <div className="flex flex-col items-center gap-4">
                             <Loader2Icon className="w-8 h-8 text-primary animate-spin" />
                             <p>Preparing lesson material...</p>
                           </div>
                        ) : "Select a concept from the path or type one above to begin a lesson."}
                      </div>
                    )}
                  </section>
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
