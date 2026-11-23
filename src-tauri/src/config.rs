//! Ajustes del usuario (JSON en el directorio de configuración de la app).
//! Los valores por defecto coinciden con los campos y estados de la lista
//! "Tickets Activos"; se pueden cambiar desde Ajustes sin recompilar.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FieldNames {
    pub start_date: String,
    pub end_date: String,
    pub pause_starts: Vec<String>,
    pub pause_ends: Vec<String>,
    pub pause_reason: String,
    pub pause_state: String,
    pub pause_state_none: String,
    pub pause_state_paused: String,
    pub pause_state_closed: String,
    pub reopened_date: String,
    pub reopened_reason: String,
}

impl Default for FieldNames {
    fn default() -> Self {
        Self {
            start_date: "Fecha de Inicio".into(),
            end_date: "Fecha Fin".into(),
            pause_starts: (1..=3).map(|i| format!("Inicio Pausa {i}")).collect(),
            pause_ends: (1..=3).map(|i| format!("Fin Pausa {i}")).collect(),
            pause_reason: "Motivo de Pausa".into(),
            pause_state: "Estado de Pausa".into(),
            pause_state_none: "Sin pausa".into(),
            pause_state_paused: "En pausa".into(),
            pause_state_closed: "Cerrada".into(),
            reopened_date: "Fecha de Reabierto".into(),
            reopened_reason: "Motivo de Reabierto".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StatusNames {
    /// Estado al pulsar Play.
    pub in_progress: String,
    /// Estado al pulsar Stop.
    pub done: String,
    /// Estados que cuentan como "Por iniciar".
    pub not_started: Vec<String>,
    /// Estados que cuentan como "Reabierto".
    pub reopened: Vec<String>,
}

impl Default for StatusNames {
    fn default() -> Self {
        Self {
            in_progress: "en desarrollo".into(),
            done: "listo para release".into(),
            not_started: vec![
                "por iniciar".into(),
                "análisis técnico".into(),
                "reportado a desarrollo".into(),
                "nuevo reporte".into(),
            ],
            reopened: vec!["reabierto".into()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EmailSettings {
    pub enabled: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    /// "starttls" (puerto 587) o "tls" (puerto 465).
    pub security: String,
    pub username: String,
    pub from: String,
    pub to: String,
    /// Hora local HH:MM del resumen diario.
    pub send_at: String,
    pub weekdays_only: bool,
}

impl Default for EmailSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            smtp_host: "smtp.gmail.com".into(),
            smtp_port: 587,
            security: "starttls".into(),
            username: String::new(),
            from: String::new(),
            to: String::new(),
            send_at: "17:30".into(),
            weekdays_only: true,
        }
    }
}

/// Jornada de un día. `enabled = false` = día libre.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkDay {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

impl Default for WorkDay {
    fn default() -> Self {
        Self { enabled: true, start: "07:00".into(), end: "17:00".into() }
    }
}

impl WorkDay {
    /// Lunes a jueves 7:00–17:00, viernes 7:00–16:30, fin de semana libre.
    pub fn default_week() -> Vec<WorkDay> {
        let day = |enabled: bool, end: &str| WorkDay { enabled, start: "07:00".into(), end: end.into() };
        vec![
            day(true, "17:00"),
            day(true, "17:00"),
            day(true, "17:00"),
            day(true, "17:00"),
            day(true, "16:30"),
            day(false, "17:00"),
            day(false, "17:00"),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub team_id: Option<String>,
    /// Vacío = todas las listas del workspace.
    pub list_ids: Vec<String>,
    pub fields: FieldNames,
    pub statuses: StatusNames,
    pub poll_minutes: u64,
    pub reminder_minutes: u64,
    /// Jornada por día de la semana, de lunes (0) a domingo (6).
    pub schedule: Vec<WorkDay>,
    /// Al llegar la hora de salida, pausar el cronómetro solo en la app
    /// (esa pausa no se registra en ClickUp).
    pub auto_pause_shift: bool,
    /// Al recordar campos pendientes, además de la notificación, traer la ventana al frente.
    pub reminder_popup: bool,
    pub open_in_browser_on_play: bool,
    pub ask_pause_reason: bool,
    /// Campos obligatorios que el desarrollador no llena (los llena otra área).
    pub ignored_fields: Vec<String>,
    pub email: EmailSettings,
    /// Tickets agregados a mano (ids internos). ClickUp no devuelve en la búsqueda
    /// de "asignadas a mí" las tareas compartidas sueltas ("Shared with me").
    pub extra_task_ids: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            team_id: None,
            list_ids: vec![],
            fields: FieldNames::default(),
            statuses: StatusNames::default(),
            poll_minutes: 5,
            reminder_minutes: 60,
            schedule: WorkDay::default_week(),
            auto_pause_shift: true,
            reminder_popup: false,
            open_in_browser_on_play: false,
            ask_pause_reason: true,
            // Los llenan soporte / analítica al crear el ticket, no el desarrollador.
            ignored_fields: vec![
                "Fecha y hora reporte".into(),
                "Canal de reporte".into(),
                "Origen".into(),
                "Revisor de Soporte".into(),
                "Analista Responsable".into(),
                "Aprobación QA".into(),
                "Estado de Validación".into(),
                "Rol Validador".into(),
                "N.º de semana del año".into(),
            ],
            email: EmailSettings::default(),
            extra_task_ids: vec![],
        }
    }
}

impl Settings {
    /// Jornada (inicio, fin) en minutos desde medianoche para una fecha; `None` = día libre.
    pub fn shift_for(&self, date: chrono::NaiveDate) -> Option<(u32, u32)> {
        use chrono::Datelike;
        let d = self.schedule.get(date.weekday().num_days_from_monday() as usize)?;
        let (a, b) = (parse_hhmm(&d.start)?, parse_hhmm(&d.end)?);
        (d.enabled && a < b).then_some((a, b))
    }

    /// ¿La hora local está dentro de la jornada de ese día?
    pub fn within_shift(&self, at: chrono::DateTime<chrono::Local>) -> bool {
        use chrono::Timelike;
        let m = at.hour() * 60 + at.minute();
        self.shift_for(at.date_naive()).is_some_and(|(a, b)| a <= m && m < b)
    }

    fn path(dir: &Path) -> PathBuf {
        dir.join("settings.json")
    }

    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(Self::path(dir))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Escritura atómica: archivo temporal + rename, para no corromper el JSON
    /// si la app se cierra a mitad de escritura.
    pub fn save(&self, dir: &Path) -> AppResult<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join("settings.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, Self::path(dir))?;
        Ok(())
    }
}

/// "HH:MM" → minutos desde medianoche.
pub fn parse_hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn jornada_por_defecto() {
        let s = Settings::default();
        let jue = NaiveDate::from_ymd_opt(2026, 11, 26).unwrap();
        let vie = NaiveDate::from_ymd_opt(2026, 11, 27).unwrap();
        let sab = NaiveDate::from_ymd_opt(2026, 11, 28).unwrap();
        assert_eq!(s.shift_for(jue), Some((420, 1020)));
        assert_eq!(s.shift_for(vie), Some((420, 990)));
        assert_eq!(s.shift_for(sab), None);
    }
}
