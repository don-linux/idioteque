# Feature: Paridad de temas UI y terminal

**Fecha:** 16/09/2026

## Descripción de la feature

Los selectores de Temas y de Terminal ahora ofrecen los mismos nombres.
Quien quiera el mismo tema en la interfaz y en la terminal puede
elegirlo en ambos lados. Quien quiera uno distinto en cada lado, también:
la elección sigue siendo independiente.

Se sumaron las caras que faltaban y un tema nuevo, One Dark Pro, en los
dos lados.

## Implementado exitosamente

**1. Mismos nombres en ambos selectores**

Hay quince temas, con el mismo id y el mismo label en la UI y en la
terminal. El grafo de Git viaja con el tema de la interfaz, como ya
hacía.

**2. Lo que faltaba de un lado, ahora está en el otro**

En la terminal aparecen Idioteque Dark, Idioteque Night, Idioteque Light,
Platzi, Everforest Dark y One Dark. En la UI aparecen One Half Dark,
Dracula y Campbell.

**3. One Dark, One Half Dark y One Dark Pro son tres temas**

No son el mismo con otro nombre. One Dark sigue siendo Atom. One Half
Dark se distingue por el texto más claro de Windows Terminal. One Dark
Pro trae el chrome y el ANSI de Binaryify/OneDark-Pro.

**4. La elección no se unió**

Cambiar el tema de la UI no cambia el de la terminal, ni al revés. Se
siguen guardando aparte.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Un botón o atajo que copie el tema de un lado al otro
- Fusionar los HEX de UI y terminal en un solo objeto
