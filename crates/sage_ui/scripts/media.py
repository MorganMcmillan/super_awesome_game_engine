# Media players: transport controls (play and pause, a scrubber, the time, a
# volume), an audio player and an image-sequence video player. The state is
# the native `Media` component, so a save game holds it and the host reads it
# to drive audio. Playback advances one tick per tick, the same on every run.

import sage
from sage import Entity, component
from sage.ui import Changed, Clicked, Image, Label, Media, Slider

ui = world.ui
TICKS_PER_SECOND = 60


@component
class MediaParts:
    """The controls of a player: its play button, scrubber, time label and
    volume slider."""

    play: sage.u64 = 0
    scrub: sage.u64 = 0
    time: sage.u64 = 0
    volume: sage.u64 = 0


@component
class VideoOf:
    """A player that shows an image sequence: the image and how it plays."""

    image: sage.u64 = 0
    frames: sage.u32 = 0
    fps: sage.u32 = 12


ui.MediaControls = world.prefab("UiMediaControls", base=ui.Panel)
ui.PlayButton = world.prefab("UiPlayButton", base=ui.Button)
ui.Scrubber = world.prefab("UiScrubber", base=ui.Slider)
ui.Volume = world.prefab("UiVolume", base=ui.Slider)


def _clock(ticks):
    seconds = int(ticks) // TICKS_PER_SECOND
    return "%d:%02d" % (seconds // 60, seconds % 60)


def media_controls(parent, length, source=0, volume=1.0, looping=False, **style):
    """Play and pause, a scrubber, the time and a volume. `length` is in
    ticks (60 to a second). Returns the root; read `ui.media_state(root)`. The
    `Media` component on it is what a host binds to audio."""
    root = ui.add(ui.MediaControls, parent, direction=ui.ROW, gap=4, align=ui.CENTER, width=ui.fill(), **style)
    play = ui.add(ui.PlayButton, root, text="PLAY", width=36)
    scrub = ui.add(ui.Scrubber, root, width=ui.fill())
    scrub.set(Slider(0.0, 0.0, float(length), 1.0))
    time = ui.add(ui.Text, root, text="%s / %s" % (_clock(0), _clock(length)))
    vol = ui.add(ui.Volume, root, width=30)
    vol.set(Slider(float(volume), 0.0, 1.0, 0.0))
    root.set(Media(int(source), 0, 0.0, float(length), float(volume), 1 if looping else 0))
    root.set(MediaParts(play.sid, scrub.sid, time.sid, vol.sid))
    ui.role(root, "group", name="MEDIA PLAYER")
    ui.role(play, "button", name="PLAY OR PAUSE")
    ui.role(scrub, "slider", name="POSITION")
    ui.role(vol, "slider", name="VOLUME")
    return root


def audio_player(parent, source, length, **style):
    """Controls for a sound the host has (`source` is its asset id,
    `ui.asset(path)`); `length` is its length in ticks."""
    return media_controls(parent, length, source=source, **style)


def video_player(parent, image, cols, rows=1, fps=12, width=64, height=36, looping=True, **style):
    """A video as a sprite sheet: `cols` x `rows` frames of the image `image`,
    played at `fps`, under media controls. Returns `(root, picture)`. (Real
    video decoding is not built; an image sequence needs no dependency.)"""
    box = ui.add(ui.Panel, parent, direction=ui.COLUMN, gap=3, width=ui.fill())
    picture = ui.add(ui.Image, box, image=image, cols=cols, rows=rows, width=width, height=height, fit="contain")
    frames = cols * rows
    length = frames * TICKS_PER_SECOND // fps
    root = media_controls(box, length, looping=looping, **style)
    root.set(VideoOf(picture.sid, frames, fps))
    ui.role(picture, "img", name="VIDEO")
    return root, picture


def media_state(root):
    """`(playing, position, length, volume)` of a player."""
    m = root.get(Media)
    return bool(m.playing), m.position, m.length, m.volume


def media_play(root, playing=True):
    m = root.get(Media).copy()
    if playing and m.position >= m.length:
        m.position = 0.0
    m.playing = 1 if playing else 0
    root.set(m)


def media_seek(root, ticks):
    m = root.get(Media).copy()
    m.position = max(0.0, min(m.length, float(ticks)))
    root.set(m)


ui.media_controls, ui.audio_player, ui.video_player = media_controls, audio_player, video_player
ui.media_state, ui.media_play, ui.media_seek = media_state, media_play, media_seek


@world.observer(Clicked)
def _ui_media_play(c: Clicked):
    e = world.lookup(c.target)
    if e is None:
        return
    root = e.parent
    if root is None or not root.has(MediaParts) or root.get(MediaParts).play != e.sid:
        return
    media_play(root, not root.get(Media).playing)


@world.observer(Changed)
def _ui_media_sliders(c: Changed):
    e = world.lookup(c.target)
    if e is None:
        return
    root = e.parent
    if root is None or not root.has(MediaParts):
        return
    parts = root.get(MediaParts)
    if parts.scrub == e.sid:
        media_seek(root, c.value)
    elif parts.volume == e.sid:
        m = root.get(Media).copy()
        m.volume = c.value
        root.set(m)


@world.system(phase=sage.PreUpdate)
def _ui_media_advance(e: Entity, m: Media, p: MediaParts):
    if m.playing:
        m.position += 1.0
        if m.position >= m.length:
            if m.looping:
                m.position = 0.0
            else:
                m.position = m.length
                m.playing = 0
    # Show the state: the scrubber follows the position, the button says what
    # a press does, the time reads in whole seconds.
    scrub = world.lookup(p.scrub)
    if scrub.get(Slider).value != m.position:
        s = scrub.get(Slider).copy()
        s.value = m.position
        scrub.set(s)
    play = world.lookup(p.play)
    want = "PAUSE" if m.playing else "PLAY"
    if world.string(play.get(Label).text) != want:
        play.set(Label(world.intern(want)))
    clock = "%s / %s" % (_clock(m.position), _clock(m.length))
    time = world.lookup(p.time)
    if world.string(time.get(Label).text) != clock:
        time.set(Label(world.intern(clock)))
    if e.has(VideoOf):
        v = e.get(VideoOf)
        img = world.lookup(v.image)
        frame = int(m.position * v.fps / TICKS_PER_SECOND) % max(1, v.frames)
        cur = img.get(Image)
        if cur.frame != frame:
            nxt = cur.copy()
            nxt.frame = frame
            img.set(nxt)

ui.MediaParts = MediaParts
