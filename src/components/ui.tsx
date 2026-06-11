import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { IconAlert, IconCheck, IconClose } from "./Icon";
import { useToasts } from "@/stores/toast";

// ---- Modal ----------------------------------------------------------------

interface ModalProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
}

export function Modal({ title, onClose, children, footer, wide }: ModalProps) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  return createPortal(
    <div
      className="modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className={`modal ${wide ? "modal--wide" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <div className="modal__header">
          <div className="modal__title">{title}</div>
          <button
            className="btn btn--icon btn--ghost btn--sm"
            onClick={onClose}
            aria-label="Close"
          >
            <IconClose />
          </button>
        </div>
        <div className="modal__body">{children}</div>
        {footer && <div className="modal__footer">{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}

// ---- Toasts ---------------------------------------------------------------

export function Toasts() {
  const toasts = useToasts((s) => s.toasts);
  const remove = useToasts((s) => s.remove);

  return createPortal(
    <div className="toasts">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast--${t.kind}`} onClick={() => remove(t.id)}>
          {t.kind === "error" && <IconAlert size={16} style={{ color: "var(--danger)" }} />}
          {t.kind === "success" && <IconCheck size={16} style={{ color: "var(--success)" }} />}
          <div>
            {t.title && <div className="toast__title">{t.title}</div>}
            <div className="toast__msg muted">{t.message}</div>
          </div>
        </div>
      ))}
    </div>,
    document.body,
  );
}

// ---- Switch ---------------------------------------------------------------

export function Switch({ on, onChange }: { on: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      className={`switch ${on ? "is-on" : ""}`}
      onClick={() => onChange(!on)}
      role="switch"
      aria-checked={on}
    />
  );
}

// ---- Text prompt ----------------------------------------------------------

export function TextPrompt({
  title,
  label,
  initial,
  confirmLabel,
  onSubmit,
  onClose,
}: {
  title: string;
  label: string;
  initial?: string;
  confirmLabel?: string;
  onSubmit: (value: string) => void;
  onClose: () => void;
}) {
  const [value, setValue] = useState(initial ?? "");
  const submit = () => {
    if (value.trim()) {
      onSubmit(value.trim());
      onClose();
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
          <button className="btn btn--primary" onClick={submit}>
            {confirmLabel ?? "OK"}
          </button>
        </>
      }
    >
      <div className="field">
        <label className="field__label">{label}</label>
        <input
          className="input"
          autoFocus
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && submit()}
          onFocus={(e) => e.target.select()}
        />
      </div>
    </Modal>
  );
}

// ---- Confirm --------------------------------------------------------------

export function ConfirmDialog({
  title,
  message,
  confirmLabel,
  danger,
  onConfirm,
  onClose,
}: {
  title: string;
  message: string;
  confirmLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <Modal
      title={title}
      onClose={onClose}
      footer={
        <>
          <button className="btn btn--ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            className={`btn ${danger ? "btn--danger" : "btn--primary"}`}
            onClick={() => {
              onConfirm();
              onClose();
            }}
          >
            {confirmLabel ?? "Confirm"}
          </button>
        </>
      }
    >
      <div className="muted">{message}</div>
    </Modal>
  );
}

// ---- Empty state ----------------------------------------------------------

export function EmptyState({
  icon,
  title,
  hint,
  action,
}: {
  icon: ReactNode;
  title: string;
  hint?: string;
  action?: ReactNode;
}) {
  return (
    <div className="empty">
      <div className="empty__icon">{icon}</div>
      <div className="empty__title">{title}</div>
      {hint && <div style={{ maxWidth: 360 }}>{hint}</div>}
      {action}
    </div>
  );
}
