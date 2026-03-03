import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import UploadButton from "./components/custom/UploadButton";

function App() {
  const [greetMsg, setGreetMsg] = useState("HOLA");
  const [name, setName] = useState("");

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }

  return (

      <div className="flex flex-col gap-4 h-screen w-screen items-center justify-center">
        <UploadButton />
        <p className="text-red-500">{greetMsg}</p>
      </div>
  );
}

export default App;
