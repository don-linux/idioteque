# Feature: Icono de vida de Git en el footer

**Fecha:** 27/08/2026

## Descripción de la feature

En la barra de iconos del footer del IDE (solo con carpeta abierta, no en
home ni en configuración) hay un quinto icono: el de Git (ramas).

No abre nada. El clic no hace nada. Sirve para ver que el canal de Git
está vivo: al pasar el mouse, un tooltip dice si hay Git, si la carpeta
es un repo y, si lo es, el nombre del repo y la rama.

## Implementado exitosamente

**1. Quinto icono en el footer del IDE**

Aparece junto a Casa, Carpeta, Engrane y Terminal. Solo en `/workspace`.
Se puede arrastrar como los demás. Si el orden guardado era de cuatro
iconos, este queda al final.

**2. Tooltip al hover (el clic no hace nada)**

- "Git no está disponible" si no hay Git en el sistema
- "Sin repositorio Git" si la carpeta no es un repo
- "<nombre> · <rama>" si hay repo (nombre = carpeta del repo, no la ruta)
- "<nombre> · HEAD separado" si no estás en una rama
- "Git no responde" si la consulta falla

**3. Solo señal de vida**

No hay panel, ni stage, ni commit. El icono confirma que el canal de Git
funciona.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

- Panel de cambios
- Stage, commit, push
- Decoraciones del árbol
