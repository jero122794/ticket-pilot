//! Persistencia local en SQLite (modo WAL).
//!
//! Principio: **la fuente de verdad del tiempo es local**. Cada Play/Pausa/Stop
//! se guarda aquí primero (en microsegundos de disco) y luego se replica a
//! ClickUp mediante el `outbox`. Si no hay red, o ClickUp falla, no se pierde
//! ninguna marca de tiempo y el cronómetro sobrevive a reinicios.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::AppResult;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
  id          INTEGER PRIMARY KEY,
  task_id     TEXT NOT NULL,
  task_label  TEXT NOT NULL,
  task_name   TEXT NOT NULL,
  task_url    TEXT NOT NULL,
  list_id     TEXT NOT NULL,
  state       TEXT NOT NULL CHECK (state IN ('running','paused','stopped')),
  started_at  INTEGER NOT NULL,
  ended_at    INTEGER,
  updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_sessions_state ON sessions(state);
CREATE INDEX IF NOT EXISTS ix_sessions_task  ON sessions(task_id);

-- Tramos de trabajo efectivo (entre Play y Pausa/Stop).
CREATE TABLE IF NOT EXISTS segments (
  id          INTEGER PRIMARY KEY,
  session_id  INTEGER NOT NULL REFERENCES sessions(id),
  task_id     TEXT NOT NULL,
  started_at  INTEGER NOT NULL,
  ended_at    INTEGER
);
CREATE INDEX IF NOT EXISTS ix_segments_session ON segments(session_id);
CREATE INDEX IF NOT EXISTS ix_segments_time    ON segments(started_at, ended_at);

-- slot 1..3 = campos "Inicio/Fin Pausa N"; slot 0 = se registra como comentario.
CREATE TABLE IF NOT EXISTS pauses (
  id          INTEGER PRIMARY KEY,
  session_id  INTEGER NOT NULL REFERENCES sessions(id),
  task_id     TEXT NOT NULL,
  slot        INTEGER NOT NULL,
  seq         INTEGER NOT NULL,
  started_at  INTEGER NOT NULL,
  ended_at    INTEGER,
  reason      TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS ix_pauses_task ON pauses(task_id);

-- Cola de escrituras pendientes hacia ClickUp (patrón outbox).
CREATE TABLE IF NOT EXISTS outbox (
  id          INTEGER PRIMARY KEY,
  task_id     TEXT NOT NULL,
  label       TEXT NOT NULL,
  op          TEXT NOT NULL,
  attempts    INTEGER NOT NULL DEFAULT 0,
  last_error  TEXT,
  failed      INTEGER NOT NULL DEFAULT 0,
  next_at     INTEGER NOT NULL,
  created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_outbox_due ON outbox(failed, next_at);

CREATE TABLE IF NOT EXISTS kv (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub id: i64,
    pub task_id: String,
    pub task_label: String,
    pub task_name: String,
    pub task_url: String,
    pub list_id: String,
    pub state: String,
    pub started_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Punch {
    pub at: i64,
    /// start | resume | pause | stop
    pub kind: String,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct PauseRow {
    pub slot: i64,
    pub seq: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct OutboxRow {
    pub id: i64,
    pub task_id: String,
    pub op: String,
    pub attempts: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxFailure {
    pub id: i64,
    pub label: String,
    pub last_error: String,
    pub attempts: i64,
}

pub struct Db(Mutex<Connection>);

pub struct NewSession<'a> {
    pub task_id: &'a str,
    pub task_label: &'a str,
    pub task_name: &'a str,
    pub task_url: &'a str,
    pub list_id: &'a str,
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=3000;",
        )?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self(Mutex::new(conn)))
    }

    fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> AppResult<T> {
        // Si un hilo entró en pánico con el lock tomado, la conexión sigue siendo válida.
        let conn = self.0.lock().unwrap_or_else(|p| p.into_inner());
        Ok(f(&conn)?)
    }

    /// Ejecuta varias escrituras en una transacción: o se guarda todo o nada.
    pub fn tx<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> AppResult<T> {
        let mut conn = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let tx = conn.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    // ---------- sesiones ----------

    /// Sesión visible en el widget: la que corre o, si no, la última pausada.
    pub fn current_session(&self) -> AppResult<Option<SessionRow>> {
        self.with(|c| {
            c.query_row(
                "SELECT id, task_id, task_label, task_name, task_url, list_id, state, started_at
                 FROM sessions WHERE state IN ('running','paused')
                 ORDER BY (state = 'running') DESC, updated_at DESC LIMIT 1",
                [],
                map_session,
            )
            .optional()
        })
    }

    pub fn open_session_for_task(&self, task_id: &str) -> AppResult<Option<SessionRow>> {
        self.with(|c| {
            c.query_row(
                "SELECT id, task_id, task_label, task_name, task_url, list_id, state, started_at
                 FROM sessions WHERE task_id = ?1 AND state IN ('running','paused')
                 ORDER BY updated_at DESC LIMIT 1",
                [task_id],
                map_session,
            )
            .optional()
        })
    }

    pub fn running_session(&self) -> AppResult<Option<SessionRow>> {
        self.with(|c| {
            c.query_row(
                "SELECT id, task_id, task_label, task_name, task_url, list_id, state, started_at
                 FROM sessions WHERE state = 'running' LIMIT 1",
                [],
                map_session,
            )
            .optional()
        })
    }

    pub fn paused_task_ids(&self) -> AppResult<Vec<String>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT task_id FROM sessions WHERE state = 'paused'")?;
            let rows = st.query_map([], |r| r.get(0))?;
            rows.collect()
        })
    }

    // ---------- tiempos ----------

    /// Marcas de reloj de un ticket, en orden: inicios/reanudaciones (tramos),
    /// pausas con su motivo y finales de sesión. Las últimas `limit`.
    pub fn task_punches(&self, task_id: &str, limit: i64) -> AppResult<Vec<Punch>> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT at, kind, note FROM (
                   SELECT s.started_at AS at,
                          CASE WHEN s.id = (SELECT MIN(id) FROM segments WHERE session_id = s.session_id)
                               THEN 'start' ELSE 'resume' END AS kind,
                          '' AS note
                   FROM segments s WHERE s.task_id = ?1
                   UNION ALL
                   SELECT started_at, 'pause', reason FROM pauses WHERE task_id = ?1
                   UNION ALL
                   SELECT ended_at, 'stop', '' FROM sessions WHERE task_id = ?1 AND state = 'stopped'
                 ) ORDER BY at DESC LIMIT ?2",
            )?;
            let rows = st.query_map(params![task_id, limit], |r| {
                Ok(Punch { at: r.get(0)?, kind: r.get(1)?, note: r.get(2)? })
            })?;
            let mut v: Vec<Punch> = rows.collect::<rusqlite::Result<_>>()?;
            v.reverse();
            Ok(v)
        })
    }

    pub fn task_worked_ms(&self, task_id: &str, now: i64) -> AppResult<i64> {
        self.with(|c| {
            c.query_row(
                "SELECT COALESCE(SUM(COALESCE(ended_at, ?2) - started_at), 0) FROM segments WHERE task_id = ?1",
                params![task_id, now],
                |r| r.get(0),
            )
        })
    }

    /// Horas trabajadas por ticket dentro de [from, to): recorta los tramos que cruzan el límite.
    pub fn worked_by_task_between(&self, from: i64, to: i64) -> AppResult<Vec<(String, i64)>> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT task_id,
                        SUM(MIN(COALESCE(ended_at, ?2), ?2) - MAX(started_at, ?1)) AS ms
                 FROM segments
                 WHERE started_at < ?2 AND COALESCE(ended_at, ?2) > ?1
                 GROUP BY task_id ORDER BY ms DESC",
            )?;
            let rows = st.query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })
    }

    /// Inicio del tramo abierto de una sesión (si el cronómetro está corriendo).
    pub fn open_segment_start(&self, session_id: i64) -> AppResult<Option<i64>> {
        self.with(|c| {
            c.query_row(
                "SELECT started_at FROM segments WHERE session_id = ?1 AND ended_at IS NULL ORDER BY id DESC LIMIT 1",
                [session_id],
                |r| r.get(0),
            )
            .optional()
        })
    }

    /// Tiempo trabajado por ticket (todas las sesiones), para mostrarlo en la lista.
    pub fn worked_all(&self, now: i64) -> AppResult<Vec<(String, i64)>> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT task_id, SUM(COALESCE(ended_at, ?1) - started_at) FROM segments GROUP BY task_id",
            )?;
            let rows = st.query_map([now], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })
    }

    pub fn used_pause_slots(&self, task_id: &str) -> AppResult<Vec<i64>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT slot FROM pauses WHERE task_id = ?1 AND slot > 0")?;
            let rows = st.query_map([task_id], |r| r.get(0))?;
            rows.collect()
        })
    }

    pub fn pause_count(&self, task_id: &str) -> AppResult<i64> {
        self.with(|c| c.query_row("SELECT COUNT(*) FROM pauses WHERE task_id = ?1", [task_id], |r| r.get(0)))
    }

    // ---------- outbox ----------

    pub fn outbox_due(&self, now: i64, limit: i64) -> AppResult<Vec<OutboxRow>> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT id, task_id, op, attempts FROM outbox
                 WHERE failed = 0 AND next_at <= ?1 ORDER BY id LIMIT ?2",
            )?;
            let rows = st.query_map(params![now, limit], |r| {
                Ok(OutboxRow { id: r.get(0)?, task_id: r.get(1)?, op: r.get(2)?, attempts: r.get(3)? })
            })?;
            rows.collect()
        })
    }

    pub fn outbox_done(&self, id: i64) -> AppResult<()> {
        self.with(|c| c.execute("DELETE FROM outbox WHERE id = ?1", [id]).map(|_| ()))
    }

    /// Reprograma una operación fallida. Todas las operaciones posteriores del mismo
    /// ticket se retrasan también, para conservar el orden (p. ej. fin de pausa tras inicio).
    pub fn outbox_retry_later(&self, row: &OutboxRow, err: &str, next_at: i64, permanent: bool) -> AppResult<()> {
        self.with(|c| {
            c.execute(
                "UPDATE outbox SET attempts = attempts + 1, last_error = ?2, next_at = ?3, failed = ?4 WHERE id = ?1",
                params![row.id, err, next_at, permanent as i64],
            )?;
            c.execute(
                "UPDATE outbox SET next_at = MAX(next_at, ?3) WHERE task_id = ?1 AND id > ?2 AND failed = 0",
                params![row.task_id, row.id, next_at],
            )?;
            Ok(())
        })
    }

    pub fn outbox_pending(&self) -> AppResult<i64> {
        self.with(|c| c.query_row("SELECT COUNT(*) FROM outbox WHERE failed = 0", [], |r| r.get(0)))
    }

    pub fn outbox_failures(&self) -> AppResult<Vec<OutboxFailure>> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT id, label, COALESCE(last_error, ''), attempts FROM outbox WHERE failed = 1 ORDER BY id",
            )?;
            let rows = st.query_map([], |r| {
                Ok(OutboxFailure { id: r.get(0)?, label: r.get(1)?, last_error: r.get(2)?, attempts: r.get(3)? })
            })?;
            rows.collect()
        })
    }

    pub fn outbox_retry_failed(&self, now: i64) -> AppResult<()> {
        self.with(|c| {
            c.execute("UPDATE outbox SET failed = 0, attempts = 0, next_at = ?1 WHERE failed = 1", [now])
                .map(|_| ())
        })
    }

    pub fn outbox_discard(&self, id: i64) -> AppResult<()> {
        self.with(|c| c.execute("DELETE FROM outbox WHERE id = ?1 AND failed = 1", [id]).map(|_| ()))
    }

    // ---------- kv ----------

    pub fn kv_get(&self, key: &str) -> AppResult<Option<String>> {
        self.with(|c| c.query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0)).optional())
    }

    pub fn kv_set(&self, key: &str, value: &str) -> AppResult<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO kv(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map(|_| ())
        })
    }
}

// ---------- operaciones dentro de una transacción ----------

pub fn insert_session(c: &Connection, s: &NewSession, now: i64) -> rusqlite::Result<i64> {
    c.execute(
        "INSERT INTO sessions(task_id, task_label, task_name, task_url, list_id, state, started_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6, ?6)",
        params![s.task_id, s.task_label, s.task_name, s.task_url, s.list_id, now],
    )?;
    Ok(c.last_insert_rowid())
}

pub fn set_session_state(c: &Connection, id: i64, state: &str, now: i64) -> rusqlite::Result<()> {
    let ended = (state == "stopped").then_some(now);
    c.execute(
        "UPDATE sessions SET state = ?2, updated_at = ?3, ended_at = COALESCE(?4, ended_at) WHERE id = ?1",
        params![id, state, now, ended],
    )?;
    Ok(())
}

pub fn open_segment(c: &Connection, session_id: i64, task_id: &str, now: i64) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO segments(session_id, task_id, started_at) VALUES (?1, ?2, ?3)",
        params![session_id, task_id, now],
    )?;
    Ok(())
}

/// Cierra el tramo abierto y devuelve (inicio, fin) para registrarlo en ClickUp.
pub fn close_segment(c: &Connection, session_id: i64, now: i64) -> rusqlite::Result<Option<(i64, i64)>> {
    let open: Option<(i64, i64)> = c
        .query_row(
            "SELECT id, started_at FROM segments WHERE session_id = ?1 AND ended_at IS NULL ORDER BY id DESC LIMIT 1",
            [session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match open {
        Some((id, start)) => {
            c.execute("UPDATE segments SET ended_at = ?2 WHERE id = ?1", params![id, now])?;
            Ok(Some((start, now)))
        }
        None => Ok(None),
    }
}

pub fn insert_pause(
    c: &Connection,
    session_id: i64,
    task_id: &str,
    slot: i64,
    seq: i64,
    now: i64,
    reason: &str,
) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO pauses(session_id, task_id, slot, seq, started_at, reason) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![session_id, task_id, slot, seq, now, reason],
    )?;
    Ok(())
}

pub fn close_pause(c: &Connection, session_id: i64, now: i64) -> rusqlite::Result<Option<PauseRow>> {
    let open = c
        .query_row(
            "SELECT id, slot, seq, started_at, ended_at FROM pauses
             WHERE session_id = ?1 AND ended_at IS NULL ORDER BY id DESC LIMIT 1",
            [session_id],
            |r| Ok((r.get::<_, i64>(0)?, map_pause_offset(r, 1)?)),
        )
        .optional()?;
    match open {
        Some((id, mut p)) => {
            c.execute("UPDATE pauses SET ended_at = ?2 WHERE id = ?1", params![id, now])?;
            p.ended_at = Some(now);
            Ok(Some(p))
        }
        None => Ok(None),
    }
}

pub fn enqueue(c: &Connection, task_id: &str, label: &str, op_json: &str, now: i64) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO outbox(task_id, label, op, next_at, created_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![task_id, label, op_json, now],
    )?;
    Ok(())
}

fn map_session(r: &rusqlite::Row) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow {
        id: r.get(0)?,
        task_id: r.get(1)?,
        task_label: r.get(2)?,
        task_name: r.get(3)?,
        task_url: r.get(4)?,
        list_id: r.get(5)?,
        state: r.get(6)?,
        started_at: r.get(7)?,
    })
}

fn map_pause_offset(r: &rusqlite::Row, o: usize) -> rusqlite::Result<PauseRow> {
    Ok(PauseRow {
        slot: r.get(o)?,
        seq: r.get(o + 1)?,
        started_at: r.get(o + 2)?,
        ended_at: r.get(o + 3)?,
    })
}
