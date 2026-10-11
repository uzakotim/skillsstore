import React from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Loader2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Book, Chunk } from "@/types";

interface SearchModeProps {
  selectedBook?: Book;
  searchQuery: string;
  onSearchQueryChange: (query: string) => void;
  onSearch: () => void;
  onGenerate: () => void;
  isGenerating: boolean;
  searchResults: string[];
  aiResponse: string;
  showChunksPanel: boolean;
  onCloseChunksPanel: () => void;
  chunks: Chunk[];
}

export const SearchMode: React.FC<SearchModeProps> = ({
  selectedBook,
  searchQuery,
  onSearchQueryChange,
  onSearch,
  onGenerate,
  isGenerating,
  searchResults,
  aiResponse,
  showChunksPanel,
  onCloseChunksPanel,
  chunks,
}) => {
  return (
    <div className="search-mode animate-fadein">
      <div className="search-bar-row">
        <div className="search-input-wrap">
          <svg
            className="search-icon"
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
          <input
            type="text"
            placeholder={
              selectedBook
                ? `Search in "${selectedBook.title}"...`
                : "Search across all books..."
            }
            className="search-input"
            value={searchQuery}
            onChange={(e) => onSearchQueryChange(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && onSearch()}
          />
        </div>
        <Button onClick={onSearch} className="search-btn">
          Search
        </Button>
        <Button
          variant="secondary"
          onClick={onGenerate}
          disabled={isGenerating || searchResults.length === 0}
          className="ask-btn"
        >
          {isGenerating ? (
            <>
              <Loader2Icon className="w-4 h-4 animate-spin mr-1" /> Thinking...
            </>
          ) : (
            "Ask AI"
          )}
        </Button>
      </div>

      <div className="search-results-grid">
        <div className="ai-response-panel">
          <div className="panel-header">
            <svg
              width="15"
              height="15"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <path d="M12 2a10 10 0 1 0 10 10" />
              <path d="M12 6v6l4 2" />
            </svg>
            AI Perspective
          </div>
          {aiResponse ? (
            <div className="ai-response-content prose prose-sm dark:prose-invert max-w-none">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {aiResponse}
              </ReactMarkdown>
            </div>
          ) : (
            <div className="panel-empty">
              {isGenerating ? (
                <>
                  <Loader2Icon className="w-8 h-8 animate-spin text-primary mb-3" />
                  <p>Synthesizing answer from your books...</p>
                </>
              ) : (
                <>
                  <svg
                    width="32"
                    height="32"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="1.5"
                    className="text-muted-foreground mb-3"
                  >
                    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
                  </svg>
                  <p>
                    Search for something, then click <strong>Ask AI</strong> to
                    get an answer from your books.
                  </p>
                </>
              )}
            </div>
          )}
        </div>

        <div className="context-panel">
          <div className="panel-header">
            <svg
              width="15"
              height="15"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <polyline points="14 2 14 8 20 8" />
            </svg>
            Source Passages
            {searchResults.length > 0 && (
              <span className="panel-count">{searchResults.length}</span>
            )}
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
              <svg
                width="28"
                height="28"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                className="text-muted-foreground mb-3"
              >
                <circle cx="11" cy="11" r="8" />
                <line x1="21" y1="21" x2="16.65" y2="16.65" />
              </svg>
              <p>Search results will appear here.</p>
            </div>
          )}
        </div>

        {showChunksPanel && chunks.length > 0 && (
          <div className="chunks-panel">
            <div className="panel-header">
              <div className="flex items-center gap-2">
                <svg
                  width="15"
                  height="15"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <rect x="3" y="3" width="18" height="18" rx="2" />
                  <path d="M3 9h18M9 21V9" />
                </svg>
                Knowledge Grains
              </div>
              <div className="flex items-center gap-2">
                <span className="panel-count">{chunks.length}</span>
                <button
                  className="panel-close"
                  onClick={onCloseChunksPanel}
                >
                  ×
                </button>
              </div>
            </div>
            <div className="chunks-list">
              {chunks.map((chunk) => (
                <div key={chunk.id} className="chunk-item">
                  <div className="chunk-meta">
                    <span>Segment {chunk.chunk_index}</span>
                    <span className="chunk-id">
                      {chunk.book_id.slice(0, 8)}
                    </span>
                  </div>
                  <p className="chunk-content">{chunk.content}</p>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default SearchMode;
