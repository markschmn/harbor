import { useEffect } from "react";
import { NavRail } from "@/components/NavRail";
import { Workspace } from "@/components/Workspace";
import { HostKeyPromptModal } from "@/components/HostKeyPromptModal";
import { UpdateBanner } from "@/components/UpdateBanner";
import { Toasts } from "@/components/ui";
import { TransfersPage } from "@/pages/TransfersPage";
import { KeysPage } from "@/pages/KeysPage";
import { SettingsPage } from "@/pages/SettingsPage";
import * as api from "@/services/api";
import { EVENTS, on } from "@/services/events";
import { useGlobalShortcuts } from "@/hooks/useShortcuts";
import { useUpdater } from "@/hooks/useUpdater";
import { useUi } from "@/stores/ui";
import { useProfiles } from "@/stores/profiles";
import { useTransfers } from "@/stores/transfers";
import type { TransferEvent } from "@/types";

export default function App() {
  const view = useUi((s) => s.view);
  const setAppInfo = useUi((s) => s.setAppInfo);

  useGlobalShortcuts();
  useUpdater();

  useEffect(() => {
    // Initial data load.
    api.appInfo().then(setAppInfo).catch(() => {});
    useProfiles.getState().load();
    useTransfers.getState().load();

    // Stream transfer events into the store.
    let unlisten: (() => void) | undefined;
    on<TransferEvent>(EVENTS.transfer, (event) => {
      useTransfers.getState().apply(event);
    }).then((fn) => (unlisten = fn));

    return () => unlisten?.();
  }, [setAppInfo]);

  return (
    <div className="app-shell">
      <NavRail />
      <div style={{ position: "relative", minWidth: 0, overflow: "hidden" }}>
        {/* The connections workspace stays mounted so shells survive view
            switches; it is merely hidden when another view is active. */}
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: view === "connections" ? "flex" : "none",
            flexDirection: "column",
          }}
        >
          <Workspace />
        </div>
        {view !== "connections" && (
          <div
            style={{
              position: "absolute",
              inset: 0,
              display: "flex",
              flexDirection: "column",
            }}
          >
            {view === "transfers" && <TransfersPage />}
            {view === "keys" && <KeysPage />}
            {view === "settings" && <SettingsPage />}
          </div>
        )}
      </div>

      <HostKeyPromptModal />
      <UpdateBanner />
      <Toasts />
    </div>
  );
}
