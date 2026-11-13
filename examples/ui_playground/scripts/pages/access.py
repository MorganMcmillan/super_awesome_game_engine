# Page: ACCESS. What a screen reader is told: landmarks, headings, a labelled
# group, a separator, and an image with alt text. The tree it reads is printed
# by `ui_playground access --page access`.

import sage
from sage import world
from sage.ui import Label

ui = world.ui
T = world.theme
access_page = world.add_page("ACCESS")


def acard(parent, title):
    c = ui.add(T.Card, parent)
    ui.add(T.Heading, c, text=title)
    return c


acol_a, acol_b = (ui.add(T.Column, access_page) for _ in range(2))

# -- a small page made of landmarks ----------------------------------------------------
pagecard = acard(acol_a, "LANDMARKS")
site = ui.add(T.Card, pagecard)
site_header = ui.header(site, direction=ui.ROW, justify=ui.SPACE_BETWEEN, width=ui.fill())
ui.add(T.Title, site_header, text="CASTLE GAME")
site_nav = ui.nav(site_header, name="SITE", direction=ui.ROW, gap=4)
nav_play = ui.add(T.Ghost, site_nav, text="PLAY")
nav_help = ui.add(T.Ghost, site_nav, text="HELP")
ui.separator(site)
site_main = ui.main(site, direction=ui.COLUMN, gap=4, width=ui.fill())
settings = ui.section(site_main, "SETTINGS", direction=ui.COLUMN, gap=3, width=ui.fill())
audio_group, audio_body = ui.fieldset(settings, "AUDIO", width=ui.fill())
sound_row = ui.add(T.Row, audio_body)
sound_switch = ui.add(T.Switch, sound_row)
sound_label = ui.add(T.Body, sound_row, text="SOUND")
ui.role(sound_label, "label", label_for=sound_switch)
volume_title = ui.add(T.Body, audio_body, text="VOLUME")
volume_slider = ui.add(T.Track, audio_body)
ui.role(volume_slider, None, labelled_by=volume_title)
site_footer = ui.footer(site, width=ui.fill())
ui.add(T.Muted, site_footer, text="MADE WITH SAGE")

# -- an image, a decoration and a description ----------------------------------------------
imgcard = acard(acol_b, "ALT TEXT")
ui.add(T.Muted, imgcard, text="A PICTURE NEEDS A DESCRIPTION. A DECORATION SAYS NONE AND IS SKIPPED.", wrap=True, width=ui.fill())
hero = ui.image(imgcard, "art/logo.png", alt="A GOLD STAR", width=16, height=16)
deco = ui.image(imgcard, "art/logo.png", alt="", width=16, height=16)
described = ui.add(T.Badge, imgcard, text="SCORE")
ui.role(described, None, description="YOUR POINTS SO FAR")
howcard = acard(acol_b, "READ IT")
ui.add(T.Para, howcard, text="RUN  ui_playground access --page access  TO SEE THE TREE A SCREEN READER GETS: ROLES, NAMES, STATES.")
