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
  /** Horas estimadas en ClickUp, en ms. */
  estimateMs?: number | null;
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
  estimateMs?: number | null;
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
  estimate: string;
  slaDue: string;
  deliveryDue: string;
  evidence: string;
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

export interface StatusOption {
  status: string;
  color?: string | null;
  kind: string;
}

export interface NamedValue {
  name: string;
  value: string;
}

export interface Attachment {
  id: string;
  title: string;
  url: string;
  thumbnail?: string | null;
  extension: string;
  size?: number | null;
  at?: number | null;
}

export interface ChecklistItem {
  id: string;
  name: string;
  resolved: boolean;
}

export interface Checklist {
  id: string;
  name: string;
  items: ChecklistItem[];
}

export interface SubtaskRef {
  id: string;
  label: string;
  name: string;
  status: string;
  statusColor?: string | null;
  done: boolean;
}

export interface TaskDetail {
  id: string;
  label: string;
  name: string;
  url: string;
  status: string;
  statusColor?: string | null;
  statuses: StatusOption[];
  description: string;
  texts: NamedValue[];
  values: NamedValue[];
  dates: { name: string; ms: number }[];
  slaDue?: number | null;
  deliveryDue?: number | null;
  assignees: string[];
  attachments: Attachment[];
  checklists: Checklist[];
  subtasks: SubtaskRef[];
  parentId?: string | null;
  timeSpentMs: number;
  estimateMs?: number | null;
  missingFields: string[];
  sessionState?: SessionState | null;
  /** Campo de evidencias («Observaciones»), aunque esté vacío. */
  evidence?: { id: string; name: string; value: string } | null;
}

export interface Comment {
  id: string;
  text: string;
  user: string;
  userId?: number | null;
  mine: boolean;
  at: number;
  replies: number;
}

export interface Member {
  id: number;
  name: string;
}

export interface CommentPart {
  text?: string;
  userId?: number;
}

export interface TimeEntry {
  id: string;
  start: number;
  end: number;
  duration: number;
  description: string;
  running: boolean;
}

export interface GitHint {
  repo: string;
  branch: string;
  branchUrl?: string | null;
  mrUrl?: string | null;
  matches: boolean;
}

export interface NotifySettings {
  assigned: boolean;
  status: boolean;
  comments: boolean;
  due: boolean;
  dueHours: number;
}

export type Period = "day" | "week" | "month";

export interface ReportTicket {
  taskId: string;
  label: string;
  name: string;
  url: string;
  workedMs: number;
  estimateMs?: number | null;
  finishedAt?: number | null;
}

export interface ReportDay {
  at: number;
  workedMs: number;
  tickets: number;
}

export interface Report {
  period: Period;
  from: number;
  to: number;
  workedMs: number;
  workedTickets: number;
  finishedTickets: number;
  days: ReportDay[];
  tickets: ReportTicket[];
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
  /** Campos que se piden aunque ClickUp no los marque obligatorios. */
  extraRequired: string[];
  notify: NotifySettings;
  /** Carpetas de repositorios locales para sugerir rama y merge request. */
  repoPaths: string[];
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
  report: (period: Period, offset: number) => invoke<Report>("get_report", { period, offset }),
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
  taskDetail: (taskId: string) => invoke<TaskDetail>("get_task_detail", { taskId }),
  comments: (taskId: string) => invoke<Comment[]>("get_comments", { taskId }),
  taskMembers: (taskId: string) => invoke<Member[]>("get_task_members", { taskId }),
  addComment: (taskId: string, parts: CommentPart[]) => invoke<void>("add_comment", { taskId, parts }),
  setStatus: (taskId: string, status: string, note: string) => invoke<void>("set_task_status", { taskId, status, note }),
  setChecklistItem: (checklistId: string, itemId: string, resolved: boolean) =>
    invoke<void>("set_checklist_item", { checklistId, itemId, resolved }),
  timeEntries: (taskId: string) => invoke<TimeEntry[]>("get_time_entries", { taskId }),
  addTimeEntry: (taskId: string, start: number, end: number, note: string) => invoke<void>("add_time_entry", { taskId, start, end, note }),
  updateTimeEntry: (taskId: string, entryId: string, start: number, end: number) =>
    invoke<void>("update_time_entry", { taskId, entryId, start, end }),
  deleteTimeEntry: (entryId: string) => invoke<void>("delete_time_entry", { entryId }),
  uploadEvidence: (taskId: string, filename: string, mime: string, data: string) =>
    invoke<{ url: string; title: string }>("upload_evidence", { taskId, filename, mime, data }),
  gitSuggestions: (label: string) => invoke<GitHint[]>("git_suggestions", { label }),
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
