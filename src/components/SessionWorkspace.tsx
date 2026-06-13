import { TerminalPane } from "./TerminalPane";
import { FileBrowser } from "./FileBrowser";
import { MetricsPane } from "./MetricsPane";
import { IconActivity, IconFolder, IconTerminal } from "./Icon";
import { useSessions } from "@/stores/sessions";

/**
 * One connected session: a persistent terminal plus the dual-pane file browser,
 * toggled with a segmented control. Both panes stay mounted so the shell and
 * SFTP session survive switching between them.
 */
export function SessionWorkspace({
  sessionId,
  active,
}: {
  sessionId: string;
  active: boolean;
}) {
  const session = useSessions((s) => s.sessions[sessionId]);
  const setPanel = useSessions((s) => s.setPanel);
  if (!session) return null;

  const panel = session.panel;

  return (
    <div className="session-view">
      <div className="session-toolbar">
        <div className="segmented">
          <button
            className={panel === "terminal" ? "is-active" : ""}
            onClick={() => setPanel(sessionId, "terminal")}
          >
            <span className="row" style={{ gap: 6 }}>
              <IconTerminal size={14} /> Terminal
            </span>
          </button>
          <button
            className={panel === "files" ? "is-active" : ""}
            onClick={() => setPanel(sessionId, "files")}
          >
            <span className="row" style={{ gap: 6 }}>
              <IconFolder size={14} /> Files
            </span>
          </button>
          <button
            className={panel === "metrics" ? "is-active" : ""}
            onClick={() => setPanel(sessionId, "metrics")}
          >
            <span className="row" style={{ gap: 6 }}>
              <IconActivity size={14} /> Metrics
            </span>
          </button>
        </div>
        <div className="spacer" />
        <span className="badge badge--accent">{session.info.title}</span>
      </div>

      <div style={{ flex: 1, minHeight: 0, minWidth: 0, display: "flex" }}>
        <div
          style={{
            flex: 1,
            // BOTH min-* must be 0: otherwise the flex item's min size is its
            // content size, and xterm's fit() then grows it unbounded — a
            // resize-storm feedback loop that breaks full-screen apps.
            minHeight: 0,
            minWidth: 0,
            display: panel === "terminal" ? "flex" : "none",
            flexDirection: "column",
          }}
        >
          <TerminalPane sessionId={sessionId} active={active && panel === "terminal"} />
        </div>
        <div
          style={{
            flex: 1,
            minHeight: 0,
            minWidth: 0,
            display: panel === "files" ? "flex" : "none",
            flexDirection: "column",
          }}
        >
          <FileBrowser sessionId={sessionId} active={active && panel === "files"} />
        </div>
        <div
          style={{
            flex: 1,
            minHeight: 0,
            minWidth: 0,
            display: panel === "metrics" ? "flex" : "none",
            flexDirection: "column",
          }}
        >
          <MetricsPane sessionId={sessionId} active={active && panel === "metrics"} />
        </div>
      </div>
    </div>
  );
}
