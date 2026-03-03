import { useState } from "react";
import "./App.css";
import UploadButton from "@/components/custom/UploadButton";
import { useAtom } from "jotai";
import { consoleMsgAtom } from "@/store/atoms";
import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";

interface Chunk {
  id: number;
  book_id: string;
  chunk_index: number;
  content: string;
}

function App() {
  const [consoleMsg, setConsoleMsg] = useAtom(consoleMsgAtom);
  const [chunks, setChunks] = useState<Chunk[]>([]);
  const [searchResults, setSearchResults] = useState<string[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [aiResponse, setAiResponse] = useState("");
  const [isGenerating, setIsGenerating] = useState(false);

  const handleGetChunks = async () => {
    try {
      const data = await invoke<Chunk[]>("get_chunks");
      setChunks(data);
      setConsoleMsg(`Fetched ${data.length} chunks`);
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
      const results = await invoke<string[]>("search_context", { query: searchQuery });
      setSearchResults(results);
      setConsoleMsg(`Found ${results.length} relevant context chunks`);
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
    setConsoleMsg("Simulating LLM generation...");
    
    // Simulate LLM delay
    setTimeout(() => {
      setAiResponse("This is a simulated AI response based on the context retrieved above. In the future, this will be replaced by an actual call to an LLM like DeepSeek or GPT-4, using the retrieved chunks as context.");
      setIsGenerating(false);
      setConsoleMsg("AI generation complete (Simulation)");
    }, 1500);
  };

  return (
    <div className="flex flex-col gap-6 p-10 min-h-screen w-screen items-center bg-background text-foreground overflow-auto">
      <header className="flex flex-col items-center gap-2 mt-8">
        <h1 className="text-4xl font-extrabold tracking-tight lg:text-5xl">
          SkillsStore
        </h1>
        <p className="text-muted-foreground">Retrieval-Augmented Generation Dashboard</p>
      </header>

      <div className="flex gap-4 items-center">
        <UploadButton />
        <Button variant="outline" onClick={handleGetChunks}>
          Fetch All Chunks
        </Button>
      </div>

      <div className="flex flex-col gap-4 w-full max-w-2xl mt-4">
        <div className="flex gap-2">
          <input
            type="text"
            placeholder="Search in context..."
            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
          />
          <Button onClick={handleSearch}>
            Search
          </Button>
          <Button variant="secondary" onClick={handleGenerate} disabled={isGenerating || searchResults.length === 0}>
            {isGenerating ? "Generating..." : "Ask AI"}
          </Button>
        </div>
      </div>

      <div className="flex flex-col gap-2 items-center">
         <span className="text-xs font-mono bg-muted px-2 py-1 rounded shadow-sm border">Status: {consoleMsg}</span>
      </div>

      <div className="w-full max-w-6xl grid grid-cols-1 md:grid-cols-2 gap-8 mt-4">
        <div className="flex flex-col gap-6">
          <section className="flex flex-col gap-4">
            <h2 className="text-xl font-semibold border-b pb-2 flex justify-between items-center">
              AI Response 
              <span className="text-[10px] uppercase tracking-widest text-muted-foreground font-normal">Placeholder</span>
            </h2>
            {aiResponse ? (
              <div className="p-6 rounded-xl bg-primary/5 border border-primary/20 text-sm leading-relaxed shadow-sm">
                {aiResponse}
              </div>
            ) : (
              <div className="h-24 flex items-center justify-center border border-dashed rounded-xl text-muted-foreground text-sm italic">
                {isGenerating ? "Reasoning..." : "Search and click 'Ask AI' to generate a response."}
              </div>
            )}
          </section>

          <section className="flex flex-col gap-4">
            <h2 className="text-xl font-semibold border-b pb-2">Retrieved Context</h2>
            {searchResults.length > 0 ? (
              <div className="flex flex-col gap-3">
                {searchResults.map((res, i) => (
                  <div key={i} className="p-4 rounded-lg bg-card border text-sm shadow-sm hover:shadow-md transition-shadow">
                     <p className="text-card-foreground">"{res}"</p>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-muted-foreground italic text-center py-10 border rounded-lg">No context retrieved yet.</p>
            )}
          </section>
        </div>

        <section className="flex flex-col gap-4">
          <h2 className="text-xl font-semibold border-b pb-2 flex justify-between items-center">
            Database Chunks
            <span className="text-xs font-normal text-muted-foreground">{chunks.length} objects</span>
          </h2>
          {chunks.length > 0 ? (
            <div className="flex flex-col gap-3 max-h-[800px] overflow-y-auto pr-2 custom-scrollbar">
              {chunks.map((chunk) => (
                <div key={chunk.id} className="p-4 rounded-lg bg-muted/30 border text-xs hover:bg-muted/50 transition-colors">
                  <div className="flex justify-between items-center mb-2 opacity-70">
                    <span className="font-bold">Index: {chunk.chunk_index}</span>
                    <span className="font-mono bg-background px-1 rounded border text-[10px]">{chunk.book_id.slice(0,8)}...</span>
                  </div>
                  <p className="line-clamp-4 leading-relaxed">{chunk.content}</p>
                </div>
              ))}
            </div>
          ) : (
             <p className="text-sm text-muted-foreground italic text-center py-20 border rounded-lg">Click "Fetch All Chunks" to view database contents.</p>
          )}
        </section>
      </div>
    </div>
  );
}

export default App;
