import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

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
  return <button onClick={uploadBook} className="bg-blue-500 hover:bg-blue-600 text-white font-bold py-2 px-4 rounded">Upload PDF</button>;
}