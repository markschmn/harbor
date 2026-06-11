import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { IconKey, IconLock, IconPlus, IconRefresh } from "@/components/Icon";
import { EmptyState } from "@/components/ui";
import * as api from "@/services/api";
import { errorMessage, toast } from "@/stores/toast";
import type { DiscoveredKey } from "@/types";

export function KeysPage() {
  const [keys, setKeys] = useState<DiscoveredKey[]>([]);
  const [loading, setLoading] = useState(true);

  const load = async () => {
    setLoading(true);
    try {
      setKeys(await api.listKeys());
    } catch (e) {
      toast("error", errorMessage(e), "Could not list keys");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const inspectExternal = async () => {
    try {
      const selected = await open({ multiple: false, directory: false });
      if (typeof selected !== "string") return;
      const key = await api.inspectKey(selected);
      setKeys((prev) =>
        prev.some((k) => k.private_key_path === key.private_key_path)
          ? prev
          : [...prev, key],
      );
      toast("success", "Key inspected");
    } catch (e) {
      toast("error", errorMessage(e), "Not a valid key");
    }
  };

  return (
    <div className="panel">
      <div className="panel__header">
        <div>
          <div className="panel__title">SSH Keys</div>
          <div className="panel__subtitle">
            Keys discovered in <span className="mono">~/.ssh</span>
          </div>
        </div>
        <div className="row" style={{ gap: 8 }}>
          <button className="btn" onClick={inspectExternal}>
            <IconPlus size={15} /> Inspect a key…
          </button>
          <button className="btn btn--icon btn--ghost" onClick={load} title="Refresh">
            <IconRefresh size={16} />
          </button>
        </div>
      </div>
      <div className="panel__body">
        {loading ? (
          <div className="empty">
            <div className="spinner" />
          </div>
        ) : keys.length === 0 ? (
          <EmptyState
            icon={<IconKey />}
            title="No keys found"
            hint="Harbor looks in ~/.ssh. You can also inspect a key from anywhere."
          />
        ) : (
          keys.map((key) => (
            <div className="key-card" key={key.private_key_path}>
              <div className="key-card__icon">
                <IconKey size={18} />
              </div>
              <div className="key-card__main">
                <div className="row" style={{ gap: 8 }}>
                  <span style={{ fontWeight: 600 }}>
                    {key.private_key_path.split(/[\\/]/).pop()}
                  </span>
                  <span className="badge badge--accent">{key.algorithm}</span>
                  {key.bits && <span className="badge">{key.bits}-bit</span>}
                  {key.encrypted && (
                    <span className="badge badge--warning">
                      <IconLock size={11} /> Encrypted
                    </span>
                  )}
                </div>
                <div className="key-card__fp" title={key.fingerprint}>
                  {key.fingerprint}
                  {key.comment ? `  ·  ${key.comment}` : ""}
                </div>
                <div className="faint mono" style={{ fontSize: 12, marginTop: 2 }}>
                  {key.private_key_path}
                </div>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
