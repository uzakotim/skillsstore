import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useSetAtom } from "jotai";
import { consoleMsgAtom } from "@/store/atoms";
import { Book, BookA, BookCopy } from "lucide-react";

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
      <BookCopy className="w-4 h-4" />
      Add Book
    </button>
  );
}