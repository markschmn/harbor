import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles/global.css";
import "./styles/components.css";

// Note: React StrictMode is intentionally omitted. Its double-invocation of
// effects in development would open two shells / SFTP sessions per terminal.
createRoot(document.getElementById("root")!).render(<App />);
