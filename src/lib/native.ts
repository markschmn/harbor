// Make the webview feel like a native desktop app rather than a web page.
//
// Suppresses the browser right-click context menu (Print / Reload / Back / view
// source / Inspect…) and the default browser keyboard shortcuts that have no
// place in a desktop app. Editable fields keep their native copy/paste menu.
//
// Only applied in production builds so DevTools and reload stay available while
// developing (`npm run tauri dev`).

function isEditable(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return !!el?.closest?.("input, textarea, [contenteditable='true']");
}

export function installNativeBehaviors(): void {
  if (import.meta.env.DEV) return;

  // No browser context menu, except inside text fields.
  window.addEventListener(
    "contextmenu",
    (e) => {
      if (!isEditable(e.target)) e.preventDefault();
    },
    { capture: true },
  );

  // Swallow browser-level shortcuts (reload, print, find, view-source, zoom).
  window.addEventListener(
    "keydown",
    (e) => {
      const key = e.key.toLowerCase();
      const mod = e.ctrlKey || e.metaKey;

      if (key === "f5" || (mod && key === "r")) return e.preventDefault();
      if (mod && key === "p") return e.preventDefault();
      if (mod && e.shiftKey && key === "u") return e.preventDefault();
      if (mod && (key === "+" || key === "-" || key === "=" || key === "0"))
        return e.preventDefault();
      // Browser find — leave inputs/terminals alone.
      if (mod && key === "f" && !isEditable(e.target)) return e.preventDefault();
    },
    { capture: true },
  );

  // Dropping a file onto empty chrome shouldn't navigate the webview to it.
  // (The file browser's own drop zones call stopPropagation, so they're safe.)
  const stop = (e: DragEvent) => {
    if (!(e.target as HTMLElement)?.closest?.(".file-list")) e.preventDefault();
  };
  window.addEventListener("dragover", stop);
  window.addEventListener("drop", stop);
}
