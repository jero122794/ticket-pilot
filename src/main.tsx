import { StrictMode, Suspense, lazy } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { TooltipProvider } from "./ui";
import "./styles/global.css";
import "./styles/app.css";

// Cada ventana carga solo su código: el widget arranca sin el panel completo.
const Widget = lazy(() => import("./windows/Widget"));
const MainWindow = lazy(() => import("./windows/MainWindow"));

// Color del fósforo: "green" o quitar la línea para volver al ámbar (ver global.css).
document.documentElement.dataset.phosphor = "green";

const View = getCurrentWindow().label === "widget" ? Widget : MainWindow;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <TooltipProvider>
      <Suspense fallback={null}>
        <View />
      </Suspense>
    </TooltipProvider>
  </StrictMode>,
);
