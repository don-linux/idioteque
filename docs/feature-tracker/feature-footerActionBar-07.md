# Feature: Barra de acciones en el footer

**Fecha:** 20/08/2026

## Descripción de la feature

Antes, con una carpeta abierta, el sidebar tenía tres controles: el engrane
de Configuración, el botón “Inicio” y el botón “Cambiar”. El footer solo
tenía la palabra “idioteque” a la izquierda y el icono de terminal a la
derecha.

Ahora esos tres se suman a la barra inferior como iconos, junto al de
terminal. El sidebar del IDE solo muestra la ruta de la carpeta.

## Implementado exitosamente

**1. Los tres controles pasan al footer**

En el IDE, Inicio, Cambiar y Configuración ya no viven en el sidebar. Van
abajo, como iconos, al lado del de terminal. El sidebar queda solo con la
ruta de la carpeta abierta.

**2. Orden por defecto (sin texto, con tooltip)**

1. Casa — Inicio (vuelve a la grilla de carpetas; eso apaga la terminal)
2. Carpeta — Cambiar (abre el selector nativo)
3. Engrane — Configuración (va a `/configuracion` sin cerrar el workspace,
   para no apagar la terminal)
4. Terminal — igual que antes (clic = abajo, Alt+clic = derecha, queda
   marcado si está abierta)

**3. Se pueden reordenar y se recuerda**

Puedes arrastrar los iconos para cambiar el orden. Ese orden se guarda en
`~/.idioteque/config.json` (junto al historial y la fuente de la terminal)
y vuelve a salir igual.

Si más adelante se agregan iconos nuevos, aparecen al final sin desarmar el
orden guardado.

**4. Home y Configuración no cambian**

La pantalla de selección de carpetas sigue con el engrane en el header y
“Abrir carpeta”. Configuración tampoco cambió de UI.

**5. Decisiones de diseño**

Se escribió `docs/DESIGN.md` con las decisiones de diseño (paleta, home,
IDE, footer, configuración, terminal).

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Footer o iconos en la home o en Configuración
- Ocultar iconos, botón de resetear orden, reordenar con teclado
- Guardar tamaño o lado del panel de terminal
