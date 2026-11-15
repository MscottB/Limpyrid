# Genera las imagenes del instalador NSIS (BMP de 24 bits). Se puede repetir: sobrescribe los dos archivos.
# Uso: powershell -File scripts/imagenes-instalador.ps1
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

# Medidas del esquema de Tauri (NsisConfig): cabecera 150x57, lateral 164x314.
$CabW = 150; $CabH = 57; $CabIcono = 45
$LatW = 164; $LatH = 314; $LatIcono = 96; $LatY = 36
$Fondo = [System.Drawing.ColorTranslator]::FromHtml('#0B1220') # --bg de src/app.css

$raiz = Split-Path -Parent $PSScriptRoot
$icono = [System.Drawing.Image]::FromFile((Join-Path $raiz 'src-tauri/icons/128x128.png'))
$salida = Join-Path $raiz 'src-tauri/windows'
New-Item -ItemType Directory -Force $salida | Out-Null

function Nueva($archivo, $w, $h, $lado, $y) {
  $bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.Clear($Fondo)
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  # El PNG trae transparencia: se compone sobre el fondo oscuro, sin blanco alrededor.
  $g.DrawImage($icono, [int](($w - $lado) / 2), $y, $lado, $lado)
  $g.Dispose()
  $bmp.Save((Join-Path $salida $archivo), [System.Drawing.Imaging.ImageFormat]::Bmp)
  $bmp.Dispose()
}

Nueva 'cabecera.bmp' $CabW $CabH $CabIcono ([int](($CabH - $CabIcono) / 2))
Nueva 'lateral.bmp' $LatW $LatH $LatIcono $LatY
$icono.Dispose()
