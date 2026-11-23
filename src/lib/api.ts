import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

// ---------- tipos (espejo de los structs de Rust, en camelCase) ----------

export type Bucket = "notStarted" | "inProgress" | "done" | "reopened";
export type SessionState = "running" | "paused";

export interface User {
  id: number;
  username: string;
  email?: string | null;
  color?: string | null;
}

export interface Ticket {
  id: string;
  label: string;
  name: string;
  status: string;
  statusColor?: string | null;
  url: string;
  listName: string;
  priority?: string | null;
  priorityColor?: string | null;
  dueDate?: number | null;
  bucket: Bucket;
  missingFields: string[];
  workedMs: number;
  sessionState?: SessionState | null;
  /** Agregado a mano por enlace (compartido, no llega en la búsqueda de asignadas). */
  manual: boolean;
}

export interface Counts {
  notStarted: number;
  inProgress: number;
  done: number;
  reopened: number;
  withMissing: number;
}

export interface Punch {
  at: number;
  kind: "start" | "resume" | "pause" | "shift" | "stop";
  note: string;
}

export interface Current {
  taskId: string;
  label: string;
  name: string;
  url: string;
  state: SessionState;
  workedMs: number;
  runningSince?: number | null;
  pauseCount: number;
  missingFields: string[];
  punches: Punch[];
  /** En pausa por fin de turno (no se registra en ClickUp). */
  offShift: boolean;
  /** Hora en que se pausará sola por fin de turno. */
  shiftEndAt?: number | null;
}

export interface OutboxFailure {
  id: number;
  label: string;
  lastError: string;
  attempts: number;
}

export interface Snapshot {
  configured: boolean;
  user?: User | null;
  lastSync?: number | null;
  lastError?: string | null;
  syncing: boolean;
  tickets: Ticket[];
  counts: Counts;
  current?: Current | null;
  pendingSync: number;
  failures: OutboxFailure[];
  todayWorkedMs: number;
}

export interface FieldNames {
  startDate: string;
  endDate: string;
  pauseStarts: string[];
  pauseEnds: string[];
  pauseReason: string;
  pauseState: string;
  pauseStateNone: string;
  pauseStatePaused: string;
  pauseStateClosed: string;
  reopenedDate: string;
  reopenedReason: string;
}

export interface StatusNames {
  inProgress: string;
  done: string;
  notStarted: string[];
  reopened: string[];
}

export interface EmailSettings {
  enabled: boolean;
  smtpHost: string;
  smtpPort: number;
  security: "starttls" | "tls";
  username: string;
  from: string;
  to: string;
  sendAt: string;
  weekdaysOnly: boolean;
}

export interface WorkDay {
  enabled: boolean;
  start: string;
  end: string;
}

export interface Settings {
  teamId?: string | null;
  listIds: string[];
  fields: FieldNames;
  statuses: StatusNames;
  pollMinutes: number;
  reminderMinutes: number;
  /** Lunes (0) a domingo (6). */
  schedule: WorkDay[];
  autoPauseShift: boolean;
  reminderPopup: boolean;
  openInBrowserOnPlay: boolean;
  askPauseReason: boolean;
  ignoredFields: string[];
  email: EmailSettings;
}

export interface Team {
  id: string;
  name: string;
}

export interface FieldOption {
  id: string;
  name: string;
}

export interface EditableField {
  id: string;
  name: string;
  kind: string;
  options: FieldOption[];
  supported: boolean;
}

export interface FillForm {
  taskId: string;
  label: string;
  name: string;
  url: string;
  me?: number | null;
  fields: EditableField[];
  members: FieldOption[];
}

export interface FieldInput {
  fieldId: string;
  name: string;
  kind: string;
  value: string | number | boolean | string[];
}

export interface RequiredField {
  name: string;
  applies: boolean;
}

/** Error que devuelven los comandos de Rust. */
export interface AppError {
  code: "unauthorized" | "rate_limited" | "offline" | "missing_fields" | "invalid" | "error";
  message: string;
  fields: string[];
}

export function asAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "message" in e) return e as AppError;
  return { code: "error", message: String(e), fields: [] };
}

// ---------- comandos ----------

export const api = {
  snapshot: () => invoke<Snapshot>("get_snapshot"),
  refresh: () => invoke<Snapshot>("refresh_now"),
  play: (taskId: string) => invoke<Snapshot>("play", { taskId }),
  pause: (reason: string) => invoke<Snapshot>("pause", { reason }),
  /** Falla con code "missing_fields" si faltan campos obligatorios: no se puede forzar. */
  stop: (taskId: string) => invoke<Snapshot>("stop", { taskId }),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  setToken: (token: string) => invoke<User>("set_clickup_token", { token }),
  clearToken: () => invoke<void>("clear_clickup_token"),
  teams: () => invoke<Team[]>("list_teams"),
  setSmtpPassword: (password: string) => invoke<void>("set_smtp_password", { password }),
  hasSmtpPassword: () => invoke<boolean>("has_smtp_password"),
  testEmail: () => invoke<void>("send_test_email"),
  retryFailed: () => invoke<Snapshot>("retry_failed"),
  discardFailed: (id: number) => invoke<Snapshot>("discard_failed", { id }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  showMain: (view?: string) => invoke<void>("show_main", { view }),
  toggleWidget: () => invoke<void>("toggle_widget"),
  resizeWidget: (height: number) => invoke<void>("resize_widget", { height }),
  getAutostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),
  fillForm: (taskId: string) => invoke<FillForm>("get_fill_form", { taskId }),
  saveFields: (taskId: string, values: FieldInput[]) => invoke<void>("save_fields", { taskId, values }),
  requiredFields: () => invoke<RequiredField[]>("list_required_fields"),
  setFieldApplies: (name: string, applies: boolean) => invoke<void>("set_field_applies", { name, applies }),
  addTaskByRef: (reference: string) => invoke<string>("add_task_by_ref", { reference }),
  removeManualTask: (taskId: string) => invoke<void>("remove_manual_task", { taskId }),
};

// ---------- hooks ----------

/** Foto del estado: carga inicial + actualizaciones que empuja Rust. */
export function useSnapshot(): [Snapshot | null, (s: Snapshot) => void] {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  useEffect(() => {
    let alive = true;
    api.snapshot().then((s) => alive && setSnap(s)).catch(() => {});
    const un = listen<Snapshot>("snapshot", (e) => alive && setSnap(e.payload));
    return () => {
      alive = false;
      un.then((f) => f());
    };
  }, []);
  return [snap, setSnap];
}

/** Reloj que se actualiza cada segundo solo mientras `active` es verdadero. */
export function useNow(active: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    setNow(Date.now());
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [active]);
  return now;
}

/** Tiempo total del ticket activo en este instante. */
export function liveWorked(cur: Current, now: number): number {
  return cur.workedMs + (cur.runningSince ? Math.max(0, now - cur.runningSince) : 0);
}
