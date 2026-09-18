# Feature: Navegador CEF

**Fecha:** 17/09/2026

## Descripción de la feature

El IDE tenía dos superficies: el editor y el canvas de terminales.
Faltaba un navegador de verdad para ver una página, una app en
desarrollo o la documentación publicada sin salir de idioteque, y que
no dependiera del webview de la app.

Ahora hay una tercera superficie. `Ctrl+B` abre Chromium a pantalla
completa dentro de la ventana y otro `Ctrl+B` devuelve a donde estabas.
El motor viene de fábrica en el instalador y se actualiza solo mientras
las versiones nuevas sigan siendo compatibles con la app.

## Implementado exitosamente

**1. Tercera superficie: el navegador**

`Ctrl+B` abre el navegador ocupando todo el cuerpo de la ventana; el
footer se queda. Desde la terminal es `Ctrl+Shift+B`, porque `Ctrl+B`
es el prefijo de tmux. Otro `Ctrl+B` vuelve a la superficie anterior,
sea el editor o las terminales.

El árbol de archivos cambió de atajo: pasa de `Ctrl+B` a `Ctrl+T`
(`Ctrl+Shift+T` desde la terminal). En el footer hay un icono de globo
nuevo que también abre y cierra el navegador, y queda marcado mientras
está abierto.

**2. Chromium de verdad**

No es el webview de Tauri. Es Chromium vía CEF (Chromium Embedded
Framework): animaciones, JavaScript, scroll y DevTools funcionan tal
cual Chromium.

Lo corre un proceso aparte, `cef-host` (Rust, crate `cef` 152.3.0), que
se embebe como ventana hija X11 dentro de la ventana de idioteque. Si
ese proceso muere, la app sigue.

**3. Barra propia de idioteque**

Arriba va una barra de idioteque, no el chrome de Chromium: atrás,
adelante, recargar o parar, la URL, DevTools y una cruz para cerrar.
DevTools abre en su ventana propia de Chromium (F12 o `Ctrl+Shift+I`).

Hay una sola pestaña; los popups se abren en ella. Ocultar el navegador
no mata el proceso ni la página. Volver a Inicio sí.

**4. El motor viene de fábrica**

Cada release trae un CEF de fábrica bundleado en el instalador (hoy
152.0.6 / Chromium 152.0.7977.83). El usuario no descarga nada al
instalar.

**5. El motor se actualiza solo**

Encima del base hay un updater. Baja la stable más nueva del índice
oficial de CEF a `~/.idioteque/cef/candidate`, verifica el sha1,
extrae, stripea `libcef.so` (de 1.4 GB a 268 MB) y comprueba que la
API sea compatible con esta versión de idioteque.

Después hace un health check: arranca `cef-host` sin ventana y carga
`about:blank`. Si pasa, promueve el candidato a `current` de forma
atómica. Si no, `current` se queda como está, la versión se apunta en
una denylist para no volver a intentarla y la app avisa.

**6. Avisos**

Cuando actualiza: "Se ha actualizado a Chromium XXX".

Cuando no puede: "Se intentó actualizar a Chromium XXX, no es
compatible con esta versión de idioteque. Chromium continuará en YYY.
Puedes abrir un issue reportando: …", con un enlace que abre el issue
ya rellenado.

Mientras el navegador está visible los avisos esperan a que se cierre,
porque la ventana de Chromium los taparía.

**7. Configuración → Navegador**

Sección nueva. Muestra el Chromium actual, el base de fábrica, la
última comprobación, las versiones descartadas y un botón "Buscar
actualización". No hay nada que guardar: es información y un botón.

**8. Pipeline y documentación**

`bun run cef:prepare` compila el sidecar y prepara el base pinneado en
`src-tauri/cef/base.json`. Un test exige que ese base coincida con la
versión del crate `cef`, así no se puede subir uno sin el otro.

El detalle está en `docs/cef/CONTRACT.md` (el contrato entre las
piezas) y `docs/CEF-RUNTIME.md` (cómo vive el motor en disco y cómo
migrar el base en un release). `docs/DESIGN.md` lo recoge en la sección
"Navegador".

**9. Pruebas**

428 tests de frontend y 245 de Rust en verde.

Probado a mano en la VM: páginas con animaciones, DevTools, ida y
vuelta con `Ctrl+B`, un update real de 151 a 152 (descarga real, health
check, promoción y aviso) y un update falso incompatible (denylist y
aviso).

## NO se pudo implementar

- El embebido es Linux/X11 (`linux64`). En una sesión Wayland la app
  corre sobre XWayland con `GDK_BACKEND=x11`.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Pestañas múltiples
- DevTools acoplado dentro de la vista (abre en ventana propia)
- Canal beta del updater
- Guardar la URL entre sesiones
