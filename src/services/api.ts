// Typed wrappers around the Tauri command bridge. Every backend command is
// surfaced here so the rest of the app never calls `invoke` directly.

import { invoke } from "@tauri-apps/api/core";
import type {
  AppInfo,
  ConnectRequest,
  DirEntry,
  DiscoveredKey,
  ProfileDraft,
  ServerProfile,
  SessionInfo,
  TofuResolution,
  TransferTask,
} from "@/types";

// ---- app ------------------------------------------------------------------

export const appInfo = () => invoke<AppInfo>("app_info");
export const forgetHost = (host: string, port: number) =>
  invoke<void>("forget_host", { host, port });

// ---- profiles -------------------------------------------------------------

export const listProfiles = () => invoke<ServerProfile[]>("list_profiles");
export const searchProfiles = (query: string) =>
  invoke<ServerProfile[]>("search_profiles", { query });
export const createProfile = (draft: ProfileDraft) =>
  invoke<ServerProfile>("create_profile", { draft });
export const updateProfile = (id: string, draft: ProfileDraft) =>
  invoke<ServerProfile>("update_profile", { id, draft });
export const deleteProfile = (id: string) =>
  invoke<void>("delete_profile", { id });
export const toggleFavorite = (id: string) =>
  invoke<ServerProfile>("toggle_favorite", { id });
export const setProfilePassword = (id: string, password: string) =>
  invoke<void>("set_profile_password", { id, password });
export const hasProfilePassword = (id: string) =>
  invoke<boolean>("has_profile_password", { id });
export const clearProfilePassword = (id: string) =>
  invoke<void>("clear_profile_password", { id });

// ---- sessions / terminal --------------------------------------------------

export const connect = (request: ConnectRequest) =>
  invoke<SessionInfo>("connect", { request });
export const disconnect = (sessionId: string) =>
  invoke<void>("disconnect", { sessionId });
export const listSessions = () => invoke<SessionInfo[]>("list_sessions");
export const openShell = (sessionId: string, cols: number, rows: number) =>
  invoke<void>("open_shell", { sessionId, cols, rows });
export const sendInput = (sessionId: string, data: string) =>
  invoke<void>("send_input", { sessionId, data });
export const resizeTerminal = (sessionId: string, cols: number, rows: number) =>
  invoke<void>("resize_terminal", { sessionId, cols, rows });
export const respondHostKey = (requestId: string, resolution: TofuResolution) =>
  invoke<void>("respond_host_key", { requestId, resolution });

// ---- sftp / remote fs -----------------------------------------------------

export const listRemoteDir = (sessionId: string, path: string) =>
  invoke<DirEntry[]>("list_remote_dir", { sessionId, path });
export const remoteHomeDir = (sessionId: string) =>
  invoke<string>("remote_home_dir", { sessionId });
export const makeRemoteDir = (sessionId: string, path: string) =>
  invoke<void>("make_remote_dir", { sessionId, path });
export const removeRemote = (sessionId: string, path: string, isDir: boolean) =>
  invoke<void>("remove_remote", { sessionId, path, isDir });
export const renameRemote = (sessionId: string, from: string, to: string) =>
  invoke<void>("rename_remote", { sessionId, from, to });

// ---- local fs -------------------------------------------------------------

export const listLocalDir = (path: string) =>
  invoke<DirEntry[]>("list_local_dir", { path });
export const localHomeDir = () => invoke<string>("local_home_dir");
export const localParentDir = (path: string) =>
  invoke<string | null>("local_parent_dir", { path });
export const makeLocalDir = (path: string) =>
  invoke<void>("make_local_dir", { path });
export const removeLocal = (path: string, isDir: boolean) =>
  invoke<void>("remove_local", { path, isDir });
export const renameLocal = (from: string, to: string) =>
  invoke<void>("rename_local", { from, to });

// ---- transfers ------------------------------------------------------------

export const uploadFile = (
  sessionId: string,
  localPath: string,
  remotePath: string,
) => invoke<string>("upload_file", { sessionId, localPath, remotePath });
export const downloadFile = (
  sessionId: string,
  remotePath: string,
  localPath: string,
) => invoke<string>("download_file", { sessionId, remotePath, localPath });
export const listTransfers = () => invoke<TransferTask[]>("list_transfers");
export const cancelTransfer = (id: string) =>
  invoke<void>("cancel_transfer", { id });
export const retryTransfer = (id: string) =>
  invoke<string>("retry_transfer", { id });
export const clearFinishedTransfers = () =>
  invoke<void>("clear_finished_transfers");

// ---- keys -----------------------------------------------------------------

export const listKeys = () => invoke<DiscoveredKey[]>("list_keys");
export const inspectKey = (path: string) =>
  invoke<DiscoveredKey>("inspect_key", { path });
