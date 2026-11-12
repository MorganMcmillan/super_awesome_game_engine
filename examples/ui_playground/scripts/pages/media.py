# Page: MEDIA. An audio player (the host plays a generated tone through the
# mixer), a video player (a sprite sheet played at its frame rate), and an SVG
# icon drawn into a canvas.

import sage
from sage.ui import Label

ui = world.ui
T = world.theme
media_page = world.add_page("MEDIA")


def mcard(parent, title):
    c = ui.add(T.Card, parent)
    ui.add(T.Heading, c, text=title)
    return c


mcol_a, mcol_b = (ui.add(T.Column, media_page) for _ in range(2))

# -- audio ----------------------------------------------------------------------------
TONE_TICKS = 120  # two seconds at 60 ticks a second
audiocard = mcard(mcol_a, "AUDIO")
ui.add(T.Muted, audiocard, text="PLAY, PAUSE, SCRUB AND SET THE VOLUME.", wrap=True, width=ui.fill())
audio = ui.audio_player(audiocard, ui.asset("sounds/tone.wav"), TONE_TICKS)

# -- video ---------------------------------------------------------------------------------
videocard = mcard(mcol_a, "VIDEO")
ui.add(T.Muted, videocard, text="AN IMAGE SEQUENCE: FOUR FRAMES AT FOUR PER SECOND.", wrap=True, width=ui.fill())
video, video_picture = ui.video_player(videocard, "art/spinner.png", 4, 1, fps=4, width=48, height=48)

# -- svg -------------------------------------------------------------------------------------
svgcard = mcard(mcol_b, "SVG")
ui.add(T.Muted, svgcard, text="PATHS, SHAPES, FILL AND STROKE: AN ICON SET'S WORTH OF SVG.", wrap=True, width=ui.fill())
SVG = """
<svg viewBox="0 0 120 60">
  <rect x="2" y="2" width="116" height="56" rx="6" fill="#201B16" stroke="#3A332C"/>
  <g transform="translate(8 6)">
    <path d="M22 14 C22 2 2 2 2 16 C2 28 22 38 22 44 C22 38 42 28 42 16 C42 2 22 2 22 14 Z" fill="#F28C28"/>
  </g>
  <polygon points="85,8 89,20 102,20 92,28 96,41 85,33 74,41 78,28 68,20 81,20" fill="#4FC3C0" stroke="#FFFFFF"/>
  <path d="M52 48 A6 6 0 1 1 64 48 A6 6 0 1 1 52 48 Z" fill="#7BC96F"/>
  <g stroke="#FFFFFF" stroke-width="1.5" fill="none" opacity="0.8" transform="rotate(20 60 48)">
    <line x1="54" y1="48" x2="66" y2="48"/>
  </g>
</svg>
"""
icon = ui.svg(svgcard, SVG, alt="A HEART, A STAR AND A DOT", width=120, height=60)
