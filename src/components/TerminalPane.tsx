import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import {
  readText as readClipboard,
  writeText as writeClipboard,
} from "@tauri-apps/plugin-clipboard-manager";
import * as api from "@/services/api";
import { EVENTS, on } from "@/services/events";
import { errorMessage } from "@/stores/toast";
import type { TerminalClosed, TerminalData } from "@/types";

const IS_MAC = navigator.userAgent.includes("Mac");

const TERMINAL_THEME = {
  background: "#0a0e14",
  foreground: "#cdd6e3",
  cursor: "#5b8def",
  cursorAccent: "#0a0e14",
  selectionBackground: "rgba(91,141,239,0.3)",
  black: "#11151c",
  red: "#ff5a5f",
  green: "#34c759",
  yellow: "#ffb020",
  blue: "#5b8def",
  magenta: "#bd93f9",
  cyan: "#56c2d6",
  white: "#cdd6e3",
  brightBlack: "#5b6673",
  brightRed: "#ff7a7f",
  brightGreen: "#5cd97a",
  brightYellow: "#ffc04d",
  brightBlue: "#7da6f5",
  brightMagenta: "#d0a8ff",
  brightCyan: "#7ad4e6",
  brightWhite: "#ffffff",
};

function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

export function TerminalPane({
  sessionId,
  active,
}: {
  sessionId: string;
  active: boolean;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const term = new Terminal({
      fontFamily:
        '"JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace',
      fontSize: 13,
      lineHeight: 1.0,
      cursorBlink: true,
      scrollback: 8000,
      theme: TERMINAL_THEME,
      allowProposedApi: true,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon());
    term.open(host);
    termRef.current = term;
    fitRef.current = fit;

    // --- Universal copy/paste -------------------------------------------
    // Works regardless of what runs inside the shell (tmux, vim, less …),
    // bridging the OS clipboard so text can move in and out of Harbor.
    //
    //   • Copy : Ctrl+Shift+C / ⌘C, or right-click while text is selected.
    //   • Paste: Ctrl+Shift+V / ⌘V, middle-click, or right-click (no
    //            selection). Routed through term.paste() so bracketed-paste
    //            mode keeps multi-line input from auto-running in tmux/vim.
    //
    // Selecting text while an app grabs the mouse (e.g. tmux mouse mode):
    // hold Shift while dragging — xterm then makes a local selection instead
    // of forwarding the drag to the remote program.
    const copySelection = (): boolean => {
      const selection = term.getSelection();
      if (!selection) return false;
      writeClipboard(selection).catch(() => {});
      return true;
    };
    const pasteClipboard = () => {
      readClipboard()
        .then((text) => {
          if (text) term.paste(text);
        })
        .catch(() => {});
    };

    // OSC 52 — programs inside the shell set the system clipboard by emitting
    // ESC ] 52 ; c ; <base64> BEL. This is how tmux (with `set-clipboard on`)
    // and vim copy out: a mouse-drag or yank in tmux fires OSC 52 rather than
    // creating an xterm selection, so without this handler "copy from tmux"
    // silently does nothing. xterm has no built-in handler, so bridge it to
    // the OS clipboard. Clipboard *reads* (payload "?") are refused so a remote
    // host can never exfiltrate the local clipboard.
    const oscSub = term.parser.registerOscHandler(52, (data) => {
      const sep = data.indexOf(";");
      const payload = sep === -1 ? data : data.slice(sep + 1);
      if (!payload || payload === "?") return true;
      try {
        const text = new TextDecoder().decode(base64ToBytes(payload));
        if (text) writeClipboard(text).catch(() => {});
      } catch {
        /* malformed base64 — ignore */
      }
      return true;
    });

    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown") return true;
      const primary = IS_MAC ? e.metaKey && !e.ctrlKey : e.ctrlKey && e.shiftKey;
      if (!primary) return true;
      if (e.code === "KeyC") {
        // Always swallow the copy chord so Ctrl+Shift+C can never fall
        // through to xterm and be sent as a bare Ctrl+C (SIGINT).
        e.preventDefault();
        copySelection();
        return false;
      }
      if (e.code === "KeyV") {
        // preventDefault stops the webview firing its own native paste event
        // into xterm's textarea, which would double up with term.paste().
        e.preventDefault();
        pasteClipboard();
        return false;
      }
      return true;
    });

    // Mouse paste/copy. Capture phase + stopPropagation so these never reach
    // xterm's own mouse handling or the remote app's mouse reporting.
    const onMouseDown = (e: MouseEvent) => {
      if (e.button === 1) {
        // Middle-click → paste (X11 convention).
        e.preventDefault();
        e.stopPropagation();
        pasteClipboard();
      } else if (e.button === 2) {
        // Right-click → copy a selection if present, otherwise paste.
        e.preventDefault();
        e.stopPropagation();
        if (copySelection()) term.clearSelection();
        else pasteClipboard();
      }
    };
    const onContextMenu = (e: MouseEvent) => e.preventDefault();
    host.addEventListener("mousedown", onMouseDown, true);
    host.addEventListener("contextmenu", onContextMenu);

    const safeFit = () => {
      if (host.clientHeight > 1 && host.clientWidth > 1) {
        try {
          fit.fit();
        } catch {
          /* ignore */
        }
      }
    };

    // Forward keystrokes to the remote shell.
    const dataSub = term.onData((data) => {
      api.sendInput(sessionId, data).catch(() => {});
    });
    // Inform the backend of terminal size changes (incl. the initial fit).
    const resizeSub = term.onResize(({ cols, rows }) => {
      api.resizeTerminal(sessionId, cols, rows).catch(() => {});
    });

    // Receive output for this session.
    const unlisteners: Array<() => void> = [];
    on<TerminalData>(EVENTS.terminalData, (payload) => {
      if (payload.session_id === sessionId) {
        term.write(base64ToBytes(payload.data));
      }
    }).then((fn) => unlisteners.push(fn));
    on<TerminalClosed>(EVENTS.terminalClosed, (payload) => {
      if (payload.session_id === sessionId) {
        const code = payload.exit_code ?? 0;
        term.write(`\r\n\x1b[90m[session closed${code ? ` · exit ${code}` : ""}]\x1b[0m\r\n`);
      }
    }).then((fn) => unlisteners.push(fn));

    // Keep the PTY sized to the element on any layout change. Coalesce rapid
    // observer fires into one fit per frame so a transient layout can never
    // turn into a resize storm.
    let rafId = 0;
    const ro = new ResizeObserver(() => {
      if (rafId) return;
      rafId = requestAnimationFrame(() => {
        rafId = 0;
        safeFit();
      });
    });
    ro.observe(host);

    // Open the shell only once the terminal has a real, font-measured size,
    // otherwise full-screen apps (nano, vim, htop) receive wrong dimensions
    // and the bottom row gets clipped. Wait for fonts + a non-zero layout.
    let disposed = false;
    (async () => {
      try {
        await document.fonts.ready;
      } catch {
        /* no web fonts to wait on */
      }
      for (let i = 0; i < 60 && !disposed; i++) {
        if (host.clientHeight > 1 && host.clientWidth > 1) break;
        await new Promise((r) => requestAnimationFrame(() => r(undefined)));
      }
      if (disposed) return;

      safeFit();
      if (term.rows < 2 || term.cols < 2) {
        term.resize(Math.max(term.cols, 80), Math.max(term.rows, 24));
      }
      try {
        await api.openShell(sessionId, term.cols, term.rows);
      } catch (e) {
        term.write(`\r\n\x1b[31mFailed to open shell: ${errorMessage(e)}\x1b[0m\r\n`);
        return;
      }
      // Correct any late layout/sub-pixel changes after the shell is up.
      window.setTimeout(() => !disposed && safeFit(), 120);
      window.setTimeout(() => !disposed && safeFit(), 400);
    })();

    return () => {
      disposed = true;
      if (rafId) cancelAnimationFrame(rafId);
      host.removeEventListener("mousedown", onMouseDown, true);
      host.removeEventListener("contextmenu", onContextMenu);
      oscSub.dispose();
      dataSub.dispose();
      resizeSub.dispose();
      ro.disconnect();
      unlisteners.forEach((fn) => fn());
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, [sessionId]);

  // When this pane becomes visible, refit and focus.
  useEffect(() => {
    if (!active) return;
    const id = window.setTimeout(() => {
      const host = hostRef.current;
      try {
        if (host && host.clientHeight > 1) fitRef.current?.fit();
        termRef.current?.focus();
      } catch {
        /* ignore */
      }
    }, 50);
    return () => window.clearTimeout(id);
  }, [active]);

  return <div className="terminal-host" ref={hostRef} />;
}
