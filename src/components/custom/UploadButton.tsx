import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";

async function uploadBook() {
  const selected = await open({
    multiple: false,
    filters: [{ name: "PDF", extensions: ["pdf"] }],
  });

  if (!selected) return;

  await invoke("upload_pdf", { path: selected });
  alert("Book uploaded and indexed!");
}

export default function UploadButton() {
  return <Button onClick={uploadBook}>Upload PDF</Button>;
}