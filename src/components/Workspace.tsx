import { IconClose, IconServer } from "./Icon";
import { SessionWorkspace } from "./SessionWorkspace";
import { ConnectionsPage } from "@/pages/ConnectionsPage";
import { MANAGER_TAB, useSessions } from "@/stores/sessions";

function TabBar() {
  const order = useSessions((s) => s.order);
  const sessions = useSessions((s) => s.sessions);
  const activeTab = useSessions((s) => s.activeTab);
  const setActiveTab = useSessions((s) => s.setActiveTab);
  const close = useSessions((s) => s.close);

  return (
    <div className="tabbar">
      <button
        className={`tab ${activeTab === MANAGER_TAB ? "is-active" : ""}`}
        onClick={() => setActiveTab(MANAGER_TAB)}
      >
        <IconServer size={15} /> Connections
      </button>
      {order.map((id) => {
        const session = sessions[id];
        if (!session) return null;
        const state = session.info.status.state;
        return (
          <div
            key={id}
            className={`tab ${activeTab === id ? "is-active" : ""}`}
            onClick={() => setActiveTab(id)}
          >
            <span className={`status-dot ${state}`} />
            <span>{session.info.title}</span>
            <button
              className="tab__close"
              onClick={(e) => {
                e.stopPropagation();
                close(id);
              }}
              aria-label="Close session"
            >
              <IconClose size={13} />
            </button>
          </div>
        );
      })}
    </div>
  );
}

/**
 * The "Connections" view: a tab bar over the connection manager plus every open
 * session. All session panes stay mounted (hidden via CSS) so shells and SFTP
 * connections persist across tab and view switches.
 */
export function Workspace() {
  const order = useSessions((s) => s.order);
  const activeTab = useSessions((s) => s.activeTab);

  return (
    <div className="workspace">
      <TabBar />
      <div style={{ flex: 1, minHeight: 0, position: "relative" }}>
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: activeTab === MANAGER_TAB ? "flex" : "none",
          }}
        >
          <ConnectionsPage />
        </div>
        {order.map((id) => (
          <div
            key={id}
            style={{
              position: "absolute",
              inset: 0,
              display: activeTab === id ? "flex" : "none",
              flexDirection: "column",
            }}
          >
            <SessionWorkspace sessionId={id} active={activeTab === id} />
          </div>
        ))}
      </div>
    </div>
  );
}
