# Feature: Historial de carpetas y footer

**Fecha:** 18/08/2026

## Descripción de la feature

Antes, al abrir idioteque solo había un botón para elegir una carpeta. Al cerrar
la app, se olvidaba. Había que volver a buscarla cada vez.

Ahora la pantalla de inicio es una grilla con el historial de las carpetas que
ya abriste. Clic y vuelves a entrar. Abajo, en todas las pantallas, queda
siempre visible la palabra “idioteque”.

Esa lista se guarda en un archivo de config que puedes copiar a otra máquina.

## Implementado exitosamente

**1. Grilla de historial al abrir la app**

Al arrancar ves cajas con todas las carpetas que has abierto (por ejemplo
`proyecto/.cursor`). Cada caja muestra el nombre de la carpeta y la ruta de
la carpeta padre.

Clic reabre esa carpeta. Hay una X para quitarla del historial.

Si la carpeta ya no existe en el disco, la caja se ve apagada y no abre. No
desaparece sola: sigue en la lista hasta que la quites.

**2. Abrir, volver e intercambiar carpetas**

Sigue existiendo “Abrir carpeta” para sumar una nueva.

Desde el editor hay “Inicio” para volver a la grilla y “Cambiar” para elegir
otra.

Abrir una carpeta la registra o la mueve al frente del historial. No se
duplica. El tope es 24.

**3. Footer sticky con el nombre de la app**

En la home y en el editor hay un pie siempre visible con la palabra
“idioteque” (así se escribe).

Si bajas en la grilla, el logo se queda abajo.

**4. Persistencia portable**

Las configs viven en `~/.idioteque/config.json`. Se crea al primer guardado.

- Linux: `/home/<user>/.idioteque/config.json`
- macOS: `/Users/<user>/.idioteque/config.json`
- Windows: `C:\Users\<user>\.idioteque\config.json`

Es un JSON que se puede copiar entre máquinas. No se usa el directorio de
config de Tauri ni un plugin de store.

Si el archivo no existe o está roto, la app arranca vacía y no se rompe.

**5. Pruebas**

Se escribieron pruebas del guardado, del historial (no duplicar, tope, quitar)
y de que las carpetas que ya no existen se quedan en la lista pero marcadas.

El typecheck del frontend pasó. Las pruebas de Rust pasaron.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- No se reabre sola la última carpeta al arrancar (la home es la grilla)
- No hay favoritos ni pines
- No hay sync automático entre máquinas
- No se usa SQLite ni plugin de store
