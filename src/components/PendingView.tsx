import { ArrowUpRight } from "lucide-react";

import { api } from "../lib/api";
import type { Snapshot } from "../lib/api";
import { Button, IconButton } from "../ui";

/** Tickets abiertos con campos obligatorios sin llenar. */
export default function PendingView({ snap, onFill }: { snap: Snapshot; onFill: (taskId: string, finish?: boolean) => void }) {
  const rows = snap.tickets.filter((t) => t.missingFields.length > 0);
  const total = rows.reduce((n, t) => n + t.missingFields.length, 0);

  if (rows.length === 0) {
    return (
      <div className="empty">
        <p className="empty__title">0 campos pendientes</p>
        <p>Tus tickets abiertos tienen todos los campos obligatorios llenos.</p>
      </div>
    );
  }

  return (
    <>
      <p className="page-intro">
        {total} campos sin llenar en {rows.length} {rows.length === 1 ? "ticket" : "tickets"}. Si un campo lo llena otra
        área, márcalo «no me aplica» y no se te volverá a pedir.
      </p>
      <div>
        {rows.map((t) => (
          <article key={t.id} className="pending">
            <span className="code">{t.label}</span>
            <div style={{ minWidth: 0 }}>
              <div className="truncate" title={t.name}>
                {t.sessionState && <span className="status-dot" data-state={t.sessionState} style={{ marginRight: 8 }} />}
                {t.name}
              </div>
              <div className="pending__fields">{t.missingFields.join(", ")}</div>
            </div>
            <div className="row">
              <IconButton size="sm" label={`Abrir ${t.label} en ClickUp`} onClick={() => api.openUrl(t.url)}>
                <ArrowUpRight />
              </IconButton>
              <Button size="sm" variant="primary" onClick={() => onFill(t.id)}>
                completar
              </Button>
            </div>
          </article>
        ))}
      </div>
    </>
  );
}
