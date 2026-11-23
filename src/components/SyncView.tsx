import { Trash2 } from "lucide-react";

import { api } from "../lib/api";
import type { Snapshot } from "../lib/api";
import { plural, relativeTime } from "../lib/format";
import { Button, IconButton } from "../ui";
import type { Act } from "../windows/MainWindow";

/** Estado de la cola de escrituras hacia ClickUp, como salida de un comando. */
export default function SyncView({ snap, act }: { snap: Snapshot; act: Act }) {
  return (
    <>
      <p className="page-intro">
        Cada play, pausa, stop y campo que completas se guarda primero en este equipo y luego se envía a ClickUp. Sin
        conexión, se reintenta solo y en orden.
      </p>

      <section className="panel" style={{ padding: "20px 20px 16px" }}>
        <span className="panel__title">estado</span>
        <div className="stack" style={{ gap: 2 }}>
          <span>
            <span className="muted">última sincronización ...... </span>
            {relativeTime(snap.lastSync)}
          </span>
          <span>
            <span className="muted">cambios en cola ............ </span>
            <b>{snap.pendingSync}</b>
          </span>
          <span>
            <span className="muted">cambios con error ........... </span>
            <b>{snap.failures.length}</b>
          </span>
          <span style={{ marginTop: 8 }}>
            {snap.pendingSync === 0 && snap.failures.length === 0 ? "> todo está al día con ClickUp" : "> hay cambios por enviar"}
          </span>
        </div>
      </section>

      {snap.failures.length > 0 && (
        <section className="panel" style={{ padding: "20px 0 8px" }}>
          <span className="panel__title">
            <b>{plural(snap.failures.length, "error", "errores")}</b>
          </span>
          <div className="row" style={{ padding: "0 20px 10px" }}>
            <span className="muted" style={{ flex: 1 }}>
              Revisa el motivo. Si ya lo corregiste en ClickUp, descarta el cambio.
            </span>
            <Button size="sm" variant="primary" onClick={() => act(api.retryFailed)}>
              reintentar todos
            </Button>
          </div>
          {snap.failures.map((f) => (
            <div key={f.id} className="ticket" style={{ gridTemplateColumns: "minmax(0,1fr) auto", padding: "6px 20px" }}>
              <div style={{ minWidth: 0 }}>
                <div>{f.label}</div>
                <div className="hint">
                  {f.lastError} ({plural(f.attempts, "intento", "intentos")})
                </div>
              </div>
              <IconButton size="sm" label={`Descartar «${f.label}»`} onClick={() => act(() => api.discardFailed(f.id))}>
                <Trash2 />
              </IconButton>
            </div>
          ))}
        </section>
      )}
    </>
  );
}
