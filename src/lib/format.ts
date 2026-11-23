/** 3 723 000 ms → "01:02:03" (cronómetro). */
export function clock(ms: number): string {
  const total = Math.floor(Math.max(0, ms) / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return [h, m, s].map((n) => String(n).padStart(2, "0")).join(":");
}

/** 5 400 000 ms → "1 h 30 min" (listas y resúmenes). */
export function duration(ms: number): string {
  const mins = Math.round(Math.max(0, ms) / 60000);
  if (mins < 60) return `${mins} min`;
  const m = mins % 60;
  return m ? `${Math.floor(mins / 60)} h ${String(m).padStart(2, "0")} min` : `${mins / 60} h`;
}

/** Hora local "10:03". */
export function hhmm(ms: number): string {
  return timeFmt.format(new Date(ms));
}

const dateFmt = new Intl.DateTimeFormat("es-CO", { day: "numeric", month: "short" });
const timeFmt = new Intl.DateTimeFormat("es-CO", { hour: "2-digit", minute: "2-digit", hour12: false });

export function shortDate(ms?: number | null): string {
  return ms ? dateFmt.format(new Date(ms)).replace(" de ", " ").replace(".", "") : "—";
}

export function relativeTime(ms?: number | null, now = Date.now()): string {
  if (!ms) return "nunca";
  const diff = Math.round((now - ms) / 60000);
  if (diff < 1) return "hace un momento";
  if (diff < 60) return `hace ${diff} min`;
  return `a las ${timeFmt.format(new Date(ms))}`;
}

export const bucketLabel = {
  notStarted: "Por iniciar",
  inProgress: "En curso",
  done: "Finalizado",
  reopened: "Reabierto",
} as const;

export function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** "en code review" → "En code review". */
export function sentence(s: string): string {
  return s ? s[0].toUpperCase() + s.slice(1) : s;
}
