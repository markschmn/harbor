import { create } from "zustand";
import type { CommandError } from "@/types";

export type ToastKind = "error" | "success" | "info";

export interface Toast {
  id: number;
  kind: ToastKind;
  title?: string;
  message: string;
}

interface ToastState {
  toasts: Toast[];
  push: (kind: ToastKind, message: string, title?: string) => void;
  remove: (id: number) => void;
}

let counter = 0;

export const useToasts = create<ToastState>((set) => ({
  toasts: [],
  push: (kind, message, title) => {
    const id = ++counter;
    set((s) => ({ toasts: [...s.toasts, { id, kind, message, title }] }));
    const ttl = kind === "error" ? 7000 : 3500;
    setTimeout(
      () => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
      ttl,
    );
  },
  remove: (id) =>
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/** Extract a human message from a rejected Tauri command. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) {
    return (e as CommandError).message;
  }
  return String(e);
}

/** The stable error code from a rejected command, if present. */
export function errorCode(e: unknown): string | null {
  if (e && typeof e === "object" && "code" in e) {
    return (e as CommandError).code;
  }
  return null;
}

export function toast(kind: ToastKind, message: string, title?: string) {
  useToasts.getState().push(kind, message, title);
}
