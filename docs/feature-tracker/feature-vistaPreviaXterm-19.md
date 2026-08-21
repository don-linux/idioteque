# Feature: Vista previa real de xterm

**Fecha:** 21/08/2026

## Descripción de la feature

En Configuración → Terminal la vista previa ya no es un texto HTML.
Ahora usa xterm.js de verdad, igual que la terminal del IDE, pero
chiquita y solo para mirar.

Al elegir un tema, esa miniatura se recarga y se ve el color aplicado.
Si se cambia el tamaño de fuente, el prompt crece o achica al momento.
Arriba se sigue viendo la tira de 16 colores ANSI.

No se puede escribir ni hacer clic útil ahí. La terminal de verdad
sigue igual: hay que Guardar o Ctrl+S para que el cambio llegue al IDE.

## Implementado exitosamente

**1. xterm de verdad**

La miniatura pinta el prompt `❯ git status` con xterm.js y el tema
elegido, no con un párrafo disfrazado.

**2. Solo visual**

No acepta teclado ni clics. No está conectada al shell.

**3. Tema y tamaño en vivo**

Cambiar el tema recarga la miniatura. El tamaño de fuente se ve
al instante. La tira ANSI se queda.

**4. Compacta**

No copia el tamaño del panel del IDE. Cabe el prompt, sin recorte
ni un recuadro enorme.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Aplicar el tema al panel del IDE sin Guardar
- Usar el renderer WebGL en la miniatura
- Editar la paleta ANSI a mano
