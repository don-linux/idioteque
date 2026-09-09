Integra un visor de las ramas de Git en el proyecto

Ya tenemos una implementacion de Git, actualmente permite detectar si hay un repo, y nos muestra un par de datos al hacer hoover sobre el icono

Quiero hacer una implementacion escalonada de mi idea

La visualizacion de Git debe tener dos vistas, o dos formas de visualizacion

En este alcance solo haremos la primer vista

En el mismo espacio del layout donde actualmente, se muestra el arbol de archivos, se debera poder cambiar ese mismo espacio, por un grafico de las ramas de Git, al estilo Visual Studio Code

Es decir, se podra hacer un flip de esta parte del layout, con los comandos que se detallan posteriormente, conservando las caracteristicas actuales del espacio del layout del arbol de archivos, como el redimensionamiento correcto y ocultamiento correcto

Esta vista debe desplegarse al hacer clic en el icono de git del footer, o usando el shortcut Ctrl + G

Se muestra intercambia la vista del arbol de archivos, por la vista del grafico de la rama actual de Git

Este grafico debe mostrar los nombres de los commits, entre parentesis una pequeña parte del hash de los commits

Por defecto e idealmente, la rama actual de Git, sera que la veremos representado en el grafico 

Pero con la opcion de poder habilitar la visualizacion del resto de ramas

Para salir de este grafico podemos ocultarlo con Ctrl + G o si presionamos Ctrl + B, abririamos el arbol de archivos, cualquiera de las dos formas 


Digamos , con un shortcut se muestra el arbol de archivos y con otro shortcut se muestra el grafico de Git