import { useState } from "react";
import { Modal } from "./ui";
import * as api from "@/services/api";
import { errorMessage, toast } from "@/stores/toast";
import { useLock } from "@/stores/lock";

const pinInput = {
  type: "password" as const,
  inputMode: "numeric" as const,
  autoComplete: "off",
  maxLength: 12,
};

function clean(v: string) {
  return v.replace(/\D/g, "").slice(0, 12);
}

/** Set or change the app PIN (enter + confirm). */
export function SetPinModal({
  change,
  onClose,
}: {
  change?: boolean;
  onClose: () => void;
}) {
  const setHasPin = useLock((s) => s.setHasPin);
  const [pin, setPin] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const valid = pin.length >= 4 && pin.length <= 12 && pin === confirm;

  const submit = async () => {
    if (!valid || busy) return;
    setBusy(true);
    try {
      await api.setAppPin(pin);
      setHasPin(true);
      toast("success", change ? "PIN changed" : "App lock enabled");
      onClose();
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title={change ? "Change PIN" : "Set a PIN"}
      onClose={onClose}
      footer={
        <>
          <button className="btn btn--ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn btn--primary" onClick={submit} disabled={!valid || busy}>
            {change ? "Change PIN" : "Enable lock"}
          </button>
        </>
      }
    >
      <div className="field">
        <label className="field__label">New PIN (4–12 digits)</label>
        <input
          className="input"
          {...pinInput}
          autoFocus
          value={pin}
          onChange={(e) => setPin(clean(e.target.value))}
        />
      </div>
      <div className="field">
        <label className="field__label">Confirm PIN</label>
        <input
          className="input"
          {...pinInput}
          value={confirm}
          onChange={(e) => setConfirm(clean(e.target.value))}
          onKeyDown={(e) => e.key === "Enter" && submit()}
        />
        {confirm && pin !== confirm && (
          <span className="field__hint" style={{ color: "var(--danger)" }}>
            PINs don't match
          </span>
        )}
      </div>
    </Modal>
  );
}

/** Remove the app PIN — requires the current PIN. */
export function RemovePinModal({ onClose }: { onClose: () => void }) {
  const setHasPin = useLock((s) => s.setHasPin);
  const [pin, setPin] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (pin.length < 4 || busy) return;
    setBusy(true);
    try {
      await api.clearAppPin(pin);
      setHasPin(false);
      toast("info", "App lock disabled");
      onClose();
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title="Remove PIN"
      onClose={onClose}
      footer={
        <>
          <button className="btn btn--ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn btn--danger" onClick={submit} disabled={pin.length < 4 || busy}>
            Remove lock
          </button>
        </>
      }
    >
      <div className="field">
        <label className="field__label">Current PIN</label>
        <input
          className="input"
          {...pinInput}
          autoFocus
          value={pin}
          onChange={(e) => setPin(clean(e.target.value))}
          onKeyDown={(e) => e.key === "Enter" && submit()}
        />
      </div>
    </Modal>
  );
}
