import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useSetAtom } from "jotai";
import { consoleMsgAtom } from "@/store/atoms";

interface UploadButtonProps {
  onUploadStart?: () => void;
  onUpload?: (bookId?: string) => void;
}

export default function UploadButton({ onUpload, onUploadStart }: UploadButtonProps) {
  const setConsoleMsg = useSetAtom(consoleMsgAtom);

  async function handleUpload() {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });

      if (!selected) return;
      onUploadStart?.();
      setConsoleMsg("Placing book on shelf... this may take a moment.");
      await invoke("upload_pdf", { path: selected });
      setConsoleMsg("Book placed on your shelf!");
      onUpload?.();
    } catch (error) {
      console.error("Failed to upload book:", error);
      setConsoleMsg(`Error: ${error}`);
      onUpload?.();
    }
  }

  return (
    <button className="upload-book-btn" onClick={handleUpload}>
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20"/>
        <line x1="12" y1="9" x2="12" y2="15"/><line x1="9" y1="12" x2="15" y2="12"/>
      </svg>
      Add Book
    </button>
  );
}