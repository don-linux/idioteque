# Feature: Iconos fijos en el footer

**Fecha:** 02/09/2026

## Descripción de la feature

Los iconos de la barra del footer del IDE dejan de ser arrastrables. El
orden lo define solo el código. El usuario ve cursor de clic (o el cursor
normal en Git, que no hace nada al clic).

## Implementado exitosamente

**1. Botones estáticos**

Casa, Carpeta, Engrane, Terminal y Git ya no se pueden reordenar desde
la UI. El orden es siempre casa → carpeta → engrane → terminal → git.

**2. Cursor correcto**

Al hover: pointer en las acciones. Default en Git. El clic no se confunde
con un arrastre.

**3. Sin persistencia de orden**

Se quitó `footer.actionOrder` de la config (frontend y Rust). Un
`config.json` de desarrollo que todavía traiga `footer` se ignora.

**4. DESIGN.md**

La sección del footer ya no habla de arrastrar ni de guardar el orden.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

- Cambiar qué hace cada icono
- Ocultar iconos o resetear orden desde la UI
- Reescribir a mano `~/.idioteque/config.json`
