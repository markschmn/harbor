import { IconAnchor, IconLock, IconShield } from "@/components/Icon";
import { Switch } from "@/components/ui";
import { useUi } from "@/stores/ui";

export function SettingsPage() {
  const { theme, setTheme, appInfo } = useUi();

  return (
    <div className="panel">
      <div className="panel__header">
        <div>
          <div className="panel__title">Settings</div>
          <div className="panel__subtitle">Appearance, security and about</div>
        </div>
      </div>
      <div className="panel__body" style={{ maxWidth: 680 }}>
        <div className="detail-card">
          <div className="row row--between" style={{ padding: "8px 0" }}>
            <div>
              <div style={{ fontWeight: 600 }}>Dark mode</div>
              <div className="faint" style={{ fontSize: 12 }}>
                Switch between the dark and light themes.
              </div>
            </div>
            <Switch
              on={theme === "dark"}
              onChange={(on) => setTheme(on ? "dark" : "light")}
            />
          </div>
        </div>

        <div className="detail-card" style={{ marginTop: 16 }}>
          <div className="row" style={{ gap: 10, marginBottom: 12 }}>
            <IconShield size={18} className="muted" />
            <div style={{ fontWeight: 600 }}>Security</div>
          </div>
          <div className="detail-row">
            <span className="detail-row__key">Host key verification</span>
            <span className="detail-row__val">
              <span className="badge badge--success">Always on</span>
            </span>
          </div>
          <div className="detail-row">
            <span className="detail-row__key">known_hosts</span>
            <span className="detail-row__val">~/.ssh/known_hosts</span>
          </div>
          <div className="detail-row">
            <span className="detail-row__key">Secret storage</span>
            <span className="detail-row__val row" style={{ gap: 6, justifyContent: "flex-end" }}>
              <IconLock size={14} />
              {appInfo?.keychain_persistent ? "OS keychain" : "Session only"}
            </span>
          </div>
        </div>

        <div className="detail-card" style={{ marginTop: 16 }}>
          <div className="row" style={{ gap: 12 }}>
            <div className="nav-rail__brand" style={{ margin: 0 }}>
              <IconAnchor size={20} />
            </div>
            <div>
              <div style={{ fontWeight: 650, fontSize: 16 }}>Harbor</div>
              <div className="muted" style={{ fontSize: 13 }}>
                A modern, secure SSH client &amp; SFTP file manager
              </div>
              <div className="faint mono" style={{ fontSize: 12, marginTop: 4 }}>
                v{appInfo?.version ?? "—"} · {appInfo?.platform ?? "—"}
              </div>
            </div>
          </div>
        </div>

        {!appInfo?.keychain_persistent && (
          <div className="callout callout--warning" style={{ marginTop: 16 }}>
            No OS keychain is reachable in this environment, so saved passwords
            are kept only for the current session and are never written to disk.
          </div>
        )}
      </div>
    </div>
  );
}
