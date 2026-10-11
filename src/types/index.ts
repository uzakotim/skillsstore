export interface Chunk {
  id: number;
  book_id: string;
  chunk_index: number;
  content: string;
}

export interface Book {
  id: string;
  title: string;
}

export interface ModelConfig {
  llm_model: string;
  embed_model: string;
}

export interface ConceptItem {
  id: string;
  title: string;
  description: string;
  excerpts: string[];
}

export interface AnalysisProgress {
  status: string;
  processed?: number;
  total?: number;
  percent?: number | null;
}

export type LearnTab = "concepts" | "excerpts" | "lesson";
export type AppMode = "search" | "learn";
export type OllamaStatus = "checking" | "online" | "offline";
