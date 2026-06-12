import { useState } from "react";
import {
  IconAnchor,
  IconDownload,
  IconLock,
  IconRefresh,
  IconShield,
  IconTerminal,
} from "@/components/Icon";
import { Switch } from "@/components/ui";
import { RemovePinModal, SetPinModal } from "@/components/PinDialogs";
import { useUi } from "@/stores/ui";
import { useUpdates } from "@/stores/updates";
import { useLock } from "@/stores/lock";

function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <span
      className="mono"
      style={{
        padding: "2px 7px",
        borderRadius: 6,
        background: "var(--bg-elev-2)",
        border: "1px solid var(--border)",
        fontSize: 11,
      }}
    >
      {children}
    </span>
  );
}

export function SettingsPage() {
  const { theme, setTheme, appInfo } = useUi();
  const checking = useUpdates((s) => s.checking);
  const check = useUpdates((s) => s.check);
  const hasPin = useLock((s) => s.hasPin);
  const [pinDialog, setPinDialog] = useState<"set" | "change" | "remove" | null>(null);
  const mod = appInfo?.platform === "macos" ? "⌘" : "Ctrl";

  const shortcuts: [string, string][] = [
    [`${mod} + 1 … 4`, "Switch between Connections / Transfers / Keys / Settings"],
    [`${mod} + W`, "Close the active session tab"],
  ];

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
          <div className="row row--between">
            <div className="row" style={{ gap: 10 }}>
              <IconLock size={18} className="muted" />
              <div>
                <div style={{ fontWeight: 600 }}>App lock (PIN)</div>
                <div className="faint" style={{ fontSize: 12 }}>
                  {hasPin
                    ? "Harbor asks for your PIN each time it launches."
                    : "Require a PIN to open Harbor."}
                </div>
              </div>
            </div>
            {hasPin ? (
              <div className="row" style={{ gap: 8 }}>
                <button className="btn btn--sm" onClick={() => setPinDialog("change")}>
                  Change
                </button>
                <button
                  className="btn btn--sm btn--danger"
                  onClick={() => setPinDialog("remove")}
                >
                  Remove
                </button>
              </div>
            ) : (
              <button className="btn btn--sm btn--primary" onClick={() => setPinDialog("set")}>
                Set up PIN
              </button>
            )}
          </div>
        </div>

        <div className="detail-card" style={{ marginTop: 16 }}>
          <div className="row row--between" style={{ marginBottom: 4 }}>
            <div className="row" style={{ gap: 10 }}>
              <IconDownload size={18} className="muted" />
              <div style={{ fontWeight: 600 }}>Software update</div>
            </div>
            <button
              className="btn btn--sm"
              onClick={() => check(false)}
              disabled={checking}
            >
              {checking ? <span className="spinner" /> : <IconRefresh size={14} />}
              {checking ? "Checking…" : "Check for updates"}
            </button>
          </div>
          <div className="faint" style={{ fontSize: 12 }}>
            Harbor checks for updates automatically on launch and installs them
            with one click. Currently on v{appInfo?.version ?? "—"}.
          </div>
        </div>

        <div className="detail-card" style={{ marginTop: 16 }}>
          <div className="row" style={{ gap: 10, marginBottom: 12 }}>
            <IconTerminal size={18} className="muted" />
            <div style={{ fontWeight: 600 }}>Keyboard shortcuts</div>
          </div>
          {shortcuts.map(([keys, desc]) => (
            <div className="detail-row" key={keys}>
              <span className="detail-row__key">{desc}</span>
              <span className="detail-row__val">
                <Kbd>{keys}</Kbd>
              </span>
            </div>
          ))}
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

      {(pinDialog === "set" || pinDialog === "change") && (
        <SetPinModal change={pinDialog === "change"} onClose={() => setPinDialog(null)} />
      )}
      {pinDialog === "remove" && <RemovePinModal onClose={() => setPinDialog(null)} />}
    </div>
  );
}
