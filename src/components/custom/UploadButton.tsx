import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { useSetAtom } from "jotai";
import { consoleMsgAtom } from "@/store/atoms";

interface UploadButtonProps {
  onUpload?: () => void;
}

export default function UploadButton({ onUpload }: UploadButtonProps) {
  const setConsoleMsg = useSetAtom(consoleMsgAtom);

  async function handleUpload() {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });

      if (!selected) return;

      await invoke("upload_pdf", { path: selected });
      setConsoleMsg("Book uploaded and indexed!");
      if (onUpload) onUpload();
    } catch (error) {
      console.error("Failed to upload book:", error);
      setConsoleMsg(`Error: ${error}`);
    }
  }

  return <Button onClick={handleUpload}>Upload PDF</Button>;
}