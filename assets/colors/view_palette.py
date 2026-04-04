import sys
import numpy as np
from PIL import Image
import colour
import plotly.graph_objects as go

if len(sys.argv) < 2:
  print("usage: python view_palette.py palette.png")
  sys.exit(1)

img = Image.open(sys.argv[1]).convert('RGB')
pixels = np.array(img).reshape(-1, 3) / 255.0

# colour.convert handles sRGB gamma decoding then converts to OKLab
oklab = colour.convert(pixels, 'sRGB', 'Oklab')
L = oklab[:, 0]
a = oklab[:, 1]
b = oklab[:, 2]

colors = ['rgb({},{},{})'.format(int(p[0]*255), int(p[1]*255), int(p[2]*255)) for p in pixels]

fig = go.Figure(data=[go.Scatter3d(
  x=a, y=b, z=L,
  mode='markers',
  marker=dict(size=8, color=colors, opacity=1.0),
  text=colors,
)])

fig.update_layout(
  title='{} colors from {}'.format(len(pixels), sys.argv[1]),
  scene=dict(
    xaxis_title='a  (green <- 0 -> red)',
    yaxis_title='b  (blue <- 0 -> yellow)',
    zaxis_title='L  (lightness)',
    aspectmode='data',
  )
)

fig.show()