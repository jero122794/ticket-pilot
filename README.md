# TicketPilot

Cronómetro de escritorio para los tickets de ClickUp. Le das **Play** cuando empiezas un ticket, **Pausa** cuando lo dejas y **Finalizar** cuando terminas. TicketPilot llena por ti en ClickUp las fechas de inicio, las pausas y la fecha de fin, cambia el estado del ticket y te recuerda los campos obligatorios que faltan.

Funciona en **Linux, macOS y Windows**.

---

## Contenido

1. [Qué hace](#qué-hace)
2. [Instalación (usuarios)](#instalación-usuarios)
3. [Primer uso: conectar ClickUp](#primer-uso-conectar-clickup)
4. [Uso diario](#uso-diario)
5. [Qué escribe en ClickUp](#qué-escribe-en-clickup)
6. [Campos pendientes](#campos-pendientes)
7. [Resumen diario por correo](#resumen-diario-por-correo)
8. [Ajustes](#ajustes)
9. [Sin conexión y sincronización](#sin-conexión-y-sincronización)
10. [Solución de problemas](#solución-de-problemas)
11. [Para desarrolladores](#para-desarrolladores)

---

## Qué hace

| Necesidad | Cómo lo resuelve |
|---|---|
| Ver mis tickets asignados | El panel lista todos tus tickets de ClickUp, abiertos y finalizados en los últimos 30 días. |
| No olvidar llenar fechas y pausas | Play, Pausa y Finalizar escriben los campos y el estado en ClickUp automáticamente. |
| Saber cuánto me toma cada ticket | Cronómetro por ticket. El tiempo también se registra en el time tracking nativo de ClickUp. |
| Que no queden campos vacíos | Recordatorios del sistema y un formulario para completar los campos sin abrir ClickUp. |
| Seguimiento de mi trabajo | Contadores de tickets por iniciar, en curso, reabiertos y finalizados. |
| Reporte diario | Correo automático con el tiempo del día y los campos pendientes. |

La app tiene dos ventanas:

- **Cronómetro flotante:** una ventana pequeña, siempre encima de las demás, con el reloj y los botones. La puedes arrastrar a donde quieras.
- **Panel:** la ventana completa con tus tickets, los pendientes, el estado de sincronización y los ajustes.

Si cierras cualquiera de las dos, la app sigue corriendo en la **bandeja del sistema**, el ícono junto al reloj del sistema operativo. Para salir del todo, usa clic derecho en ese ícono › *Salir de TicketPilot*.

---

## Instalación (usuarios)

Descarga el instalador de tu sistema, que te entrega quien administra la app o se genera como se explica en [Generar instaladores](#generar-instaladores):

| Sistema | Archivo | Cómo instalar |
|---|---|---|
| Windows | `TicketPilot_x.y.z_x64-setup.exe` o `.msi` | Doble clic y sigue el asistente. |
| macOS | `TicketPilot_x.y.z_aarch64.dmg` (Apple Silicon) o `_x64.dmg` (Intel) | Abre el `.dmg` y arrastra TicketPilot a *Aplicaciones*. La primera vez: clic derecho › *Abrir*. |
| Ubuntu / Debian | `ticket-pilot_x.y.z_amd64.deb` | `sudo apt install ./ticket-pilot_x.y.z_amd64.deb` |
| Fedora / RHEL | `ticket-pilot-x.y.z-1.x86_64.rpm` | `sudo dnf install ./ticket-pilot-x.y.z-1.x86_64.rpm` |
| Otras distribuciones Linux | `ticket-pilot_x.y.z_amd64.AppImage` | `chmod +x` al archivo y ejecútalo. |

> En Linux, el token se guarda en el llavero del sistema (GNOME Keyring o KWallet). Ya viene en Ubuntu, Fedora y la mayoría de escritorios.

---

## Primer uso: conectar ClickUp

TicketPilot usa la **API oficial de ClickUp** con tu token personal. Solo ve lo mismo que tú ves en ClickUp.

1. En ClickUp, haz clic en tu avatar (abajo a la izquierda) › **Settings** › **Apps**.
2. En **API Token**, pulsa **Generate** y copia el token. Empieza por `pk_`.
3. Abre TicketPilot. Si es la primera vez, se abre sola la sección **Ajustes**.
4. Pega el token en **Cuenta de ClickUp** y pulsa **conectar**.

Listo: en unos segundos aparecen tus tickets. El token se guarda en el llavero del sistema operativo (Keychain, Credential Manager o Secret Service), nunca en un archivo de texto.

**Recomendado:** activa en Ajustes **Abrir al iniciar sesión** para que la app arranque sola con el equipo, minimizada en la bandeja.

---

## Uso diario

### Empezar un ticket (Play)

Puedes hacerlo de cualquiera de estas formas:

- **Cronómetro flotante:** pulsa **elegir ticket**, escribe parte del código o del nombre (por ejemplo `15237`) y pulsa Enter.
- **Panel › Mis tickets:** pulsa ▷ en la fila del ticket.
- **Panel:** si no hay nada corriendo, el panel *ahora* sugiere el siguiente ticket con un botón para iniciarlo.

Solo un ticket corre a la vez. Si le das Play a otro, el anterior se pausa solo, con el motivo «Cambio al ticket …».

### Pausar y reanudar

Pulsa **pausar**. La app te pide el motivo (por ejemplo «me reasignaron un ticket urgente»). Para confirmar sin el mouse, usa **Ctrl + Enter**. Para continuar, pulsa **reanudar**.

### Fin de turno

Cuando un ticket sigue corriendo a la hora de salida, TicketPilot lo pausa solo **a la hora exacta de salida**. Esto pasa aunque la app estuviera cerrada o el equipo suspendido. Esa pausa es **solo local**:

- En ClickUp no se llena «Inicio/Fin Pausa N», «Estado de Pausa» ni «Motivo de Pausa», y no se escribe ningún comentario.
- Tampoco cuenta en el número de pausas.
- Sí se envía a ClickUp el registro de tiempo del tramo trabajado hasta la salida.

Al día siguiente, al empezar tu jornada, te llega un recordatorio. Dale **reanudar** y el ticket sigue sin dejar rastro de pausa en ClickUp.

Si reanudas después de la salida, la app lo toma como **tiempo extra a propósito** y no lo vuelve a pausar. Lo mismo pasa con lo que empieces en un día libre.

La jornada viene configurada de lunes a jueves de 7:00 a 17:00 y los viernes de 7:00 a 16:30. Se cambia en **Ajustes → Jornada laboral**, donde también puedes desactivar la pausa automática.

### Finalizar (Stop)

Pulsa **finalizar** y confirma.

- **No se puede finalizar con campos obligatorios vacíos.** Si falta alguno, se abre el formulario **completa para finalizar** con los campos pendientes. El botón **guardar y finalizar** se activa cuando todos están llenos: guarda los valores y finaliza el ticket en un solo paso.
- Si un campo solo se puede llenar en ClickUp, el formulario lo indica. Llénalo allá y pulsa **ya los llené, revisar**.
- Los campos que llena otra área no te bloquean: márcalos **no me aplica** (ver [Campos pendientes](#campos-pendientes)).
- TicketPilot no te pide las fechas que pone él mismo (inicio, fin y reabierto).
- Al finalizar, el ticket pasa al estado configurado («listo para release» por defecto) y se deja un comentario con el tiempo total.

### El panel

| Sección | Tecla | Para qué sirve |
|---|---|---|
| Mis tickets | `1` | El ticket activo con su reloj y sus **marcas** (hora de inicio, pausas con su motivo y reanudaciones), y la lista de tickets con filtros por estado y buscador. |
| Pendientes | `2` | Tickets con campos obligatorios sin llenar y el botón **completar**. |
| Sincronización | `3` | Cambios que aún no llegan a ClickUp y errores, si los hay. |
| Ajustes | `4` | Cuenta, comportamiento, campos, recordatorios y correo. |

En la parte superior verás cuánto llevas trabajado hoy, el botón para sincronizar ahora y el que muestra u oculta el cronómetro flotante.

### Atajos de teclado

| Dónde | Tecla | Acción |
|---|---|---|
| Panel | `1` `2` `3` `4` | Cambiar de sección. |
| Buscador del cronómetro | `↑` `↓` `Enter` | Elegir ticket. |
| Buscador del cronómetro | `Esc` | Cancelar. |
| Motivo de pausa | `Ctrl + Enter` | Pausar. |

---

## Qué escribe en ClickUp

Los nombres corresponden a la lista **Tickets Activos**. Si en tu equipo se llaman distinto, cámbialos en **Ajustes › Avanzado**.

| Acción | Campos y cambios en ClickUp |
|---|---|
| **Play** (primera vez) | «Fecha de Inicio» = ahora, solo si estaba vacía. Estado → «en desarrollo», si el ticket estaba por iniciar o reabierto. «Estado de Pausa» = «Sin pausa». Si el ticket está reabierto: «Fecha de Reabierto» = ahora, si estaba vacía. |
| **Pausa** | «Inicio Pausa N» (usa el primero libre entre 1 y 3). «Estado de Pausa» = «En pausa». El motivo se **agrega** al final de «Motivo de Pausa», con fecha y hora. |
| **Reanudar** | «Fin Pausa N». «Estado de Pausa» = «Cerrada». |
| **4.ª pausa en adelante** | ClickUp solo tiene 3 espacios de pausa, así que el inicio y el fin se registran como **comentario** en el ticket. |
| **Finalizar** | «Fecha Fin» = ahora. Estado → «listo para release». Comentario con el tiempo total y la cantidad de pausas. |
| **Cada tramo trabajado** | Registro de tiempo en el time tracking de ClickUp (tramos de 1 minuto o más). |

TicketPilot respeta los campos que alguien llenó a mano: no sobrescribe una «Fecha de Inicio» existente ni usa un espacio de pausa ocupado.

### Tickets compartidos («Shared with me»)

A veces te comparten un ticket suelto (por ejemplo, la subtarea de una reapertura) que vive en una carpeta a la que tu usuario no tiene acceso. En ClickUp aparece en **Shared with me**, pero la API de ClickUp **no lo devuelve** al pedir tus tareas asignadas.

Para traerlo:

1. En *Mis tickets*, pulsa **agregar por enlace**.
2. Pega la URL del ticket (`https://app.clickup.com/t/90133100517/TKM-20742`) o solo su código (`TKM-20742`).
3. Pulsa **agregar**.

El ticket aparece con la marca `[compartido]` y se recarga como cualquier otro. Se deja de seguir solo cuando lleva 30 días cerrado o pierdes el acceso. También puedes quitarlo con ✕ en su fila.

> Si la lista de ese ticket usa estados distintos (por ejemplo, no tiene «en desarrollo»), el cambio de estado puede fallar. El error aparece en **Sincronización** y el resto de campos se escribe igual.

---

## Campos pendientes

TicketPilot revisa qué campos **obligatorios** de cada ticket están vacíos.

- **Recordatorios:** en horario laboral te llega una notificación del sistema con los tickets en curso o reabiertos que tienen campos vacíos. La frecuencia y el horario se configuran en Ajustes.
- **Completar desde la app:** en *Pendientes* pulsa **completar**. Se abre un formulario con un control según el tipo de campo:

  | Tipo de campo | Cómo se llena |
  |---|---|
  | Texto | Caja de texto |
  | Número | Número |
  | Fecha | Selector, con el botón **ahora** |
  | Lista desplegable | Selector de opciones |
  | Etiquetas | Botones que se activan y desactivan |
  | Casilla | `[x]` |
  | Persona | «Yo» o un miembro del workspace |

  Los tipos que no se pueden editar desde la app tienen un enlace para abrirlos en ClickUp.
- **«No me aplica»:** hay campos obligatorios que llena otra área (soporte, QA, analítica). Márcalos con **no me aplica** en el formulario y no se te volverán a pedir en ningún ticket. Para revisarlos o reactivarlos, ve a **Ajustes › Campos que lleno yo**.

---

## Resumen diario por correo

Sin costo y sin servicios extra: TicketPilot envía el correo por SMTP con tu propia cuenta.

1. Ve a **Ajustes › Resumen diario por correo** y marca **Enviar el resumen cada día**.
2. Llena los datos según tu proveedor:

   | Proveedor | Servidor | Seguridad / Puerto | Contraseña |
   |---|---|---|---|
   | Gmail o Google Workspace | `smtp.gmail.com` | STARTTLS · 587 | Contraseña de aplicación (ver abajo) |
   | Outlook / Microsoft 365 | `smtp.office365.com` | STARTTLS · 587 | Tu contraseña o una de aplicación |
   | Correo corporativo | El que te indique TI | Normalmente STARTTLS · 587 o TLS · 465 | La que te indique TI |

3. En **Enviar a** escribe uno o varios correos, separados por coma.
4. Elige la **hora de envío** y pulsa **Enviar resumen ahora** para probarlo.

**Contraseña de aplicación de Gmail:**
1. Entra a [myaccount.google.com](https://myaccount.google.com) › **Seguridad**.
2. Activa la **Verificación en dos pasos**, si no la tienes.
3. Busca **Contraseñas de aplicaciones** y crea una llamada «TicketPilot».
4. Copia los 16 caracteres en el campo **Contraseña de aplicación**. Queda guardada en el llavero del sistema.

El correo trae el tiempo trabajado hoy por ticket, los contadores y los campos pendientes. La app debe estar abierta, aunque sea en la bandeja, a la hora de envío. Si el equipo estaba apagado a esa hora, el correo sale apenas abras la app ese mismo día.

---

## Ajustes

| Sección | Qué puedes cambiar |
|---|---|
| Cuenta de ClickUp | Conectar o desconectar el token. Elegir el workspace, si tienes varios. Abrir la app al iniciar sesión. |
| Play, Pausa y Stop | El estado al iniciar y al finalizar. Pedir motivo al pausar. Abrir el ticket en el navegador al dar Play. |
| Campos que lleno yo | Qué campos obligatorios te aplican a ti. |
| Jornada laboral | Días que trabajas con su hora de entrada y de salida. Pausar al fin de turno (solo en la app, sin registrar pausa en ClickUp). |
| Recordatorios | Cada cuántos minutos (0 = apagados) y cada cuánto recargar los tickets. Solo llegan dentro de la jornada. |
| Resumen por correo | Servidor, cuenta, destinatarios y hora. |
| Avanzado | Los nombres exactos de los campos en ClickUp. |

Si cambias algo, aparece abajo la barra **guardar ajustes**.

---

## Sin conexión y sincronización

**El tiempo se guarda primero en tu equipo.** Cada Play, Pausa, Finalizar o campo completado queda registrado en una base de datos local y luego se envía a ClickUp en orden.

- **Si se cae internet:** el cronómetro sigue funcionando y los cambios esperan en la cola.
- **Al volver la conexión:** se envían solos, con reintentos cada vez más espaciados.
- **Si cierras o reinicias el equipo:** al abrir la app, el ticket sigue donde lo dejaste.
- **Si ClickUp rechaza un cambio** (por ejemplo, porque alguien renombró un campo): aparece en **Sincronización** con el motivo. Desde ahí puedes **reintentar** o **descartar** el cambio.

---

## Solución de problemas

| Problema | Qué hacer |
|---|---|
| «El token de ClickUp no es válido o expiró» | Genera un token nuevo en ClickUp y pégalo en Ajustes. |
| Faltan tickets en la lista | La lista se recarga cada 5 minutos; pulsa ⟳ para forzarla. Revisa que el ticket esté asignado a ti y que no esté cerrado hace más de 30 días. |
| Me asignaron un ticket y no aparece (está en «Shared with me») | ClickUp no incluye en «asignadas a mí» las tareas que te comparten sueltas desde una carpeta a la que no tienes acceso. Usa **agregar por enlace** en *Mis tickets* y pega la URL o el código (`TKM-20742`). Ver [Tickets compartidos](#tickets-compartidos-shared-with-me). |
| Un ticket no muestra sus campos obligatorios | Puede estar en una carpeta a la que tu usuario no tiene acceso. TicketPilot usa entonces los campos que trae el propio ticket; pide acceso a esa carpeta si necesitas más. |
| «La lista no tiene el campo …» | Alguien renombró el campo en ClickUp. Pon el nombre nuevo en **Ajustes › Avanzado** y luego **reintentar** en Sincronización. |
| «El campo … no tiene la opción …» | Cambió una opción de lista desplegable, por ejemplo en «Estado de Pausa». Revisa los nombres en ClickUp. |
| No llegan las notificaciones | Revisa que el sistema operativo permita notificaciones de TicketPilot y que estés dentro de la jornada laboral configurada. |
| El correo falla al enviar | Con Gmail, usa una **contraseña de aplicación**, no tu contraseña normal. Revisa servidor y puerto. |
| (Linux) No guarda el token | Instala y desbloquea un llavero: `sudo apt install gnome-keyring`. |
| No encuentro la ventana | Busca el ícono en la bandeja del sistema: clic izquierdo abre el panel y el clic derecho muestra el menú. |

### Dónde se guardan los datos

| Qué | Linux | macOS | Windows |
|---|---|---|---|
| Ajustes (`settings.json`) | `~/.config/co.tickets-ne.ticketpilot/` | `~/Library/Application Support/co.tickets-ne.ticketpilot/` | `%APPDATA%\co.tickets-ne.ticketpilot\` |
| Tiempos y cola (`ticketpilot.sqlite3`) | `~/.local/share/co.tickets-ne.ticketpilot/` | igual que arriba | igual que arriba |
| Token y contraseña SMTP | Llavero del sistema | Keychain | Credential Manager |

Para empezar de cero, cierra la app, borra esas carpetas y desconecta la cuenta.

---

## Para desarrolladores

### Tecnologías

| Capa | Tecnología |
|---|---|
| Escritorio | [Tauri 2](https://tauri.app) (Rust). Binario de ~10 MB y poca RAM; ideal para una ventana siempre abierta. |
| Interfaz | React 18 + TypeScript + Vite |
| Componentes | [Radix UI](https://www.radix-ui.com) (accesibilidad y teclado) con **CSS nativo**. Sin Tailwind ni Bootstrap. |
| Íconos y fuentes | `lucide-react`, IBM Plex Mono y VT323 (incluidas en la app; no se cargan de internet) |
| Datos locales | SQLite embebido (modo WAL) |
| ClickUp | API REST v2 oficial |
| Correo | SMTP con `lettre` (rustls) |
| Secretos | Llavero del sistema operativo (`keyring`) |

### Requisitos

| Sistema | Instalar |
|---|---|
| Todos | [Node.js 20+](https://nodejs.org) y Rust estable |
| Ubuntu / Debian | `sudo apt install build-essential libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev libxdo-dev` |
| Fedora | `sudo dnf install webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel dbus-devel libxdo-devel` y `sudo dnf group install "c-development"` |
| macOS | `xcode-select --install` |
| Windows | [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) con «Desarrollo para el escritorio con C++». WebView2 ya viene en Windows 10 y 11. |

Para instalar Rust (Linux y macOS):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

En Windows, usa el instalador de [rustup.rs](https://rustup.rs).

> La primera compilación descarga y compila unas 600 dependencias. Necesita entre 3 y 5 GB libres y tarda varios minutos. Las siguientes son mucho más rápidas.

### Ejecutar en desarrollo

```bash
npm install
npm run app:dev      # abre la app con recarga en caliente
```

Otros comandos útiles:

```bash
npm run typecheck                  # verificar tipos de TypeScript
npm run build                      # compilar solo el frontend
cd src-tauri && cargo check        # verificar el backend en Rust
```

### Generar instaladores

```bash
npm run app:build
```

Los instaladores quedan en `src-tauri/target/release/bundle/`. Cada sistema genera **solo sus propios** instaladores: `.deb`, `.rpm` y `.AppImage` en Linux; `.dmg` y `.app` en macOS; `.msi` y `.exe` en Windows. Para generarlos todos desde un mismo repositorio, usa GitHub Actions con [`tauri-apps/tauri-action`](https://github.com/tauri-apps/tauri-action).

### Arquitectura

```
┌────────── Ventanas (React + Radix + CSS nativo) ──────────┐
│  widget: cronómetro flotante     main: panel completo     │
└───────────────▲──────── invoke() / evento "snapshot" ─────┘
                │
┌───────────────┴──────────────── Rust (Tauri) ─────────────┐
│ tracker   Play/Pausa/Stop = transacción SQLite + outbox   │
│ sync      bucle cada 15 s: envía el outbox → recarga      │
│           tickets → recordatorios → correo diario         │
│ fields    formulario de campos y "no me aplica"           │
│ clickup   cliente de la API v2                            │
│ db        SQLite: sesiones, tramos, pausas, outbox        │
│ secrets   token y contraseña SMTP en el llavero del SO    │
│ mail      resumen diario por SMTP                         │
└───────────────────────────────────────────────────────────┘
```

**Principios:**

- **Patrón outbox:** cada acción se guarda localmente en una transacción junto con las escrituras pendientes hacia ClickUp. Un bucle las envía en orden por ticket, con reintentos exponenciales; los errores permanentes quedan visibles para el usuario.
- **Snapshot:** Rust arma una «foto» del estado y la envía a ambas ventanas con el evento `snapshot`. El frontend no guarda estado propio del negocio.
- **Cronómetro en el cliente:** Rust envía el tiempo cerrado y el inicio del tramo abierto; la interfaz suma los segundos en vivo, sin consultar el backend cada segundo.

### Estructura del proyecto

```
ticket-pilot/
├── index.html                 Entrada única; cada ventana carga su vista
├── src/                       Frontend
│   ├── main.tsx               Elige Widget o MainWindow según la ventana
│   ├── windows/
│   │   ├── Widget.tsx         Cronómetro flotante
│   │   └── MainWindow.tsx     Panel: encabezado, pestañas, atajos
│   ├── components/
│   │   ├── TicketsView.tsx    Panel «ahora», marcas y lista de tickets
│   │   ├── PendingView.tsx    Campos pendientes
│   │   ├── FillFieldsDialog.tsx  Formulario para completar campos
│   │   ├── SyncView.tsx       Cola y errores de sincronización
│   │   └── SettingsView.tsx   Ajustes
│   ├── ui/index.tsx           Componentes base (Button, Dialog, Select…)
│   ├── lib/api.ts             Tipos y llamadas a Rust
│   ├── lib/format.ts          Formato de horas y duraciones
│   └── styles/
│       ├── global.css         Colores, tipografía y componentes base
│       └── app.css            Maquetación de cada pantalla
└── src-tauri/                 Backend
    ├── tauri.conf.json        Ventanas, permisos e instaladores
    ├── capabilities/          Permisos de cada ventana
    ├── icons/                 Íconos de la app
    └── src/
        ├── lib.rs             Arranque, bandeja del sistema, ventanas
        ├── commands.rs        Comandos que llama el frontend
        ├── tracker.rs         Play / Pausa / Stop
        ├── sync.rs            Bucle de fondo, outbox y recordatorios
        ├── fields.rs          Completar campos y «no me aplica»
        ├── state.rs           Estado compartido y snapshot
        ├── db.rs              SQLite
        ├── clickup/           Cliente y modelos de la API
        ├── mail.rs            Resumen por correo
        ├── config.rs          Ajustes y sus valores por defecto
        ├── secrets.rs         Llavero del sistema
        └── error.rs           Errores que entiende el frontend
```

### Personalizar el aspecto

La interfaz imita una terminal de fósforo monocromático sobre negro.

- **Color:** en `src/main.tsx`, la línea `document.documentElement.dataset.phosphor = "green";` activa el verde. Bórrala para usar ámbar. Los tonos de cada fósforo están al inicio de `src/styles/global.css`; para crear otro (por ejemplo blanco), copia el bloque `[data-phosphor="green"]` y cambia sus cinco colores.
- **Tipografía:** IBM Plex Mono para la interfaz y VT323 solo para los cronómetros (`--font-mono` y `--font-crt` en `global.css`).
- **Animaciones:** se apagan solas si el sistema operativo tiene activado «reducir movimiento».

### Adaptar a otra lista de ClickUp

Los valores por defecto están en `src-tauri/src/config.rs`:

- `FieldNames`: nombres de los campos.
- `StatusNames`: estados.
- `ignored_fields`: campos que no aplican al desarrollador.

Cada usuario también puede cambiarlos desde **Ajustes › Avanzado**, sin recompilar.
