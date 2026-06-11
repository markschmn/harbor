import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const EVENTS = {
  terminalData: "harbor://terminal-data",
  terminalClosed: "harbor://terminal-closed",
  transfer: "harbor://transfer-event",
  hostKeyPrompt: "harbor://host-key-prompt",
} as const;

/** Subscribe to a backend event, returning an unlisten handle. */
export function on<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}
