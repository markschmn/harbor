// TypeScript mirror of the serde DTOs exposed by the Rust backend.
// Field names use snake_case to match the backend's wire format exactly.

export type AuthMethod =
  | { type: "agent" }
  | { type: "password" }
  | { type: "public_key"; key_path: string; encrypted: boolean };

export type AuthKind = AuthMethod["type"];

export interface ServerProfile {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  auth: AuthMethod;
  notes: string;
  tags: string[];
  favorite: boolean;
  created_at: string;
  updated_at: string;
}

export interface ProfileDraft {
  name: string;
  host: string;
  port: number | null;
  username: string;
  auth: AuthMethod;
  notes: string;
  tags: string[];
  favorite: boolean;
}

export type FileKind = "file" | "directory" | "symlink" | "other";

export interface DirEntry {
  name: string;
  path: string;
  kind: FileKind;
  size: number;
  permissions: number | null;
  modified: string | null;
  symlink_target?: string | null;
}

export type SessionStatus =
  | { state: "connecting" }
  | { state: "connected" }
  | { state: "disconnected"; reason: string }
  | { state: "failed"; reason: string };

export interface SessionInfo {
  id: string;
  profile_id: string | null;
  title: string;
  status: SessionStatus;
}

export type TransferDirection = "upload" | "download";

export type TransferState =
  | { state: "queued" }
  | { state: "active" }
  | { state: "paused" }
  | { state: "completed" }
  | { state: "failed"; error: string }
  | { state: "cancelled" };

export interface TransferTask {
  id: string;
  session_id: string;
  direction: TransferDirection;
  source: string;
  destination: string;
  file_name: string;
  state: TransferState;
  total_bytes: number;
  transferred_bytes: number;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
}

export interface TransferProgress {
  id: string;
  transferred_bytes: number;
  total_bytes: number;
  bytes_per_second: number;
}

export type TransferEvent =
  | { kind: "added"; task: TransferTask }
  | { kind: "progress"; progress: TransferProgress }
  | { kind: "state_changed"; id: string; state: TransferState };

export interface DiscoveredKey {
  private_key_path: string;
  public_key_path: string | null;
  algorithm: string;
  fingerprint: string;
  comment: string | null;
  bits: number | null;
  encrypted: boolean;
}

export interface AppInfo {
  version: string;
  keychain_persistent: boolean;
  platform: string;
}

export interface HostKeyPrompt {
  request_id: string;
  host: string;
  port: number;
  algorithm: string;
  fingerprint: string;
}

export type TofuResolution = "trust_and_save" | "trust_once" | "reject";

export interface TerminalData {
  session_id: string;
  data: string; // base64
}

export interface TerminalClosed {
  session_id: string;
  exit_code: number | null;
}

export interface CommandError {
  code: string;
  message: string;
}

export interface ConnectRequest {
  profile_id: string;
  password?: string;
  passphrase?: string;
  remember_secret?: boolean;
}
