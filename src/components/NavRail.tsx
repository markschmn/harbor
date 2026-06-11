import {
  IconAnchor,
  IconKey,
  IconMoon,
  IconServer,
  IconSettings,
  IconSun,
  IconTransfers,
} from "./Icon";
import { useUi, type View } from "@/stores/ui";
import { useTransfers, activeTransferCount } from "@/stores/transfers";

const ITEMS: { view: View; label: string; icon: typeof IconServer }[] = [
  { view: "connections", label: "Connections", icon: IconServer },
  { view: "transfers", label: "Transfers", icon: IconTransfers },
  { view: "keys", label: "Keys", icon: IconKey },
  { view: "settings", label: "Settings", icon: IconSettings },
];

export function NavRail() {
  const view = useUi((s) => s.view);
  const setView = useUi((s) => s.setView);
  const theme = useUi((s) => s.theme);
  const toggleTheme = useUi((s) => s.toggleTheme);
  const transferBadge = useTransfers(activeTransferCount);

  return (
    <nav className="nav-rail">
      <div className="nav-rail__brand" title="Harbor">
        <IconAnchor size={20} />
      </div>

      {ITEMS.map(({ view: v, label, icon: Icon }) => (
        <button
          key={v}
          className={`nav-btn ${view === v ? "is-active" : ""}`}
          onClick={() => setView(v)}
          title={label}
          aria-label={label}
        >
          <Icon />
          {v === "transfers" && transferBadge > 0 && (
            <span className="nav-rail__badge nav-btn__badge">{transferBadge}</span>
          )}
        </button>
      ))}

      <div className="nav-rail__spacer" />

      <button
        className="nav-btn"
        onClick={toggleTheme}
        title="Toggle theme"
        aria-label="Toggle theme"
      >
        {theme === "dark" ? <IconSun /> : <IconMoon />}
      </button>
    </nav>
  );
}
