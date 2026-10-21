# Render-inspection helper.
#
# The build host cannot view images, so the interface is checked as text: a
# coarse luminance grid, the ink bounds, the best text contrast, and whether
# anything is clipped at the window edge.
#
# Glyphs:  space = background,  . = surface,  * = mid tone,
#          # = bright,  @ = accent (amber).
Add-Type -Path (Join-Path $PSScriptRoot 'Inspect.cs') -ReferencedAssemblies System.Drawing, System.Drawing.Primitives
