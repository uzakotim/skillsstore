import React from "react";
import { UploadIcon, DownloadIcon } from "lucide-react";
import { AppMode, Book } from "@/types";

export interface HeaderProps {
  mode: AppMode;
  selectedBook?: Book;
  consoleMsg: string;
  onSaveDatabase: () => void;
  onLoadDatabase: () => void;
  onDebugChunks: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  mode,
  selectedBook,
  consoleMsg,
  onSaveDatabase,
  onLoadDatabase,
  onDebugChunks,
}) => {
  return (
    <header className="app-topbar">
      <div className="topbar-left">
        <h1 className="topbar-title">
          {mode === "search" ? "Search & Ask" : "Learn Mode"}
          {selectedBook && (
            <span className="topbar-book"> · {selectedBook.title}</span>
          )}
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
          onClick={onSaveDatabase}
          title="Save database"
        >
          <UploadIcon className="w-4 h-4" />
          Save database
        </button>
        <button
          className="debug-btn"
          onClick={onLoadDatabase}
          title="Load database"
        >
          <DownloadIcon className="w-4 h-4" />
          Load database
        </button>
        <button
          className="debug-btn"
          onClick={onDebugChunks}
          title="Inspect database chunks"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
            <line x1="8" y1="11" x2="14" y2="11" />
            <line x1="11" y1="8" x2="11" y2="14" />
          </svg>
          Debug Chunks
        </button>
      </div>
    </header>
  );
};

export default Header;
