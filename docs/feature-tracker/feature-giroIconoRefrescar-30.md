# Feature: Giro del icono de refrescar

**Fecha:** 13/09/2026

## Descripción de la feature

El botón “Refrescar” de la toolbar del árbol de archivos ya se pintaba
con el color de acento mientras recargaba, pero el icono se quedaba
quieto. Ahora gira sobre sí mismo, como el spinner de toda la vida.

El detalle está en que recargar el árbol tarda milisegundos: si la
animación durara solo lo que dura el trabajo real, sería un parpadeo
que nadie alcanza a ver. Por eso el giro se sostiene un rato.

Las reglas de tiempo son estas:

- Una vuelta completa dura 800 ms.
- Siempre se dan al menos dos vueltas, así que el giro dura ~1.6 s
  aunque el refresco termine al instante.
- Si leer el disco tarda más que ese mínimo, el icono sigue girando
  hasta cerrar la vuelta en la que iba. Nunca se corta a medio giro.
- Si se vuelve a hacer clic mientras gira, el clic se ignora: no se
  encolan refrescos ni se reinicia la animación.
- Con las animaciones reducidas del sistema activadas, el icono no
  gira. Queda el color de acento, que ya avisaba que estaba trabajando.

## Implementado exitosamente

**1. El icono gira**

El `<svg>` del botón rota sobre su centro mientras se recarga el
árbol, a velocidad constante y en bucle.

**2. El giro dura lo suficiente para verse**

Hay un mínimo de dos vueltas. Un refresco instantáneo igual se ve como
una animación y no como un pestañeo.

**3. El giro termina en vuelta completa**

Si el refresco tarda más del mínimo, el giro se extiende hasta el
siguiente múltiplo de la vuelta. El icono siempre queda derecho.

**4. Los clics repetidos no rompen nada**

Mientras gira, el botón no vuelve a disparar el refresco. La animación
no se corta ni se reinicia a mitad de camino.

**5. Respeta las animaciones reducidas**

Con `prefers-reduced-motion` en el sistema no hay giro, solo el color
de acento de siempre.

**6. Un solo número para la duración**

La duración de la vuelta se define una vez y el botón se la pasa al
CSS, así no queda el mismo 800 escrito en dos sitios que se puedan
desincronizar.

**7. Tests y documentación**

Se sumaron seis pruebas para el cálculo del tiempo de giro (refresco
instantáneo, rápido, justo en el mínimo, lento, muy lento y un caso
raro con tiempo negativo). El comportamiento quedó anotado también en
`docs/DESIGN.md`, junto a la descripción de la toolbar del árbol.

Se verificó con `bun run check` (sin errores ni avisos), `bun run test`
(331 pruebas en verde) y `bun run build`.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

Vale anotar que este es el único botón de refrescar visible de la app.
El refresco de Git del footer y el del grafo son automáticos —al
enfocar la ventana o al pasar el mouse— y no tienen botón propio, así
que no había nada más que animar.
