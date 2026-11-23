//! Resumen diario por correo vía SMTP. Sin costo: sirve la cuenta corporativa
//! o una de Gmail/Outlook con "contraseña de aplicación". Nada pasa por
//! servidores de terceros además del propio proveedor de correo.

use chrono::{Datelike, Local, Timelike, Weekday};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::{parse_hhmm, EmailSettings};
use crate::error::{AppError, AppResult};
use crate::secrets;
use crate::state::{local_day_start, now_ms, snapshot, AppState, Bucket, Snapshot};
use crate::tracker::fmt_duration;

const KV_LAST_SENT: &str = "digest_last_sent";

/// Se llama cada minuto: envía el resumen una vez al día a partir de la hora configurada.
pub async fn tick(state: &AppState) -> AppResult<()> {
    let s = state.settings();
    if !s.email.enabled {
        return Ok(());
    }
    let now = Local::now();
    if s.email.weekdays_only && matches!(now.weekday(), Weekday::Sat | Weekday::Sun) {
        return Ok(());
    }
    let Some(at) = parse_hhmm(&s.email.send_at) else { return Ok(()) };
    if now.hour() * 60 + now.minute() < at {
        return Ok(());
    }
    let today = now.format("%Y-%m-%d").to_string();
    if state.db.kv_get(KV_LAST_SENT)?.as_deref() == Some(today.as_str()) {
        return Ok(());
    }
    send_digest(state).await?;
    state.db.kv_set(KV_LAST_SENT, &today)?;
    Ok(())
}

pub async fn send_digest(state: &AppState) -> AppResult<()> {
    let s = state.settings();
    let snap = snapshot(state, false).await?;
    let now = now_ms();
    let worked = state.db.worked_by_task_between(local_day_start(now), now)?;
    let html = render(&snap, &worked);
    let subject = format!(
        "TicketPilot · {} · {} trabajadas, {} con campos pendientes",
        Local::now().format("%d/%m/%Y"),
        fmt_duration(snap.today_worked_ms),
        snap.counts.with_missing
    );
    send(&s.email, &subject, html).await
}

async fn send(e: &EmailSettings, subject: &str, html: String) -> AppResult<()> {
    let mail_err = |x: &dyn std::fmt::Display| AppError::Mail(x.to_string());
    if e.username.trim().is_empty() || e.to.trim().is_empty() {
        return Err(AppError::invalid("Completa el usuario SMTP y el destinatario en Ajustes › Correo."));
    }
    let password = secrets::get(secrets::SMTP_PASSWORD)?
        .ok_or_else(|| AppError::invalid("Falta la contraseña SMTP (Ajustes › Correo)."))?;
    let from = if e.from.trim().is_empty() { &e.username } else { &e.from };

    let mut builder = Message::builder()
        .from(from.parse().map_err(|x| mail_err(&x))?)
        .subject(subject)
        .header(ContentType::TEXT_HTML);
    for to in e.to.split([',', ';']).map(str::trim).filter(|x| !x.is_empty()) {
        builder = builder.to(to.parse().map_err(|x| mail_err(&x))?);
    }
    let msg = builder.body(html).map_err(|x| mail_err(&x))?;

    let relay = if e.security == "tls" {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&e.smtp_host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&e.smtp_host)
    }
    .map_err(|x| mail_err(&x))?;
    let transport = relay
        .port(e.smtp_port)
        .credentials(Credentials::new(e.username.clone(), password))
        .timeout(Some(std::time::Duration::from_secs(20)))
        .build();
    transport.send(msg).await.map_err(|x| mail_err(&x))?;
    Ok(())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// HTML con estilos en línea (los clientes de correo ignoran las hojas de estilo).
fn render(snap: &Snapshot, worked_today: &[(String, i64)]) -> String {
    let c = &snap.counts;
    let tile = |label: &str, n: usize, color: &str| {
        format!(
            r#"<td style="padding:12px 16px;border:1px solid #e3e6ea;border-radius:8px;text-align:left">
<div style="font-size:12px;color:#5f6b7a">{label}</div>
<div style="font-size:24px;font-weight:600;color:{color}">{n}</div></td>"#
        )
    };
    let label_of = |id: &str| {
        snap.tickets
            .iter()
            .find(|t| t.id == id)
            .map(|t| (t.label.clone(), t.name.clone(), t.url.clone()))
            .unwrap_or_else(|| (id.to_owned(), String::new(), String::new()))
    };

    let mut worked_rows = String::new();
    for (id, ms) in worked_today {
        let (label, name, url) = label_of(id);
        worked_rows.push_str(&format!(
            r#"<tr><td style="padding:6px 8px"><a href="{}" style="color:#1565c0">{}</a></td><td style="padding:6px 8px">{}</td><td style="padding:6px 8px;text-align:right;font-variant-numeric:tabular-nums">{}</td></tr>"#,
            esc(&url), esc(&label), esc(&name), fmt_duration(*ms)
        ));
    }
    if worked_rows.is_empty() {
        worked_rows = r#"<tr><td colspan="3" style="padding:6px 8px;color:#5f6b7a">No se registró tiempo hoy.</td></tr>"#.into();
    }

    let mut missing_rows = String::new();
    for t in snap.tickets.iter().filter(|t| !t.missing_fields.is_empty() && t.bucket != Bucket::NotStarted) {
        missing_rows.push_str(&format!(
            r#"<tr><td style="padding:6px 8px;vertical-align:top"><a href="{}" style="color:#1565c0">{}</a></td><td style="padding:6px 8px">{}</td></tr>"#,
            esc(&t.url), esc(&t.label), esc(&t.missing_fields.join(", "))
        ));
    }
    if missing_rows.is_empty() {
        missing_rows = r#"<tr><td colspan="2" style="padding:6px 8px;color:#2e7d32">Todos los tickets en curso tienen sus campos completos.</td></tr>"#.into();
    }

    let pending_sync = if snap.pending_sync > 0 || !snap.failures.is_empty() {
        format!(
            r#"<p style="color:#b26a00">Hay {} cambios sin sincronizar con ClickUp y {} con error. Abre TicketPilot para revisarlos.</p>"#,
            snap.pending_sync,
            snap.failures.len()
        )
    } else {
        String::new()
    };

    format!(
        r#"<!doctype html><html><body style="margin:0;padding:24px;background:#f6f7f9;font-family:Roboto,Segoe UI,Helvetica,Arial,sans-serif;color:#1f2933">
<div style="max-width:640px;margin:0 auto;background:#fff;border:1px solid #e3e6ea;border-radius:12px;padding:24px">
<h1 style="font-size:20px;margin:0 0 4px">Resumen del día</h1>
<p style="margin:0 0 20px;color:#5f6b7a">Tiempo trabajado hoy: <strong>{worked}</strong></p>
<table role="presentation" cellspacing="8" style="width:100%;border-collapse:separate"><tr>{t1}{t2}{t3}{t4}</tr></table>
<h2 style="font-size:16px;margin:24px 0 8px">Tiempo por ticket</h2>
<table style="width:100%;border-collapse:collapse;font-size:14px">{worked_rows}</table>
<h2 style="font-size:16px;margin:24px 0 8px">Campos pendientes</h2>
<table style="width:100%;border-collapse:collapse;font-size:14px">{missing_rows}</table>
{pending_sync}
<p style="margin:24px 0 0;font-size:12px;color:#8a94a3">Finalizados: últimos 30 días. Enviado por TicketPilot.</p>
</div></body></html>"#,
        worked = fmt_duration(snap.today_worked_ms),
        t1 = tile("Por iniciar", c.not_started, "#5f6b7a"),
        t2 = tile("En curso", c.in_progress, "#1565c0"),
        t3 = tile("Finalizados", c.done, "#2e7d32"),
        t4 = tile("Reabiertos", c.reopened, "#c62828"),
    )
}
