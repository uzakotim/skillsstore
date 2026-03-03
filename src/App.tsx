import "./App.css";
import UploadButton from "./components/custom/UploadButton";
import { useAtom } from "jotai";
import { consoleMsgAtom } from "./store/atoms";

function App() {
  const [consoleMsg, _setConsoleMsg] = useAtom(consoleMsgAtom);

  return (

      <div className="flex flex-col gap-4 h-screen w-screen items-center justify-center">
        <UploadButton />
        <p className="text-red-500">{consoleMsg}</p>
      </div>
  );
}

export default App;
