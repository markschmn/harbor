import { useEffect, useRef, useState } from "react";
import { IconAnchor, IconLock } from "./Icon";
import * as api from "@/services/api";
import { useLock } from "@/stores/lock";

/** Full-screen PIN gate shown on launch when an app PIN is configured. */
export function LockScreen() {
  const locked = useLock((s) => s.locked);
  const unlock = useLock((s) => s.unlock);
  const [pin, setPin] = useState("");
  const [error, setError] = useState(false);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (locked) inputRef.current?.focus();
  }, [locked]);

  if (!locked) return null;

  const submit = async () => {
    if (pin.length < 4 || busy) return;
    setBusy(true);
    try {
      if (await api.verifyAppPin(pin)) {
        setPin("");
        unlock();
      } else {
        setError(true);
        setPin("");
        setTimeout(() => setError(false), 600);
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="lock-screen">
      <div className={`lock-card ${error ? "shake" : ""}`}>
        <div className="lock-logo">
          <IconAnchor size={30} />
        </div>
        <div className="lock-title">Harbor is locked</div>
        <div className="muted" style={{ fontSize: 13 }}>
          Enter your PIN to continue
        </div>
        <input
          ref={inputRef}
          className={`input lock-pin ${error ? "is-error" : ""}`}
          type="password"
          inputMode="numeric"
          autoComplete="off"
          value={pin}
          onChange={(e) => setPin(e.target.value.replace(/\D/g, "").slice(0, 12))}
          onKeyDown={(e) => e.key === "Enter" && submit()}
          aria-label="PIN"
        />
        <button
          className="btn btn--primary"
          style={{ width: "100%" }}
          onClick={submit}
          disabled={busy || pin.length < 4}
        >
          <IconLock size={15} /> Unlock
        </button>
        {error && <div className="badge badge--danger">Incorrect PIN</div>}
      </div>
    </div>
  );
}
