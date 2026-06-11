import { useEffect, useState } from "react";
import { Modal } from "./ui";
import { IconShield } from "./Icon";
import { EVENTS, on } from "@/services/events";
import * as api from "@/services/api";
import type { HostKeyPrompt, TofuResolution } from "@/types";

/**
 * Listens for unknown-host-key prompts emitted during an SSH handshake and asks
 * the user to make a Trust-On-First-Use decision. Prompts are queued so several
 * simultaneous connections each get answered.
 */
export function HostKeyPromptModal() {
  const [queue, setQueue] = useState<HostKeyPrompt[]>([]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    on<HostKeyPrompt>(EVENTS.hostKeyPrompt, (prompt) => {
      setQueue((q) => [...q, prompt]);
    }).then((fn) => (unlisten = fn));
    return () => unlisten?.();
  }, []);

  const current = queue[0];
  if (!current) return null;

  const respond = async (resolution: TofuResolution) => {
    try {
      await api.respondHostKey(current.request_id, resolution);
    } finally {
      setQueue((q) => q.slice(1));
    }
  };

  return (
    <Modal
      title="Verify host authenticity"
      onClose={() => respond("reject")}
      footer={
        <>
          <button className="btn btn--danger" onClick={() => respond("reject")}>
            Reject
          </button>
          <button className="btn" onClick={() => respond("trust_once")}>
            Trust once
          </button>
          <button className="btn btn--primary" onClick={() => respond("trust_and_save")}>
            Trust &amp; save
          </button>
        </>
      }
    >
      <div className="row" style={{ gap: 12, alignItems: "flex-start" }}>
        <div
          className="empty__icon"
          style={{ width: 40, height: 40, color: "var(--warning)" }}
        >
          <IconShield size={20} />
        </div>
        <div>
          The authenticity of host{" "}
          <strong>
            {current.host}:{current.port}
          </strong>{" "}
          can't be established. This is the first time Harbor has seen it.
        </div>
      </div>

      <div className="field">
        <label className="field__label">{current.algorithm} key fingerprint</label>
        <div className="fingerprint-box">{current.fingerprint}</div>
        <div className="field__hint">
          Verify this fingerprint matches the server out-of-band before trusting.
          Saving adds it to <span className="mono">~/.ssh/known_hosts</span>.
        </div>
      </div>
    </Modal>
  );
}

/**
 * Prompt for a password or key passphrase needed to (re)attempt a connection.
 */
export function SecretModal({
  title,
  label,
  onSubmit,
  onClose,
}: {
  title: string;
  label: string;
  onSubmit: (value: string, remember: boolean) => Promise<void>;
  onClose: () => void;
}) {
  const [value, setValue] = useState("");
  const [remember, setRemember] = useState(false);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!value) return;
    setBusy(true);
    try {
      await onSubmit(value, remember);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title={title}
      onClose={onClose}
      footer={
        <>
          <button className="btn btn--ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn btn--primary" onClick={submit} disabled={busy || !value}>
            {busy ? "Connecting…" : "Connect"}
          </button>
        </>
      }
    >
      <div className="field">
        <label className="field__label">{label}</label>
        <input
          className="input"
          type="password"
          autoFocus
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && submit()}
        />
      </div>
      <label className="row" style={{ gap: 8, cursor: "pointer" }}>
        <input
          type="checkbox"
          checked={remember}
          onChange={(e) => setRemember(e.target.checked)}
        />
        <span className="muted">Save in the OS keychain</span>
      </label>
    </Modal>
  );
}
