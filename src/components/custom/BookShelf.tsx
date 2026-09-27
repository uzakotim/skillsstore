import { useState } from "react";

interface Book {
  id: string;
  title: string;
}

// Generate a deterministic color from a string
function colorFromTitle(title: string): { spine: string; cover: string; accent: string } {
  let hash = 0;
  for (let i = 0; i < title.length; i++) {
    hash = title.charCodeAt(i) + ((hash << 5) - hash);
  }
  const palettes = [
    { spine: "#2D4A7A", cover: "#3A5F99", accent: "#7EAADC" },
    { spine: "#5A2D82", cover: "#7B3FAA", accent: "#B47FD4" },
    { spine: "#1A6B4A", cover: "#2A8A60", accent: "#6FC4A0" },
    { spine: "#7A2D2D", cover: "#993A3A", accent: "#DC7E7E" },
    { spine: "#6B4A1A", cover: "#8A6020", accent: "#C4A060" },
    { spine: "#2D6B6B", cover: "#3A8A8A", accent: "#7EC4C4" },
    { spine: "#4A1A6B", cover: "#602A8A", accent: "#A060C4" },
    { spine: "#1A4A6B", cover: "#205F8A", accent: "#60A0C4" },
  ];
  return palettes[Math.abs(hash) % palettes.length];
}

function BookSpine({
  book,
  isSelected,
  onClick,
  isNew = false,
}: {
  book: Book;
  isSelected: boolean;
  onClick: () => void;
  isNew?: boolean;
}) {
  const [hovered, setHovered] = useState(false);
  const colors = colorFromTitle(book.title);
  const shortTitle = book.title.length > 18 ? book.title.slice(0, 18) + "…" : book.title;

  return (
    <div
      className={`book-spine-wrapper ${isSelected ? "selected" : ""} ${isNew ? "book-new" : ""}`}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onClick={onClick}
      title={book.title}
    >
      <div
        className="book-spine"
        style={{
          background: `linear-gradient(to right, ${colors.spine}, ${colors.cover})`,
          boxShadow: isSelected
            ? `0 -8px 24px ${colors.accent}66, 0 4px 12px rgba(0,0,0,0.3)`
            : hovered
            ? `0 -6px 16px ${colors.accent}44, 0 2px 8px rgba(0,0,0,0.2)`
            : "0 2px 4px rgba(0,0,0,0.15)",
          transform: isSelected ? "translateY(-10px) scaleY(1.03)" : hovered ? "translateY(-6px)" : "translateY(0)",
        }}
      >
        {/* Spine highlight */}
        <div className="spine-highlight" />
        {/* Title text */}
        <span className="spine-title" style={{ color: colors.accent }}>
          {shortTitle}
        </span>
        {/* Bottom decoration */}
        <div className="spine-bottom" style={{ background: colors.spine }} />
      </div>
      {/* Shadow under book */}
      <div
        className="book-shadow"
        style={{ opacity: isSelected ? 0.5 : hovered ? 0.35 : 0.2 }}
      />
    </div>
  );
}

interface BookShelfProps {
  books: Book[];
  selectedBookId: string;
  onSelectBook: (id: string) => void;
  onDeleteBook: () => void;
  isUploading?: boolean;
  newBookId?: string | null;
}

export default function BookShelf({
  books,
  selectedBookId,
  onSelectBook,
  onDeleteBook,
  isUploading = false,
  newBookId = null,
}: BookShelfProps) {
  return (
    <div className="bookshelf-container">
      <div className="bookshelf-label">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20"/>
        </svg>
        Your Library
        <span className="bookshelf-count">{books.length}</span>
      </div>

      <div className="bookshelf">
        {/* Shelf wood */}
        <div className="shelf-surface">
          {/* Books */}
          <div className="books-row">
            {books.length === 0 && !isUploading && (
              <div className="empty-shelf-msg">
                <span>Upload a PDF to place it on your shelf →</span>
              </div>
            )}

            {books.map((book) => (
              <BookSpine
                key={book.id}
                book={book}
                isSelected={selectedBookId === book.id}
                onClick={() => onSelectBook(selectedBookId === book.id ? "" : book.id)}
                isNew={newBookId === book.id}
              />
            ))}

            {/* Ghost book while uploading */}
            {isUploading && (
              <div className="book-uploading-ghost">
                <div className="ghost-spine">
                  <div className="ghost-shimmer" />
                </div>
                <div className="ghost-label">Indexing...</div>
              </div>
            )}
          </div>
          <div className="shelf-edge" />
        </div>
      </div>

      {/* Selected book actions */}
      {selectedBookId && (
        <div className="selected-book-actions">
          <span className="selected-book-name">
            📖 {books.find(b => b.id === selectedBookId)?.title}
          </span>
          <button className="deselect-book-btn" onClick={() => onSelectBook("")} title="View all books">
            All books
          </button>
          <button className="delete-book-btn" onClick={onDeleteBook} title="Remove book from shelf">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/>
              <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
            </svg>
            Remove
          </button>
        </div>
      )}
    </div>
  );
}
