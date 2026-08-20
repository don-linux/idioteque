# Feature: Guardado manual

**Fecha:** 20/08/2026

## Descripción de la feature

El editor ya no guarda solo al escribir. Hay que pulsar Ctrl+S o el icono
de disquete. Así el archivo en disco no cambia hasta que uno lo decide.

Si se cambia de archivo sin guardar, el texto se queda en memoria. Al
volver, sigue ahí. El disco no se toca hasta Ctrl+S.

Si se cierra la app, se va a Inicio o se cambia de carpeta con algo sin
guardar, sale un aviso: esos cambios no se pueden recuperar. Cancelar se
queda. Salir descarta.

## Implementado exitosamente

**1. Guardar a propósito**

Hay que pulsar Ctrl+S o el icono de disquete. Escribir ya no escribe el
archivo en disco.

**2. El disquete en el footer**

Aparece a la derecha de la wordmark “idioteque”, solo cuando el archivo
actual tiene cambios sin guardar. Al guardar, desaparece. Al editar o
volver a un archivo sucio, reaparece.

**3. Cambiar de archivo sin perder el texto**

Si se cambia de archivo sin guardar, el texto se queda en memoria. Al
volver, sigue ahí. El disco no se toca hasta Ctrl+S.

**4. Aviso al salir con cambios**

Si se cierra la app, se va a Inicio o se cambia de carpeta con algo sin
guardar, sale un modal. Avisa que esos cambios no se pueden recuperar.
Cancelar se queda. Salir descarta.

## NO se pudo implementar

Nada. Todo lo previsto quedó hecho.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Indicador en el árbol de archivos
- Guardar todos los archivos a la vez
- Persistir borradores al cerrar del todo (el modal es la protección)
