import math, html, sys
sys.path.insert(0, '.')
from rebuild_main import icon as _icon, INNER, F, pill, donut, flat
COL = {'track': '{{c.track}}', 'r1': '{{c.r1}}', 'r2': '{{c.r2}}', 'r3': '{{c.r3}}', 'blocked': '{{c.blocked}}', 'done': '{{c.done}}', 'error': '{{c.error}}'}
G, PANE, RAISED, OVER, QUIET, HOVER = '{{c.ground}}', '{{c.pane}}', '{{c.raised}}', '{{c.over}}', '{{c.quiet}}', '{{c.hover}}'
INK, INK2, MUTED, ACC, TINT = '{{c.ink}}', '{{c.ink2}}', '{{c.muted}}', '{{c.acc}}', '{{c.tint}}'
ST = {'blocked': '{{c.blocked}}', 'done': '{{c.done}}', 'error': '{{c.error}}', 'bell': '{{c.blocked}}'}
RAMP = '{{c.ramp}}'
RAMP50 = '{{c.ramp50}}'
UI = "-apple-system, BlinkMacSystemFont, 'Inter', sans-serif"
MONO = "'JetBrains Mono', ui-monospace, monospace"
esc = lambda t: html.escape(t, quote=False)
def ic(key, colour=INK2, size=16): return _icon(key, colour, size, faint='{{c.faint}}')
def pie(p):
    a = p * 2 * math.pi
    return '<circle cx="8" cy="8" r="6.5" fill="currentColor" opacity="{{c.faint}}"></circle><path d="M8 8V1.5A6.5 6.5 0 %d 1 %.2f %.2f z" fill="currentColor"></path>' % (1 if p > .5 else 0, 8 + 6.5 * math.sin(a), 8 - 6.5 * math.cos(a))
BALL = {'blocked': INNER['blocked'], 'done': INNER['done'], 'error': INNER['error'],
 'spinner': '<circle cx="8" cy="8" r="6.5" fill="currentColor" opacity="{{c.faint}}"></circle><path d="M8 8V1.5A6.5 6.5 0 0 1 14.5 8z" fill="currentColor" transform="rotate(40 8 8)"></path>',
 'bell': F('M8 1.5a4.25 4.25 0 0 0-4.25 4.25v2.4L2.5 10.6a.9.9 0 0 0 .8 1.4h9.4a.9.9 0 0 0 .8-1.4l-1.25-2.45v-2.4A4.25 4.25 0 0 0 8 1.5zM6.2 13h3.6a1.8 1.8 0 0 1-3.6 0z')}
ANIM = [True]
def still(fn):
    """Build something with nothing moving: the Reduce Motion form."""
    ANIM[0] = False
    try: return fn()
    finally: ANIM[0] = True
def ball(kind, p=0.62, size=16, long=False):
    if kind is None: return ''
    if kind == 'spinner': return donut('working', None, COL, anim=ANIM[0])
    if kind == 'working': return donut('working', p, COL)
    return donut(kind, {'blocked': 0.4, 'error': 0.8}.get(kind) if p == 0.62 else p, COL)
# ---- atoms
def keycap(t): return '<span style="font-size: 12px; color: %s; background: %s; border-radius: 8px; padding: 2px 8px">%s</span>' % (INK2, QUIET, t)
def label(t): return '<span style="font-size: 11px; font-weight: 600; letter-spacing: 0.06em; text-transform: uppercase; color: %s">%s</span>' % (MUTED, t)
def fring(m, r=12):
    """Keyboard focus: a 2px ring in the session colour, 2px clear of the control."""
    return '<span style="display: inline-flex; padding: 2px; border: 2px solid %s; border-radius: %dpx">%s</span>' % (ACC, r, m)
OUTL = '{{c.outline}}'
def button(text, state='rest', key=None):
    if state == 'focus': return fring(button(text, 'rest', key))
    bg, col = {'rest': (RAISED, INK2), 'hover': (QUIET, INK), 'pressed': (OUTL, INK), 'off': (RAISED, MUTED)}[state]
    return '<span style="display: flex; align-items: center; gap: 8px; padding: 8px 12px 8px 8px; border-radius: 8px; background: %s; color: %s">%s<span>%s</span>%s</span>' % (bg, col, ic('plus', col), text, keycap(key) if key else '')
def cbutton(state='rest'):
    bg, col = {'rest': (RAISED, INK2), 'hover': (QUIET, INK), 'pressed': (OUTL, INK), 'off': (RAISED, MUTED)}[state]
    return '<span style="display: flex; align-items: center; padding: 8px; border-radius: 8px; background: %s; color: %s">%s</span>' % (bg, col, ic('plus', col))
def ibutton(key, state='rest', size=30):
    if state == 'focus': return fring(ibutton(key, 'rest', size))
    bg, col = {'rest': ('transparent', INK2), 'hover': (HOVER, INK), 'pressed': (QUIET, INK), 'off': ('transparent', '{{c.off}}')}[state]
    return '<span style="width: %dpx; height: %dpx; display: flex; align-items: center; justify-content: center; border-radius: 8px; background: %s">%s</span>' % (size, size, bg, ic(key, col, 14 if key == 'close' else 16))
def swatch(col): return '<span aria-hidden="true" style="width: 16px; height: 16px; border-radius: 5px; background: %s"></span>' % col
def bar(v): return '<span aria-hidden="true" style="width: 120px; height: 4px; border-radius: 999px; background: %s; display: flex"><span style="width: %d%%; height: 4px; border-radius: 999px; background: %s"></span></span>' % (QUIET, v, RAMP)
def cursor(active): return '<span aria-hidden="true" style="display: inline-block; box-sizing: border-box; width: 8px; height: 16px; background: %s; border: %s"></span>' % ((ACC, 'none') if active else ('transparent', '1px solid ' + INK2))
def tooltip(t): return '<span style="padding: 4px 8px; border-radius: 8px; background: %s; font-size: 12px">%s</span>' % (OVER, t)
# ---- molecules
def tab(title, sel=False, hover=False, status=None, width=140, side=False, pressed=False):
    """One slot at the right end, always 22px: the close button under the pointer, otherwise the ring. A selected tab shows no ring, since its panes are in view."""
    bg = QUIET if pressed else RAISED if sel else HOVER if hover else 'transparent'
    inner = ic('close', MUTED, 14) if hover else (ball(status) if (status and not sel) else '')
    slot = '<span style="width: 22px; height: 22px; flex-shrink: 0; display: flex; align-items: center; justify-content: center">%s</span>' % inner
    return '<span style="height: 30px; width: %s; box-sizing: border-box; display: flex; align-items: center; gap: 6px; padding: 0 4px 0 %s; border-radius: 8px; background: %s; color: %s; font-weight: %d"><span style="flex-grow: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis">%s</span>%s</span>' % ('%dpx' % width if width else '100%', '28px' if side else '12px', bg, INK if (sel or hover) else INK2, 600 if sel else 400, title, slot)
def srow(name, open_=False, hover=False, status=None, width=212, pressed=False):
    """Two fixed slots at the right end: + for a new tab, on the open session only, and x to end the session, under the pointer only."""
    bg = QUIET if pressed else TINT if open_ else HOVER if hover else 'transparent'
    slot = lambda inner: '<span style="width: 22px; height: 22px; flex-shrink: 0; display: flex; align-items: center; justify-content: center">%s</span>' % inner
    plus = ic('plus', INK if hover else MUTED) if open_ else ''
    close = ic('close', MUTED, 14) if hover else ''
    return '<span style="height: 30px; width: %s; box-sizing: border-box; display: flex; align-items: center; gap: 2px; padding: 0 4px 0 8px; border-radius: 8px; background: %s"><span style="display: flex; padding-right: 6px">%s</span><span style="flex-grow: 1; font-weight: %d">%s</span>%s%s</span>' % ('%dpx' % width if width else '100%', bg, ic('chevron-down' if open_ else 'chevron-right', MUTED, 12), 600 if open_ else 500, name, slot(plus), slot(close))
def shead(n=1):
    """The side bar's header: the label, and + for a new session. Nothing here that could be read as a status of the sessions."""
    return '<span style="display: flex; align-items: center; gap: 8px; padding: 0 4px 6px 8px">%s<span style="flex-grow: 1"></span>%s<span style="width: 22px; height: 22px; display: flex; align-items: center; justify-content: center">%s</span></span>' % (label('Sessions'), '', ic('plus', INK2))
def chip(name, status=None, hover=False, pressed=False): return '<span style="height: 34px; display: flex; align-items: center; gap: 8px; padding: 0 12px; border-radius: 10px; background: %s; color: %s">%s</span>' % (QUIET if pressed else RAISED if hover else PANE, INK if (hover or pressed) else INK2, name)
def caption(title, cwd, active=False, status=None, p=0.62, width=440, radius='8px', drop=0):
    kind = 'working' if status == 'spinner' else status
    prog = None if status == 'spinner' else {'blocked': 0.4, 'error': 0.8}.get(status, p)
    bar = flat(kind, prog, COL, '{{c.ramp6}}', anim=ANIM[0]) if status and drop < 2 else ''
    if drop >= 1: cwd = ''
    ctl = ''.join('<span style="width: 24px; height: 24px; display: flex; align-items: center; justify-content: center">%s</span>' % ic(k, INK2, 14 if k == 'close' else 16) for k in (('close',) if drop >= 3 else ('split-right', 'split-down', 'fill', 'close'))) if active else ''
    return '<span style="height: 30px; width: %s; box-sizing: border-box; display: flex; align-items: center; gap: 8px; padding: 0 %s 0 14px; border-radius: %s; background: %s; font-size: 12px; color: %s"><span style="color: %s; font-weight: 600">%s</span><span style="flex-grow: 1">%s</span>%s%s</span>' % ('%dpx' % width if width else '100%', '8px' if active else '14px', radius, RAMP50 if active else PANE, INK2, INK, title, cwd, bar, ctl)
def prow(main, sub='', right='', lead='', key='', sel=False, weight=600, width=None):
    return '<span style="height: 40px; width: %s; box-sizing: border-box; display: flex; align-items: center; gap: 12px; padding: 0 10px; border-radius: 8px; background: %s">%s<span style="font-weight: %d">%s</span><span style="flex-grow: 1; color: %s">%s</span>%s<span style="font-size: 12px; color: %s">%s</span></span>' % ('%dpx' % width if width else '100%', TINT if sel else 'transparent', lead, weight, main, INK2, sub, keycap(key) if key else '', INK2, right)
def pinput(mode, width=None):
    caret = '<span aria-hidden="true" style="display: inline-block; width: 1.5px; height: 17px; vertical-align: -3px; margin-left: 1px; background: %s"></span>' % ACC
    body = {'empty': caret + '<span style="color: %s; margin-left: 4px">Go to a session, tab or pane</span>' % MUTED, 'typed': 'car' + caret, 'actions': '<span style="color: %s">&gt;</span>split' % MUTED + caret, 'step': caret}[mode]
    chipm = '<span style="font-size: 13px; color: %s; background: %s; border-radius: 8px; padding: 3px 10px; white-space: nowrap">Session: Set colour</span>' % (INK2, QUIET) if mode == 'step' else ''
    return '<span style="width: %s; box-sizing: border-box; display: flex; align-items: center; gap: 10px; padding: 16px 16px 10px 16px"><span style="display: flex">%s</span>%s<span style="font-size: 15px; flex-grow: 1">%s</span>%s</span>' % ('%dpx' % width if width else '100%', ic('search', MUTED), chipm, body, keycap('esc'))
def needs(n, size=30):
    """The palette button with a count on it: how many panes are waiting for a person. A number in an amber pill, not a status ring, and not beside any session."""
    badge = '<span style="position: absolute; right: -2px; top: -2px; min-width: 14px; height: 14px; box-sizing: border-box; padding: 0 4px; border-radius: 999px; background: %s; color: %s; font-size: 10px; font-weight: 700; line-height: 14px; text-align: center">%d</span>' % (ST['blocked'], G, n) if n else ''
    return '<span style="position: relative; width: %dpx; height: %dpx; flex-shrink: 0; display: flex; align-items: center; justify-content: center; border-radius: 8px">%s%s</span>' % (size, size, ic('palette', INK2), badge)
def handle(state):
    pill = {'rest': 'transparent', 'hover': '{{c.outline}}', 'drag': ACC}[state]
    return '<span style="width: 150px; height: 64px; display: flex; align-items: stretch"><span style="flex: 3 1 0; border-radius: 10px; background: %s"></span><span style="width: 6px; display: flex; align-items: center; justify-content: center"><span style="width: 4px; height: 28px; border-radius: 999px; background: %s"></span></span><span style="flex: 2 1 0; border-radius: 10px; background: %s"></span></span>' % (RAISED, pill, PANE)
def empty(width=None): return '<span style="width: %s; display: flex; flex-direction: column; gap: 4px; padding: 24px 16px; align-items: center; box-sizing: border-box"><span style="font-weight: 600">No action matches "splti"</span><span style="color: %s">Remove the &gt; to look for a session, tab or pane instead.</span></span>' % ('%dpx' % width if width else '100%', INK2)
# ---- organisms
def col(items, width, gap=2, extra=''): return '<span style="width: %dpx; display: flex; flex-direction: column; gap: %dpx; %s">%s</span>' % (width, gap, extra, ''.join(items))
def sidebar():
    rows = [srow('steg', True, width=None), tab('shell', sel=True, width=None, side=True), tab('tests', status='working', width=None, side=True), tab('agent', status='blocked', width=None, side=True), tab('build', status='done', width=None, side=True), tab('server', status='error', width=None, side=True), srow('asdlc', width=None), srow('homelab', hover=True, width=None)]
    top = '<span style="display: flex; align-items: center; justify-content: flex-end; gap: 4px; padding: 0 0 10px 8px">%s%s</span>' % (ibutton('bar-top', size=24), ibutton('palette', size=24))
    return col([top, shead()] + rows, 212)
def topbar(tabs, chips, width):
    track = '<span style="display: flex; align-items: center; gap: 2px; padding: 2px; border-radius: 10px; background: %s"><span style="height: 30px; display: flex; align-items: center; padding: 0 12px; font-weight: 600">steg</span>%s%s</span>' % (TINT, ''.join(tab(t, sel=(i == 0), status=s, width=w) for i, (t, s, w) in enumerate(tabs)), ibutton('plus'))
    return '<span style="width: %dpx; display: flex; align-items: center; gap: 6px">%s%s%s<span style="flex-grow: 1"></span>%s<span style="width: 6px"></span>%s%s</span>' % (width, track, ''.join(chip(n, s) for n, s in chips), ibutton('plus'), '', ibutton('bar-side'), ibutton('palette'))
def pane(title, cwd, lines, active=False, status=None, width=330, outline='', prompt=True):
    body = ''.join('<span style="display: block"><span style="color: %s">%s</span>%s</span>' % (c, esc(a), esc(b)) for a, c, b in lines)
    cur = '<span style="display: block"><span style="color: %s">❯</span> %s</span>' % (ST['done'], cursor(active)) if (prompt and status in (None, 'done')) else ''
    return '<span style="width: %dpx; height: 190px; box-sizing: border-box; display: flex; flex-direction: column; overflow: hidden; border-radius: 14px; background: %s; %s">%s<span style="display: block; padding: 8px 14px; font-family: %s; font-size: 13px; line-height: 20px; white-space: pre">%s%s</span></span>' % (width, RAISED if active else PANE, outline, caption(title, cwd, active, status, width=None, radius='0'), MONO, body, cur)
def palette(mode, width=500):
    if mode == 'goto': rows = [label_row('Needs you'), prow('deploy', 'steg · agent', '~/src/pluto', ball('blocked'), sel=True), label_row('Recent'), prow('cargo test', 'steg · tests', '~/src/steg', ball('working')), prow('zsh', 'homelab · shell', '~/src/pluto', ic('pane', MUTED))]; foot = ['↵ go to', '⌘↵ open beside', '&gt; actions']
    if mode == 'actions': rows = [prow('Pane: Split right', key='⌘D', sel=True, weight=400), prow('Pane: Split down', key='⇧⌘D', weight=400), prow('Git: Split hunk into commits', right='plugin · git-tools', weight=400)]; foot = ['3 actions', '↵ run', '⌫ on &gt; to go to']
    if mode == 'step': rows = [prow('Violet', right='current', lead=swatch('{{s.violet}}')), prow('Pink', lead=swatch('{{s.pink}}'), sel=True), prow('Orange', lead=swatch('{{s.orange}}'))]; foot = ['Step 2 of 2', '↵ set', 'esc cancel']
    if mode == 'none': rows = [empty()]; foot = ['0 actions', '⌫ on &gt; to go to']
    inp = {'goto': 'empty', 'actions': 'actions', 'step': 'step', 'none': 'actions'}[mode]
    footer = '<span style="display: flex; align-items: center; gap: 14px; padding: 6px 16px 12px 16px; font-size: 12px; color: %s">%s</span>' % (MUTED, ''.join('<span style="flex-grow: %d">%s</span>' % (1 if i == 0 else 0, f) for i, f in enumerate(foot)))
    return '<span style="width: %dpx; display: flex; flex-direction: column; overflow: hidden; border-radius: 14px; background: %s">%s<span style="display: flex; flex-direction: column; gap: 2px; padding: 6px">%s</span>%s</span>' % (width, OVER, pinput(inp), ''.join(rows), footer)
def label_row(t): return '<span style="padding: 8px 10px 4px 10px">%s</span>' % label(t)
# ---- diagrams
def box(lab='', w=None, h=26, grow=False, bg=None, r=8):
    return '<span style="%s; height: %dpx; box-sizing: border-box; display: flex; align-items: center; justify-content: center; padding: 0 8px; border-radius: %dpx; background: %s; font-size: 11px; color: %s; white-space: nowrap">%s</span>' % ('flex-grow: 1' if grow else 'width: %dpx; flex-shrink: 0' % w, h, r, bg or RAISED, MUTED, lab)
def mini(panes, w=240, h=140):
    """A small window drawing. panes: (x, y, w, h, active, label) in px inside the window."""
    inner = ''.join('<span style="position: absolute; left: %dpx; top: %dpx; width: %dpx; height: %dpx; box-sizing: border-box; border-radius: 8px; background: %s; display: flex; align-items: center; justify-content: center; font-size: 11px; font-weight: %d; color: %s">%s</span>' % (x, y, pw, ph, RAISED if a else PANE, 600 if a else 400, INK if a else MUTED, lab) for x, y, pw, ph, a, lab in panes)
    return '<span style="position: relative; display: block; flex-shrink: 0; width: %dpx; height: %dpx; border-radius: 10px; background: %s">%s</span>' % (w, h, QUIET, inner)
# ---- live parts for the "Try it" areas
GROUPS = {}
def lic(key, size=16): return _icon(key, 'currentColor', size, faint='{{c.faint}}')
def lv_button(text, key=None, disabled=False, compact=False):
    return '<button type="button" class="lv lv-btn"%s%s>%s%s%s</button>' % (' disabled' if disabled else '', ' style="padding: 8px"' if compact else '', lic('plus'), '' if compact else '<span>%s</span>' % text, '<span class="lv-key">%s</span>' % key if key else '')
def lv_ibutton(key, size=30, disabled=False, label='button'):
    return '<button type="button" aria-label="%s" class="lv lv-ib"%s style="width: %dpx; height: %dpx">%s</button>' % (label, ' disabled' if disabled else '', size, size, lic(key, 14 if key == 'close' else 16))
def lv_tabs(group, items, side=False, width=140, default=0):
    GROUPS[group] = (len(items), default)
    return ''.join('<button type="button" role="tab" class="lv lv-tab {{on.%s.i%d}}" onClick="{{pick.%s.i%d}}" style="width: %s; padding-left: %s"><span class="lv-t">%s</span><span class="lv-slot"><span class="lv-a">%s</span><span class="lv-b">%s</span></span></button>' % (group, k, group, k, '%dpx' % width if width else '100%%', '28px' if side else '12px', t, ball(s) if s else '', lic('close', 14)) for k, (t, s) in enumerate(items)).replace('%%', '%')
def lv_srows(group, names, default=0):
    GROUPS[group] = (len(names), default)
    return ''.join('<button type="button" class="lv lv-srow {{on.%s.i%d}}" onClick="{{pick.%s.i%d}}"><span class="lv-cr">%s</span><span class="lv-cd">%s</span><span class="lv-t">%s</span><span class="lv-slot lv-plus">%s</span><span class="lv-slot lv-x">%s</span></button>' % (group, k, group, k, lic('chevron-right', 12), lic('chevron-down', 12), n, lic('plus'), lic('close', 14)) for k, n in enumerate(names))
def lv_chips(names): return ''.join('<button type="button" class="lv lv-chip">%s</button>' % n for n in names)
def lv_prows(group, rows, default=0):
    GROUPS[group] = (len(rows), default)
    return ''.join('<button type="button" class="lv lv-prow {{on.%s.i%d}}" onMouseEnter="{{pick.%s.i%d}}" onFocus="{{pick.%s.i%d}}">%s<span class="lv-main">%s</span><span class="lv-sub">%s</span>%s<span class="lv-right">%s</span></button>' % (group, k, group, k, group, k, lead, main, sub, '<span class="lv-key">%s</span>' % key if key else '', right) for k, (lead, main, sub, key, right) in enumerate(rows))
def lv_caps(group, items, default=0, width=400):
    GROUPS[group] = (len(items), default)
    ctl = ''.join('<span style="width: 24px; height: 24px; display: flex; align-items: center; justify-content: center">%s</span>' % lic(k, 14 if k == 'close' else 16) for k in ('split-right', 'split-down', 'fill', 'close'))
    return ''.join('<button type="button" class="lv lv-cap {{on.%s.i%d}}" onClick="{{pick.%s.i%d}}" style="width: %dpx"><span class="lv-main">%s</span><span class="lv-sub">%s</span><span class="lv-ctl">%s</span></button>' % (group, k, group, k, width, t, c, ctl) for k, (t, c) in enumerate(items))
def lv_handle(): return '<span class="lv lv-handle" tabindex="0" role="separator" aria-label="Resize"><span style="flex: 3 1 0; border-radius: 10px; background: %s"></span><span class="lv-gap"><span class="lv-pill"></span></span><span style="flex: 2 1 0; border-radius: 10px; background: %s"></span></span>' % (RAISED, PANE)
def lv_tip(key, text): return '<span class="lv-tipwrap">%s<span class="lv-tip">%s</span></span>' % (lv_ibutton(key, 24, label=text), text)
def lv_pinput(width=480):
    return '<span class="lv-pin" style="width: %dpx">%s<input aria-label="Palette" class="lv-input" value="{{pin.q}}" onChange="{{pin.change}}" placeholder="Go to a session, tab or pane"><span class="lv-mode">{{pin.mode}}</span>%s</span>' % (width, ic('search', MUTED), keycap('esc'))
CSS = """
@keyframes an-spin{to{transform:rotate(360deg)}}
@keyframes an-slide{from{left:-22%}to{left:100%}}
.an-spin{transform-origin:8px 8px;animation:an-spin 1.6s linear infinite}
.an-slide{animation:an-slide 2.2s linear infinite}
@media (prefers-reduced-motion: reduce){.an-spin,.an-slide{animation:none}}
.lv{font:inherit;cursor:pointer;border:none;margin:0;box-sizing:border-box;text-align:left;transition:background-color 80ms}
.lv:focus{outline:none}
.lv:focus-visible{outline:2px solid var(--acc);outline-offset:2px}
.lv:disabled{cursor:default}
.lv-key{font-size:12px;color:var(--ink2);background:var(--quiet);border-radius:8px;padding:2px 8px}
.lv-btn{display:flex;align-items:center;gap:8px;padding:8px 12px 8px 8px;border-radius:8px;background:var(--raised);color:var(--ink2)}
.lv-btn:hover{background:var(--quiet);color:var(--ink)}
.lv-btn:active{background:var(--outline);color:var(--ink)}
.lv-btn:disabled{background:var(--raised);color:var(--muted)}
.lv-ib{display:flex;align-items:center;justify-content:center;padding:0;border-radius:8px;background:transparent;color:var(--ink2)}
.lv-ib:hover{background:var(--hover);color:var(--ink)}
.lv-ib:active{background:var(--quiet);color:var(--ink)}
.lv-ib:disabled{background:transparent;color:var(--off)}
.lv-t{flex-grow:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.lv-slot{width:22px;height:22px;flex-shrink:0;display:flex;align-items:center;justify-content:center;color:var(--muted)}
.lv-tab{height:30px;display:flex;align-items:center;gap:6px;padding:0 4px 0 12px;border-radius:8px;background:transparent;color:var(--ink2);font-weight:400}
.lv-tab:hover{background:var(--hover);color:var(--ink)}
.lv-tab:active{background:var(--quiet)}
.lv-tab.on{background:var(--raised);color:var(--ink);font-weight:600}
.lv-tab .lv-a{display:flex}.lv-tab .lv-b{display:none}
.lv-tab:hover .lv-a,.lv-tab:focus-visible .lv-a,.lv-tab.on .lv-a{display:none}
.lv-tab:hover .lv-b,.lv-tab:focus-visible .lv-b{display:flex}
.lv-srow{width:212px;height:30px;display:flex;align-items:center;gap:2px;padding:0 4px 0 8px;border-radius:8px;background:transparent;color:var(--ink);font-weight:500}
.lv-srow:hover{background:var(--hover)}
.lv-srow:active{background:var(--quiet)}
.lv-srow.on{background:var(--tint);font-weight:600}
.lv-cr,.lv-cd{padding-right:6px;color:var(--muted)}
.lv-srow .lv-cr{display:flex}.lv-srow .lv-cd{display:none}
.lv-srow.on .lv-cr{display:none}.lv-srow.on .lv-cd{display:flex}
.lv-srow .lv-plus,.lv-srow .lv-x{visibility:hidden}
.lv-srow.on .lv-plus{visibility:visible}
.lv-srow:hover .lv-x,.lv-srow:focus-visible .lv-x{visibility:visible}
.lv-srow:hover .lv-plus{color:var(--ink)}
.lv-chip{height:34px;display:flex;align-items:center;padding:0 12px;border-radius:10px;background:var(--pane);color:var(--ink2)}
.lv-chip:hover{background:var(--raised);color:var(--ink)}
.lv-chip:active{background:var(--quiet);color:var(--ink)}
.lv-prow{width:480px;height:40px;display:flex;align-items:center;gap:12px;padding:0 10px;border-radius:8px;background:transparent;color:var(--ink)}
.lv-prow.on{background:var(--tint)}
.lv-prow:active{background:var(--quiet)}
.lv-main{font-weight:600}.lv-sub{flex-grow:1;color:var(--ink2)}.lv-right{font-size:12px;color:var(--ink2)}
.lv-cap{height:30px;display:flex;align-items:center;gap:8px;padding:0 14px;border-radius:8px;background:var(--pane);color:var(--ink2);font-size:12px}
.lv-cap .lv-main{color:var(--ink)}
.lv-cap .lv-ctl{display:none}
.lv-cap.on{background:var(--ramp50);padding-right:8px}
.lv-cap.on .lv-ctl{display:flex}
.lv-handle{width:150px;height:64px;display:flex;align-items:stretch;cursor:col-resize}
.lv-gap{width:6px;display:flex;align-items:center;justify-content:center}
.lv-pill{width:4px;height:28px;border-radius:999px;background:transparent;transition:background-color 80ms}
.lv-handle:hover .lv-pill,.lv-handle:focus-visible .lv-pill{background:var(--outline)}
.lv-handle:active .lv-pill{background:var(--acc)}
.lv-tipwrap{display:flex;align-items:center;gap:10px}
.lv-tip{padding:4px 8px;border-radius:8px;background:var(--over);font-size:12px;opacity:0;transition:opacity 120ms}
.lv-tipwrap:hover .lv-tip,.lv-tipwrap:focus-within .lv-tip{opacity:1;transition-delay:500ms}
.lv-pin{box-sizing:border-box;display:flex;align-items:center;gap:10px;padding:14px 16px;border-radius:14px;background:var(--over)}
.lv-input{flex-grow:1;min-width:0;padding:0;border:none;outline:none;background:transparent;color:var(--ink);caret-color:var(--acc);font:inherit;font-size:15px}
.lv-mode{font-size:12px;color:var(--muted);white-space:nowrap}
"""
# ---- documentation card
EXTRA = {}
_N = [0]
def card(name, what, stories, props, notes, stage_dir='row', sizes=None, drops=None, heads=('Prop', 'Values', 'Default'), states=None, live=None):
    x = EXTRA.get(name, {})
    states, live, stories, reduced = x.get('states', states), x.get('live', live), x.get('stories', stories), x.get('reduced')
    _N[0] += 1
    grid = lambda items: ''.join('<div style="display: flex; flex-direction: column; gap: 8px; align-items: flex-start"><div style="display: flex; align-items: center; min-height: 34px">%s</div><span style="font-size: 12px; color: %s">%s</span></div>' % (m, MUTED, esc(c)) for c, m in items)
    stage = lambda inner, d='row', bg=G: '<div style="display: flex; flex-direction: %s; flex-wrap: wrap; gap: 24px; padding: 20px; border-radius: 8px; background: %s">%s</div>' % (d, bg, inner)
    sec = lambda title, inner, hint='': '<div style="display: flex; flex-direction: column; gap: 10px"><div style="display: flex; align-items: baseline; gap: 10px">%s<span style="font-size: 12px; color: %s">%s</span></div>%s</div>' % (label(title), MUTED, hint, inner)
    parts = []
    if live: parts.append(sec('Try it', '<div style="display: flex; flex-wrap: wrap; align-items: center; gap: 16px; padding: 24px 20px; border-radius: 8px; background: %s">%s</div>' % (G, live), 'Press Play, then use the pointer, click, and the Tab key.'))
    if states: parts.append(sec('States', stage(grid(states)), 'Every state, drawn still.'))
    if stories: parts.append(sec('Variants' if states else 'Stories', stage(grid(stories), stage_dir)))
    if reduced: parts.append(sec('With Reduce Motion', stage(grid(reduced)), 'What this shows when the macOS setting is on: the same meaning, nothing moving.'))
    if sizes:
        rows = ''.join('<div style="display: grid; grid-template-columns: 90px 1fr 220px 220px; gap: 16px; align-items: center; min-height: 40px"><span style="font-weight: 600">%s</span><span style="display: flex; align-items: center">%s</span><span style="color: %s">%s</span><span style="color: %s">%s</span></div>' % (n, m, INK2, esc(shows), MUTED, esc(when)) for n, m, shows, when in sizes)
        parts.append(sec('Sizes', '<div style="display: flex; flex-direction: column; gap: 6px; padding: 16px 20px; border-radius: 8px; background: %s"><div style="display: grid; grid-template-columns: 90px 1fr 220px 220px; gap: 16px">%s%s%s%s</div>%s%s</div>' % (G, label('Size'), label('Drawn'), label('Shows'), label('When'), rows, '<span style="padding-top: 6px; color: %s"><span style="font-weight: 600; color: %s">Dropped first: </span>%s</span>' % (INK2, INK, esc(drops)) if drops else '')))
    pr = ''.join('<div style="display: grid; grid-template-columns: 150px 1fr 150px; gap: 16px; padding: 5px 0"><span style="font-family: %s; font-size: 12px">%s</span><span style="color: %s">%s</span><span style="color: %s">%s</span></div>' % (MONO, a, INK2, esc(b), MUTED, esc(d)) for a, b, d in props)
    parts.append('<div style="display: flex; flex-direction: column"><div style="display: grid; grid-template-columns: 150px 1fr 150px; gap: 16px; padding: 0 0 4px 0">%s%s%s</div>%s</div>' % (label(heads[0]), label(heads[1]), label(heads[2]), pr))
    parts.append(sec('Rules', '<ul style="margin: 0; padding: 0 0 0 18px; display: flex; flex-direction: column; gap: 4px; color: %s">%s</ul>' % (INK2, ''.join('<li>%s</li>' % esc(n) for n in notes))))
    badge = '<span style="flex-shrink: 0; padding: 4px 12px; border-radius: 999px; background: %s; color: %s; font-size: 12px; font-weight: 600; white-space: nowrap">%s</span>' % ((ACC, G, 'Interactive: press Play') if live else (QUIET, INK2, 'Static'))
    return ('<section style="display: flex; flex-direction: column; border-radius: 14px; overflow: hidden; background: %s">'
            '<div style="display: flex; align-items: center; gap: 16px; padding: 18px 24px; background: %s"><span style="width: 40px; height: 40px; flex-shrink: 0; display: flex; align-items: center; justify-content: center; border-radius: 10px; background: %s; font-family: %s; font-size: 15px; font-weight: 700">%02d</span>'
            '<div style="flex-grow: 1; display: flex; flex-direction: column; gap: 2px"><h2 style="margin: 0; font-size: 22px; font-weight: 600; letter-spacing: -0.01em">%s</h2><p style="margin: 0; color: %s">%s</p></div>%s</div>'
            '<div style="display: flex; flex-direction: column; gap: 24px; padding: 24px">%s</div></section>') % (PANE, RAISED, G, MONO, _N[0], name, INK2, esc(what), badge, ''.join(parts))
def board(fn, title, level, intro, cards, height, links=()):
    import json
    _N[0] = 0
    link = ''.join('<a href="%s" style="padding: 6px 12px; border-radius: 8px; background: %s; color: %s; text-decoration: none">%s</a>' % (h, RAISED, INK, t) for h, t in links)
    page = '''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>steg design system, %s</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;700&display=swap">
<style>
body{margin:0}
%s</style>
</helmet>
<div style="width: 1200px; height: %dpx; box-sizing: border-box; padding: 56px; display: flex; flex-direction: column; gap: 72px; --ground: {{c.ground}}; --pane: {{c.pane}}; --raised: {{c.raised}}; --over: {{c.over}}; --quiet: {{c.quiet}}; --hover: {{c.hover}}; --ink: {{c.ink}}; --ink2: {{c.ink2}}; --muted: {{c.muted}}; --outline: {{c.outline}}; --off: {{c.off}}; --acc: {{c.acc}}; --tint: {{c.tint}}; --ramp50: {{c.ramp50}}; background: %s; color: %s; font-family: %s; font-size: 13px; line-height: 1.4">
<div style="display: flex; flex-direction: column; gap: 6px; margin-bottom: -32px">
<span style="font-size: 12px; font-weight: 600; letter-spacing: 0.06em; text-transform: uppercase; color: %s">%s</span>
<h1 style="margin: 0; font-size: 28px; font-weight: 600; letter-spacing: -0.01em">%s</h1>
<p style="margin: 0; font-size: 14px; color: %s; max-width: 860px">%s</p>
<div style="display: flex; gap: 8px; padding-top: 8px">%s</div>
</div>
%s
</div>
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{"theme":{"editor":"enum","options":["dark","light"],"default":"dark"},"session":{"editor":"enum","options":["violet","pink","orange","blue","green"],"default":"violet"},"$preview":{"width":1200,"height":%d}}'>
class Component extends DCLogic {
pickFn(group, k) {
var self = this;
return function () { var s = Object.assign({}, (self.state && self.state.sel) || {}); s[group] = k; self.setState({ sel: s }); };
}
renderVals() {
var self = this;
var GROUPS = %s;
var sel = (this.state && this.state.sel) || {}, on = {}, pick = {};
for (var gname in GROUPS) { on[gname] = {}; pick[gname] = {}; var cur = sel[gname] === undefined ? GROUPS[gname][1] : sel[gname]; for (var gk = 0; gk < GROUPS[gname][0]; gk++) { on[gname]['i' + gk] = gk === cur ? 'on' : ''; pick[gname]['i' + gk] = this.pickFn(gname, gk); } }
var q = (this.state && this.state.q) || '';
var pin = { q: q, change: function (e) { self.setState({ q: e.target.value }); }, mode: q.charAt(0) === '>' ? 'Actions' : q.charAt(0) === '?' ? 'Help' : 'Go to' };
var light = this.props.theme === 'light';
var i = light ? 1 : 0;
var S = { violet: ['#a98cff', '#7b5cf0'], pink: ['#f277b5', '#d6408c'], orange: ['#ff9950', '#e0701a'], blue: ['#6fb1ff', '#2463c9'], green: ['#6fd3a0', '#14734a'] };
var c = light
? { ground: '#f1efec', pane: '#f8f6f4', raised: '#ffffff', over: '#ffffff', quiet: '#dfdcd7', hover: '#f7f5f3', ink: '#1b1a1f', ink2: '#4f4d57', muted: '#63616b', outline: '#bfbbb5', off: '#a5a2ab', track: '#cfcbc5', r1: '#7b5cf0', r2: '#d6408c', r3: '#e0701a', blocked: '#8f5300', done: '#14734a', error: '#bf2c27', faint: '0.45', ramp: 'linear-gradient(90deg, #7b5cf0 0%%, #d6408c 50%%, #e0701a 100%%)', ramp6: 'linear-gradient(90deg, #7b5cf0 0%%, #d6408c 20%%, #bf2c27 40%%, #e0701a 60%%, #8f5300 80%%, #14734a 100%%)', ramp50: 'linear-gradient(90deg, #7b5cf066 0%%, #d6408c66 50%%, #e0701a66 100%%)' }
: { ground: '#0e0d10', pane: '#141318', raised: '#1c1b21', over: '#201f26', quiet: '#28272e', hover: '#19181d', ink: '#ecebee', ink2: '#b0aeb9', muted: '#8f8d99', outline: '#3d3b45', off: '#5f5d69', track: '#3d3b45', r1: '#a98cff', r2: '#f277b5', r3: '#ff9950', blocked: '#ffc25e', done: '#6fd3a0', error: '#ff8a80', faint: '0.3', ramp: 'linear-gradient(90deg, #a98cff 0%%, #f277b5 50%%, #ff9950 100%%)', ramp6: 'linear-gradient(90deg, #a98cff 0%%, #f277b5 20%%, #ff8a80 40%%, #ff9950 60%%, #ffc25e 80%%, #6fd3a0 100%%)', ramp50: 'linear-gradient(90deg, #a98cff80 0%%, #f277b580 50%%, #ff995080 100%%)' };
c.acc = (S[this.props.session] || S.violet)[i];
c.tint = c.acc + '2e'; c.blockedSoft = c.blocked + '2e'; c.errorSoft = c.error + '2e'; c.doneSoft = c.done + '2e';
var s = {};
for (var k in S) { s[k] = S[k][i]; }
return { yes: true, c: c, s: s, theme: light ? 'light' : 'dark', on: on, pick: pick, pin: pin };
}
}
</script>
</body>
</html>
''' % (title.lower(), CSS, height, G, INK, UI, MUTED, level, title, INK2, esc(intro) + ' Use Tweaks to see this board in light, or in another session colour.', link, '\n'.join(cards), height, json.dumps({k: list(v) for k, v in GROUPS.items()}))
    open('project/' + fn, 'w').write(page); print(fn, len(page), len(cards))
# ---- every state, a live area, and the Reduce Motion form, per component
T = lambda *a, **k: tab(*a, **k)
EXTRA.update({
 'Icon button': dict(
  states=[('at rest', ibutton('plus')), ('under the pointer', ibutton('plus', 'hover')), ('pressed', ibutton('plus', 'pressed')), ('keyboard focus', ibutton('plus', 'focus')), ('cannot be used', ibutton('plus', 'off'))],
  stories=[('24px, at rest', ibutton('split-down', size=24)), ('24px, under the pointer', ibutton('split-down', 'hover', 24)), ('24px, pressed', ibutton('split-down', 'pressed', 24)), ('close, 14px icon', ibutton('close', size=24))],
  live=lv_ibutton('plus', label='New') + lv_ibutton('bar-top', label='Move the bar') + lv_ibutton('palette', label='Palette') + lv_ibutton('split-right', 24, label='Split right') + lv_ibutton('close', 24, label='Close') + lv_ibutton('plus', disabled=True, label='Cannot be used')),
 'Button': dict(
  states=[('at rest', button('New session')), ('under the pointer', button('New session', 'hover')), ('pressed', button('New session', 'pressed')), ('keyboard focus', button('New session', 'focus')), ('cannot be used', button('New session', 'off'))],
  stories=[('with a shortcut', button('New session', key='⌘N')), ('compact', cbutton()), ('compact, under the pointer', cbutton('hover'))],
  live=lv_button('New session') + lv_button('New session', key='⌘N') + lv_button('', compact=True) + lv_button('New session', disabled=True)),
 'Tooltip': dict(
  live=lv_tip('split-right', 'Split right  ⌘D') + lv_tip('fill', 'Fill the tab') + lv_tip('close', 'Close  ⌘W'),
  states=[('hidden', '<span style="display: flex; align-items: center; gap: 10px">%s</span>' % ibutton('split-right', size=24)), ('shown, after half a second', '<span style="display: flex; align-items: center; gap: 10px">%s%s</span>' % (ibutton('split-right', 'hover', 24), tooltip('Split right  ⌘D'))), ('shown for keyboard focus', '<span style="display: flex; align-items: center; gap: 10px">%s%s</span>' % (ibutton('split-right', 'focus', 24), tooltip('Split right  ⌘D')))],
  reduced=still(lambda: [('appears at once after the same half second: no fade', '<span style="display: flex; align-items: center; gap: 10px">%s%s</span>' % (ibutton('split-right', 'hover', 24), tooltip('Split right  ⌘D')))])),
 'Cursor': dict(
  reduced=still(lambda: [('steady: the cursor never blinks, with or without the setting', cursor(True))])),
 'Tab': dict(
  states=[('at rest', T('tests')), ('under the pointer', T('tests', hover=True)), ('pressed', T('tests', pressed=True)), ('keyboard focus', fring(T('tests', hover=True))), ('selected', T('shell', sel=True)), ('selected, under the pointer', T('shell', sel=True, hover=True)), ('with a status', T('tests', status='working')), ('with a status, under the pointer', T('tests', hover=True, status='working'))],
  stories=[('progress unknown: the arc keeps turning', T('lint', status='spinner')), ('short of room', T('integration-tests', status='working', width=100)), ('smallest', T('integration-tests', status='working', width=72)), ('in the side bar', '<span style="width: 212px; display: flex">%s</span>' % T('tests', status='blocked', width=None, side=True)), ('two panes: the tab shows the more pressing', '<span style="display: flex; flex-direction: column; gap: 8px">' + T('deploy', status='blocked') + '<span style="display: flex; gap: 6px">' + caption('terraform', '', status='working', width=236, drop=1) + caption('agent', '', status='blocked', width=236, drop=1) + '</span></span>')],
  live='<span style="display: flex; align-items: center; gap: 2px; padding: 2px; border-radius: 10px; background: %s">%s</span><span style="width: 212px; display: flex; flex-direction: column; gap: 2px">%s</span>' % (TINT, lv_tabs('tabs', [('shell', None), ('tests', 'working'), ('agent', 'blocked'), ('build', 'done')], width=120), lv_tabs('sidetabs', [('shell', None), ('tests', 'working'), ('server', 'error')], side=True, width=None)),
  reduced=still(lambda: [('the ring holds still: an arc of unknown progress does not turn', T('lint', status='spinner')), ('a failure does not shake', T('server', status='error'))])),
 'Session row': dict(
  states=[('closed', srow('homelab')), ('closed, under the pointer', srow('homelab', hover=True)), ('closed, pressed', srow('homelab', pressed=True)), ('keyboard focus', fring(srow('homelab', hover=True))), ('open', srow('steg', True)), ('open, under the pointer', srow('steg', True, hover=True))],
  stories=[('the header above them', '<span style="width: 212px; display: block">%s</span>' % shead())],
  live='<span style="display: flex; flex-direction: column; gap: 2px">%s</span>' % lv_srows('srows', ['steg', 'asdlc', 'homelab'])),
 'Session chip': dict(
  states=[('at rest', chip('homelab')), ('under the pointer', chip('homelab', hover=True)), ('pressed', chip('homelab', pressed=True)), ('keyboard focus', fring(chip('homelab', hover=True), 14))],
  stories=[('short of room', chip('as'))],
  live='<span style="display: flex; gap: 6px">%s</span>' % lv_chips(['asdlc', 'homelab', 'dotfiles'])),
 'Pane caption': dict(
  live='<span style="display: flex; flex-direction: column; gap: 8px">%s</span>' % lv_caps('caps', [('zsh', '~/src/steg'), ('cargo test', 'steg-kernel')]),
  reduced=still(lambda: [('running: the fill steps to its new length', caption('cargo test', 'steg-kernel', status='working')), ('progress unknown: the segment sits still in the middle', caption('agent', '~/src/steg', status='spinner')), ('failed: red at once, broken end, no flakes', caption('steg-server', '~/src/steg', status='error')), ('a bell: the amber bar, no wave', caption('make', '~/src/steg', status='bell'))])),
 'Palette row': dict(
  live='<span style="display: flex; flex-direction: column; gap: 2px; padding: 6px; border-radius: 14px; background: %s">%s</span>' % (OVER, lv_prows('prows', [(ball('blocked'), 'deploy', 'steg · agent', '', '~/src/pluto'), (ball('working'), 'cargo test', 'steg · tests', '', '~/src/steg'), (ic('pane', MUTED), 'zsh', 'homelab · shell', '', '~/src/pluto'), ('', 'Pane: Split right', '', '⌘D', '')]))),
 'Palette input': dict(
  live=lv_pinput()),
 'Split handle': dict(
  live=lv_handle(),
  states=[('at rest', handle('rest')), ('under the pointer', handle('hover')), ('keyboard focus', fring(handle('hover'), 14)), ('while dragging', handle('drag'))], stories=[]),
 'Bar, on the side': dict(
  live='<span style="width: 212px; display: flex; flex-direction: column; gap: 2px">%s%s%s</span>' % (lv_srows('osrow', ['steg']), lv_tabs('otabs', [('shell', None), ('tests', 'working'), ('agent', 'blocked'), ('server', 'error')], side=True, width=None), lv_srows('osrow2', ['asdlc', 'homelab'], default=-1))),
 'Bar, on top': dict(
  live='<span style="display: flex; align-items: center; gap: 6px"><span style="display: flex; align-items: center; gap: 2px; padding: 2px; border-radius: 10px; background: %s"><span style="height: 30px; display: flex; align-items: center; padding: 0 12px; font-weight: 600">steg</span>%s%s</span>%s%s</span>' % (TINT, lv_tabs('ttabs', [('shell', None), ('tests', 'working'), ('agent', 'blocked')], width=110), lv_ibutton('plus', label='New tab'), lv_chips(['asdlc', 'homelab']), lv_ibutton('plus', label='New session'))),
 'Command palette': dict(
  live='<span style="display: flex; flex-direction: column; border-radius: 14px; overflow: hidden; background: %s">%s<span style="display: flex; flex-direction: column; gap: 2px; padding: 0 6px 6px 6px">%s</span></span>' % (OVER, lv_pinput(492), lv_prows('oprows', [(ball('blocked'), 'deploy', 'steg · agent', '', '~/src/pluto'), (ball('working'), 'cargo test', 'steg · tests', '', '~/src/steg'), (ic('pane', MUTED), 'zsh', 'homelab · shell', '', '~/src/pluto')]))),
 'Status in and out of view': dict(
  reduced=still(lambda: [('the bar is there or it is not: it does not roll out or back', caption('cargo test', 'steg-kernel', status='working', width=360))])),
})
# ---- foundations: the named values, rules and terminal themes, in the same card layout
import json as _json
FD = _json.load(open('foundations.json'))
def sheet(name, what, body, live=False):
    """A numbered card with the same header band as a component card, and any body."""
    _N[0] += 1
    badge = '<span style="flex-shrink: 0; padding: 4px 12px; border-radius: 999px; background: %s; color: %s; font-size: 12px; font-weight: 600; white-space: nowrap">%s</span>' % ((ACC, G, 'Interactive: press Play') if live else (QUIET, INK2, 'Static'))
    return ('<section style="display: flex; flex-direction: column; border-radius: 14px; overflow: hidden; background: %s">'
            '<div style="display: flex; align-items: center; gap: 16px; padding: 18px 24px; background: %s"><span style="width: 40px; height: 40px; flex-shrink: 0; display: flex; align-items: center; justify-content: center; border-radius: 10px; background: %s; font-family: %s; font-size: 15px; font-weight: 700">%02d</span>'
            '<div style="flex-grow: 1; display: flex; flex-direction: column; gap: 2px"><h2 style="margin: 0; font-size: 22px; font-weight: 600; letter-spacing: -0.01em">%s</h2><p style="margin: 0; color: %s">%s</p></div>%s</div>'
            '<div style="display: flex; flex-direction: column; gap: 24px; padding: 24px">%s</div></section>') % (PANE, RAISED, G, MONO, _N[0], name, INK2, esc(what), badge, body)
def sw(hexv): return '<span style="display: flex; align-items: center; gap: 10px"><span aria-hidden="true" style="width: 26px; height: 26px; border-radius: 6px; background: %s; box-shadow: inset 0 0 0 1px %s"></span><span style="font-family: %s; font-size: 12px; color: %s">%s</span></span>' % (hexv, OUTL, MONO, INK2, hexv)
def colour_table(rows):
    head = '<div style="display: grid; grid-template-columns: 190px 150px 150px 1fr; gap: 16px; padding-bottom: 6px">%s%s%s%s</div>' % (label('Name'), label('Dark'), label('Light'), label('Used for'))
    return '<div style="display: flex; flex-direction: column; gap: 8px; padding: 16px 20px; border-radius: 8px; background: %s">%s%s</div>' % (G, head, ''.join('<div style="display: grid; grid-template-columns: 190px 150px 150px 1fr; gap: 16px; align-items: center; min-height: 32px"><span style="font-family: %s; font-size: 13px">%s</span>%s%s<span style="color: %s">%s</span></div>' % (MONO, r['name'], sw(r['dark']), sw(r['light']), INK2, esc(r['use'])) for r in rows))
def kv_table(rows, a='Name', b='Value'):
    head = '<div style="display: grid; grid-template-columns: 190px 1fr; gap: 16px; padding-bottom: 6px">%s%s</div>' % (label(a), label(b))
    return '<div style="display: flex; flex-direction: column; gap: 8px; padding: 16px 20px; border-radius: 8px; background: %s">%s%s</div>' % (G, head, ''.join('<div style="display: grid; grid-template-columns: 190px 1fr; gap: 16px; align-items: baseline; min-height: 26px"><span style="font-family: %s; font-size: 13px">%s</span><span style="color: %s">%s</span></div>' % (MONO, r['name'], INK2, esc(r['value'])) for r in rows))
def para(t): return '<p style="margin: 0; color: %s; max-width: 900px">%s</p>' % (INK2, esc(t))
def stagebox(inner, d='row'): return '<div style="display: flex; flex-direction: %s; flex-wrap: wrap; align-items: flex-end; gap: 24px; padding: 20px; border-radius: 8px; background: %s">%s</div>' % (d, G, inner)
def cap(m, t): return '<div style="display: flex; flex-direction: column; gap: 8px; align-items: flex-start">%s<span style="font-size: 12px; color: %s">%s</span></div>' % (m, MUTED, esc(t))
neutral = [r for r in FD['colours'] if not r['name'].startswith(('ramp.', 'session.'))]
six = [r for r in FD['colours'] if r['name'].startswith(('ramp.', 'session.'))]
R3 = lambda i: 'linear-gradient(90deg, %s 0%%, %s 50%%, %s 100%%)' % tuple(six[k]['light' if i else 'dark'] for k in (0, 1, 3))
R6 = lambda i: 'linear-gradient(90deg, %s)' % ', '.join(six[k]['light' if i else 'dark'] for k in range(6))
gbar = lambda bg, t: cap('<span aria-hidden="true" style="display: block; width: 480px; height: 28px; border-radius: 6px; background: %s"></span>' % bg, t)
spacing = ''.join(cap('<span aria-hidden="true" style="display: block; width: %dpx; height: 28px; border-radius: 2px; background: %s"></span>' % (n, ACC), '%dpx' % n) for n in (2, 4, 6, 8, 10, 12, 16))
radii = ''.join(cap('<span aria-hidden="true" style="display: block; width: 72px; height: 48px; border-radius: %dpx; background: %s"></span>' % (n, RAISED), t) for n, t in ((14, '14px: panes, palette'), (10, '10px: tab track, session chips'), (8, '8px: anything inside those'), (999, 'fully round: chips of colour, bars')))
typ = [('ui.body', 'font-size: 13px', 'Split right, then down'), ('ui.body, selected', 'font-size: 13px; font-weight: 600', 'Split right, then down'), ('ui.small', 'font-size: 12px; color: ' + INK2, '26 of 42 · ~/src/steg'), ('ui.label', 'font-size: 11px; font-weight: 600; letter-spacing: 0.06em; text-transform: uppercase; color: ' + MUTED, 'Sessions'), ('palette.input', 'font-size: 15px', 'Go to a session, tab or pane'), ('terminal', 'font-family: ' + MONO + '; font-size: 13px; line-height: 20px', 'test layout::split_right ... ok')]
specimens = ''.join(cap('<span style="%s">%s</span>' % (st, tx), nm) for nm, st, tx in typ)
found = [
 sheet('Surfaces and text', 'The neutral colours: what things sit on, and what is written on them. ' + FD['intro'], colour_table(neutral)),
 sheet('The six colours', 'Every accent, session and status colour is one of six steps, defined once, plus one blue.', colour_table(six)),
 sheet('The ramp and the extended ramp', 'The two gradients made from the six colours.', stagebox(gbar(R3(0), 'ramp, dark: steps 1, 2 and 4') + gbar(R3(1), 'ramp, light') + gbar(R6(0), 'ramp.extended, dark: steps 1 to 6') + gbar(R6(1), 'ramp.extended, light')) + para(FD['ramp_text'])),
 sheet('Type', 'Two typefaces and a handful of sizes.', stagebox(specimens) + kv_table(FD['type'])),
 sheet('Sizes, spacing and corners', 'The numbers everything is measured in.', stagebox(spacing) + stagebox(radii) + kv_table(FD['sizes'])),
] + [sheet('Rules: ' + grp['name'].lower(), 'What must hold wherever these parts are used.', '<ul style="margin: 0; padding: 0 0 0 18px; display: flex; flex-direction: column; gap: 8px; color: %s; max-width: 980px">%s</ul>' % (INK2, ''.join('<li>%s</li>' % esc(r) for r in grp['rules']))) for grp in FD['rules']]
board('Tokens.dc.html', 'Foundations', 'Design system · 1 of 5', 'The named values and the rules. Everything on the later pages is built from these and nothing else. A proposal until Bruk accepts it.', found, 5600)

def term(t):
    A = t['ansi']; nm = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white']
    sample = ('<div style="box-sizing: border-box; width: 520px; padding: 14px 16px; border-radius: 14px; background: %s; color: %s; font-family: %s; font-size: 13px; line-height: 20px; white-space: pre">'
              '<div><span style="color: %s">❯</span> git status --short</div><div><span style="color: %s"> M</span> src/pane.rs</div><div><span style="color: %s">??</span> tests/split.rs</div>'
              '<div><span style="color: %s">src/</span>  <span style="color: %s">link</span>  <span style="color: %s">image.png</span></div>'
              '<div><span style="background: %s">selected text</span> <span aria-hidden="true" style="display: inline-block; width: 8px; height: 16px; vertical-align: -3px; background: %s"></span></div></div>') % (t['bg'], t['fg'], MONO, A[2], A[3], A[1], A[4], A[6], A[5], t['selection'], t['cursor'])
    chips = lambda off: '<div style="display: grid; grid-template-columns: repeat(8, 100px); gap: 8px">%s</div>' % ''.join('<div style="display: flex; flex-direction: column; gap: 4px"><span aria-hidden="true" style="height: 28px; border-radius: 6px; background: %s; box-shadow: inset 0 0 0 1px %s"></span><span style="font-size: 11px; color: %s">%s%s</span><span style="font-family: %s; font-size: 11px; color: %s">%s</span></div>' % (A[off + k], OUTL, INK2, 'bright ' if off else '', nm[k], MONO, MUTED, A[off + k]) for k in range(8))
    return stagebox(sample) + '<div style="display: flex; flex-direction: column; gap: 14px; padding: 16px 20px; border-radius: 8px; background: %s">%s%s</div>' % (G, chips(0), chips(8)) + '<span style="font-family: %s; font-size: 12px; color: %s">%s</span>' % (MONO, INK2, esc(t['base']))
focus_states = [('a button, at rest', button('New session')), ('a button, keyboard focus', button('New session', 'focus')), ('an icon button, keyboard focus', ibutton('plus', 'focus')), ('a tab, keyboard focus', fring(tab('tests', hover=True))), ('a session chip, keyboard focus', fring(chip('homelab', hover=True), 14))]
det = [sheet('Terminal theme: ' + t['name'], 'The sixteen colours a program can choose from, with a sample of how they read together.', term(t)) for t in FD['themes']] + [
 sheet('Focus ring', 'Where the keyboard is. The one line the design draws in normal use.',
       '<div style="display: flex; flex-direction: column; gap: 10px"><div style="display: flex; align-items: baseline; gap: 10px">%s<span style="font-size: 12px; color: %s">Press Play, click in this area, then press the Tab key.</span></div><div style="display: flex; flex-wrap: wrap; align-items: center; gap: 16px; padding: 24px 20px; border-radius: 8px; background: %s">%s</div></div>' % (label('Try it'), MUTED, G, lv_button('New session') + lv_ibutton('plus', label='New') + lv_ibutton('palette', label='Palette') + lv_chips(['asdlc', 'homelab']))
       + stagebox(''.join(cap(m, t) for t, m in focus_states), 'row') + para(FD['focus_text']), live=True)]
board('Details.dc.html', 'Terminal themes and the focus ring', 'Design system · 1 of 5', FD['details_intro'], det, 2500)
HOW = ' Each component has its own numbered card and is documented the same way: a place to try it in Play if it can be used, every state drawn still, its variants, how it looks with Reduce Motion where it moves, its sizes, its props, and its rules.'
atoms = [
 card('Icon', 'One of the 19 drawn icons. The full set is on the Icons board.', [('16px, ink', ic('split-right', INK)), ('16px, secondary', ic('split-right')), ('16px, muted', ic('search', MUTED)), ('14px', ic('close', INK2, 14)), ('12px', ic('chevron-right', MUTED, 12))],
  [('name', 'Any icon on the Icons board', 'required'), ('size', '12, 14, 16', '16'), ('tone', 'ink, secondary, muted', 'secondary')],
  ['12px is only for the chevrons in the side list; 14px only for close.']),
 card('Icon button', 'A button that is only an icon.', [('30px, at rest', ibutton('plus')), ('30px, under the pointer', ibutton('plus', 'hover')), ('30px, cannot be used', ibutton('plus', 'off')), ('24px, at rest', ibutton('split-down', size=24)), ('24px, under the pointer', ibutton('split-down', 'hover', 24))],
  [('icon', 'An icon name', 'required'), ('label', 'Text for VoiceOver and the tooltip', 'required'), ('size', '24, 30', '30'), ('disabled', 'yes, no', 'no')],
  ['30px in the bar, 24px in a pane caption and inside a tab.', 'Hover is a change of fill. A button that cannot be used keeps its place and goes muted; it is never faded or hidden.']),
 card('Button', 'A button with a word on it.', [('at rest', button('New session')), ('under the pointer', button('New session', 'hover')), ('cannot be used', button('New session', 'off'))],
  [('label', 'Text', 'required'), ('icon', 'An icon name, or none', 'none'), ('key', 'A shortcut, or none', 'none'), ('disabled', 'yes, no', 'no')],
  ['Labels name the thing and the act: "New session", not "Add".', 'Compact is the same button without its label: it keeps the fill, the height and the corner. The label is not lost: it becomes the tooltip and the VoiceOver name.', 'This is not the icon button. An icon button has no fill at rest and is used where a row of controls should stay quiet, such as the bar and a pane caption.'],
  sizes=[('Compact', cbutton(), 'Icon only', 'Less than 72px to draw in'), ('Regular', button('New session'), 'Icon and label', 'The default'), ('Full', button('New session', key='⌘N'), 'Icon, label and shortcut', 'In the palette and in menus')],
  drops='the shortcut, then the label. Never the icon.'),
 card('Key cap', 'A keyboard shortcut, shown where the action is listed.', [('one key', keycap('esc')), ('with a modifier', keycap('⌘D')), ('two modifiers', keycap('⇧⌘D')), ('Return', keycap('↵'))],
  [('keys', 'Modifier symbols, then the key', 'required')],
  ['Uses the macOS symbols in the macOS order: control, option, shift, command.']),
 card('Label', 'A small heading over a group.', [('in the side bar', label('Sessions')), ('in the palette', label('Needs you'))],
  [('text', 'One or two words', 'required')], ['Upper case with wide spacing. Never a button.']),
 card('Swatch', 'One colour a person can choose.', [(n, swatch(c)) for n, c in (('violet', '{{s.violet}}'), ('pink', '{{s.pink}}'), ('orange', '{{s.orange}}'), ('blue', '{{s.blue}}'), ('green', '{{s.green}}'))],
  [('colour', 'A session colour', 'required')], ['A rounded square, never a circle, so it cannot be mistaken for a status ring.']),
 card('Cursor', 'Where typing goes.', [('in the active pane', cursor(True)), ('in any other pane', cursor(False))],
  [('active', 'yes, no', 'no')], ['Filled in the session colour when active; a hollow outline otherwise, as terminals have always done.']),
 card('Tooltip', 'The word for an icon, shown when the pointer rests on it.', [('on a status ring', '<span style="display: flex; align-items: center; gap: 10px">%s%s</span>' % (ball('blocked'), tooltip('blocked'))), ('on a button', '<span style="display: flex; align-items: center; gap: 10px">%s%s</span>' % (ibutton('split-right', 'hover', 24), tooltip('Split right  ⌘D')))],
  [('text', 'The word, and the shortcut if there is one', 'required')], ['Appears after half a second, beside the icon, never over it. It is the only place a status is written as a word.']),
]
board('Atoms.dc.html', 'Atoms', 'Design system · 2 of 5', 'The smallest parts, which cannot be broken down further. Foundations (colours, type, sizes, rules) come before these; molecules are built from them. Two atoms have boards of their own: the icons, and the status ring and bar.' + HOW, atoms, 6300, [('IconSet.dc.html', 'Open the icons board'), ('Status.dc.html', 'Open the status board')])
# ---- foundations: layout
row_d = '<span style="width: 300px; box-sizing: border-box; display: flex; align-items: center; gap: 8px; padding: 6px; border-radius: 8px; background: %s">%s%s%s</span>' % (PANE, box('fixed', 52), box('stretches', grow=True), box('fixed', 52))
stack_d = '<span style="width: 200px; display: flex; flex-direction: column; gap: 2px">%s%s%s</span>' % (box('row', 200, 30), box('row', 200, 30), box('row', 200, 30))
track_d = '<span style="display: flex; align-items: center; gap: 2px; padding: 2px; border-radius: 10px; background: %s">%s%s%s</span>' % (TINT, box('item', 84, 30), box('item', 84, 30, bg='transparent'), box('item', 84, 30, bg='transparent'))
track_real = '<span style="display: flex; align-items: center; gap: 2px; padding: 2px; border-radius: 10px; background: %s"><span style="height: 30px; display: flex; align-items: center; padding: 0 12px; font-weight: 600">steg</span>%s%s</span>' % (TINT, tab('shell', sel=True, width=110), tab('tests', status='working', width=110))
overlay_d = '<span style="position: relative; display: block; width: 300px; height: 170px; border-radius: 10px; overflow: hidden; background: %s"><span style="position: absolute; left: 8px; top: 8px; width: 138px; height: 154px; border-radius: 8px; background: %s"></span><span style="position: absolute; left: 154px; top: 8px; width: 138px; height: 154px; border-radius: 8px; background: %s"></span><span style="position: absolute; left: 0; top: 0; width: 300px; height: 170px; background: #05050899"></span><span style="position: absolute; left: 70px; top: 24px; width: 160px; height: 96px; border-radius: 10px; background: %s; display: flex; align-items: center; justify-content: center; font-size: 11px; color: %s">surface</span></span>' % (QUIET, PANE, PANE, OVER, MUTED)
lay = [
 card('Resizing', 'The one rule every component follows when room runs out. Each card that can resize has a Sizes block that fills this in.',
  [('compact', cbutton()), ('regular', button('New session')), ('full', button('New session', key='⌘N'))],
  [('compact', 'The least it can be and still be used', 'required'), ('regular', 'What it is with room', 'required'), ('full', 'Extra detail, such as a shortcut, where there is room for it', 'optional'), ('dropped first', 'The order in which parts go, least important first', 'required'), ('when', 'The width at which each step happens', 'required')],
  ['A component shrinks by dropping whole parts in a stated order. It never squeezes, fades or overlaps them.', 'An icon and a status are never dropped and never cut off. Text is the first thing to shorten.', 'What is dropped stays reachable: a label becomes the tooltip, a status stays on the ring in the bar.', 'Parts of one kind change together: every chip shortens at once, every tab in a track is the same width.'],
  heads=('Part', 'Meaning', 'Needed')),
 card('Row', 'Things in a line, centred on each other, with one fixed gap. Exactly one part stretches to take the spare room.',
  [('the shape', row_d), ('a tab', tab('tests', status='working', width=160)), ('a pane caption', caption('zsh', '~/src/steg', True, width=360))],
  [('gap', '6px in a tab, 8px in a caption or session row, 12px in a palette row', '8px'), ('stretches', 'The one part that takes spare room: usually the title', 'required'), ('height', '30px, or 40px in the palette', '30px')],
  ['Used by the tab, session row, session chip, pane caption, palette row and palette input.', 'The part that stretches is also the part that shortens first.']),
 card('Stack', 'Things in a column, each the full width, with one fixed gap.',
  [('the shape', stack_d), ('the side bar', col([srow('steg', True, width=None), tab('shell', sel=True, width=None, side=True), tab('tests', status='working', width=None, side=True)], 212))],
  [('gap', '2px between rows; 16px between groups', '2px'), ('indent', '28px for rows that belong to the row above', 'none')],
  ['Used by the side bar and the palette list.', 'A stack scrolls when it runs out of height. It never shrinks its rows.']),
 card('Track', 'A rounded container holding items of one height, inset by 2px, so the items read as one group.',
  [('the shape', track_d), ('the open session', track_real)],
  [('inset', 'Space between the track and its items', '2px'), ('gap', 'Space between items', '2px'), ('radius', '10px outside, 8px on the items', ''), ('fill', 'The session colour at 18%', '')],
  ['Used by the open session in the top bar.', 'The inner radius is the outer radius less the inset, so the corners stay parallel.']),
 card('Split', 'Two areas sharing one space, with a gap between them that is also the handle. Either area can be split again.',
  [('across', mini([(4, 4, 114, 132, True, ''), (122, 4, 114, 132, False, '')])), ('down', mini([(4, 4, 232, 64, True, ''), (4, 72, 232, 64, False, '')])), ('nested', mini([(4, 4, 114, 132, False, ''), (122, 4, 114, 64, True, ''), (122, 72, 114, 64, False, '')])), ('the handle', handle('hover'))],
  [('direction', 'across, down', 'across'), ('share', 'How much the first area takes', 'half'), ('gap', 'Drawn 6px wide, grabbed across 12px', '6px'), ('smallest', 'The least either area may be', '200 by 110px')],
  ['Used by the panes of a tab, and between the window edge and the panes. The rules for splitting, closing and resizing are on the Pane layout card.', 'The gap is the same 6px everywhere, so the ground shows as an even line round every pane.']),
 card('Overlay', 'One surface over everything else, with the rest dimmed behind it.',
  [('the shape', overlay_d)],
  [('width', 'Fixed', '640px'), ('from the top', 'Fixed, so the input does not move as results change', '120px'), ('height', 'Grows with its contents, to at most 60% of the window', ''), ('scrim', 'Black at 60% in dark, ink at 28% in light', '')],
  ['Used by the command palette, and by nothing else: steg has no dialogs.', 'In a window narrower than 720px the overlay keeps 40px clear on each side instead of its fixed width.', 'Escape or a click on the scrim closes it.']),
]
board('Layout.dc.html', 'Layout and resizing', 'Design system · 1 of 5 · Foundations', 'The handful of arrangements every part of steg is built from, and the one rule for what happens when room runs out. Values such as gaps and radii are on the first Foundations board; this board says how they combine.', lay, 4300, [('Organisms.dc.html', 'Open pane layout')])
mol = [
 card('Tab', 'One tab of the open session. The same part is used in the top bar and, indented, in the side bar.', [('at rest', tab('tests')), ('with a status', tab('tests', status='working')), ('with a status, under the pointer', tab('tests', hover=True, status='working')), ('selected', tab('shell', sel=True)), ('selected, under the pointer', tab('shell', sel=True, hover=True)), ('short of room', tab('integration-tests', status='working', width=100)), ('smallest', tab('integration-tests', status='working', width=72)), ('two panes: the tab shows the more pressing', '<span style="display: flex; flex-direction: column; gap: 8px">' + tab('deploy', status='blocked') + '<span style="display: flex; gap: 6px">' + caption('terraform', '', status='working', width=236, drop=1) + caption('agent', '', status='blocked', width=236, drop=1) + '</span></span>'), ('in the side bar', '<span style="width: 212px; display: flex">%s</span>' % tab('tests', status='blocked', width=None, side=True))],
  [('title', 'Text, set by the program or the person', 'required'), ('selected', 'yes, no', 'no'), ('status', 'A status, or none', 'none'), ('width', '72 to 240px along the top; full width on the side', '120px')],
  ['The right end of every tab is one slot, 22px wide, so nothing ever moves sideways. At rest it shows the ring, or nothing. Under the pointer, or with keyboard focus, it shows the close button in the same place.', 'A selected tab shows no ring. Its panes are in view and their headers already carry the status. It has no permanent close button either: the pointer is on it when you select it, and Cmd+W closes it.', 'Every tab in a track is the same width. They shrink together as tabs are added, and at 72px the track scrolls instead.', 'A tab shows one ring however many panes it holds: the most pressing of them. Panes are never listed in the bar; each shows its own status on its header, and the palette lists them one by one.'],
  sizes=[('Compact', tab('integration-tests', status='working', width=72), 'Start of the title, and the ring', '72px: the least a tab may be'), ('Regular', tab('tests', status='working', width=120), 'Title and ring', '120px: the default'), ('Full', tab('integration-tests', status='working', width=240), 'The whole title and the ring', 'Up to 240px, when the title needs it')],
  drops='the end of the title. Never the slot at the right end.'),
 card('Session row', 'One session in the side bar.', [('closed', srow('homelab')), ('closed, under the pointer', srow('homelab', hover=True)), ('open', srow('steg', True)), ('open, under the pointer', srow('steg', True, hover=True)), ('the header above them', '<span style="width: 212px; display: block">%s</span>' % shead())],
  [('name', 'Text', 'required'), ('open', 'yes, no', 'no'), ('colour', 'A session colour', 'violet')],
  ['The open session is tinted in its own colour. A closed session shows no colour.', 'The right end has two fixed slots, so names never shift. The first is + for a new tab: shown at rest on the open session only, since a new tab in a closed session would have to open it first. The second is x to end the session: shown under the pointer only.', 'Ending a session always asks first, whether or not anything is running in it.', 'A new session is the + in the header beside the label. It is always shown.', 'A session never shows a status of its own, open or closed. Status belongs to tabs; what needs you in a closed session is listed first in the palette.']),
 card('Session chip', 'One closed session in the top bar.', [('at rest', chip('homelab')), ('under the pointer', chip('homelab', hover=True)), ('short of room', chip('as'))],
  [('name', 'Text', 'required'), ('short', 'yes, no', 'no')],
  ['Clicking it opens that session into the track.', 'All chips change size together, so the bar never mixes long and short names.'],
  sizes=[('Compact', chip('as'), 'Two letters', 'When the tabs would otherwise go below 120px'), ('Regular', chip('asdlc'), 'The name', 'The default')],
  drops='the rest of the name, on every chip at once, before any tab shrinks.'),
 card('Pane caption', 'The strip along the top of a pane.', [('active', caption('zsh', '~/src/steg', True)), ('not active, quiet', caption('zsh', '~/src/steg')), ('running, 62%', caption('cargo test', 'steg-kernel', status='working')), ('active and running, 62%', caption('cargo test', 'steg-kernel', True, status='working')), ('running, progress unknown', caption('agent', '~/src/steg', status='spinner')), ('waiting for you, at 40%', caption('deploy', '~/src/pluto', status='blocked')), ('finished, not yet looked at', caption('just build', '~/src/steg', status='done')), ('failed, at 80%', caption('steg-server', '~/src/steg', status='error')), ('a bell rang', caption('make', '~/src/steg', status='bell'))],
  [('title', 'The running program', 'required'), ('directory', 'A path, or a note from the program', 'none'), ('active', 'yes, no', 'no'), ('status', 'A status, or none', 'none'), ('progress', '0 to 100, or unknown', 'unknown')],
  ['The header shows status as the ring rolled out flat: the same grey track and coloured fill as a 120px bar. When the task stops, the bar parts round the same mark, which sits in the middle as it does in the ring.', 'The ring is for what is out of view, in the bar. The flat bar is for what is in view, in the pane. They are one thing in two shapes, and always agree.', 'Running fills with the accent ramp, now as a smooth blend. Waiting and failed stop where they were, in amber or red. Finished is full and green.', 'The strip itself only ever says one thing: the active pane carries the ramp at half strength.'], 'column',
  sizes=[('Full', caption('cargo test', 'steg-kernel', True, status='working', width=440), 'Title, directory, status bar, four buttons', '360px or wider'),
         ('Regular', caption('cargo test', 'steg-kernel', True, status='working', width=330, drop=1), 'Title, status bar, four buttons', 'Under 360px'),
         ('Narrow', caption('cargo test', 'steg-kernel', True, status='working', width=240, drop=2), 'Title and four buttons', 'Under 260px'),
         ('Compact', caption('cargo test', 'steg-kernel', True, status='working', width=160, drop=3), 'Title and close', 'Under 200px')],
  drops='the directory, then the status bar, then split and fill. Never the title or close. A status dropped here is still on the tab\'s ring in the bar.'),
 card('Palette row', 'One result in the command palette.', [('a place, quiet', prow('zsh', 'homelab · shell', '~/src/pluto', ic('pane', MUTED), width=480)), ('a place, with a status', prow('deploy', 'steg · agent', '~/src/pluto', ball('blocked'), width=480)), ('selected', prow('cargo test', 'steg · tests', '~/src/steg', ball('working'), sel=True, width=480)), ('an action', prow('Pane: Split right', key='⌘D', weight=400, width=480)), ('an action from a plugin', prow('Git: Split hunk into commits', right='plugin · git-tools', weight=400, width=480)), ('a choice', prow('Violet', right='current', lead=swatch('{{s.violet}}'), width=480))],
  [('kind', 'place, action, choice', 'required'), ('main', 'Text', 'required'), ('detail', 'Where it is, for a place', 'none'), ('key', 'A shortcut, for an action', 'none'), ('selected', 'yes, no', 'no')],
  ['A place leads with its status ring, or the pane icon when quiet. Here the icon leads, because every row has one and nothing shifts.', 'An action is named by what it acts on, then what it does.'], 'column'),
 card('Palette input', 'Where the palette is typed into. Its first character chooses the mode.', [('go to, empty', pinput('empty', 480)), ('go to, typed', pinput('typed', 480)), ('actions', pinput('actions', 480)), ('follow-up step', pinput('step', 480))],
  [('text', 'What has been typed', 'empty'), ('step', 'The action waiting for an answer, or none', 'none')],
  ['Cmd+P opens it empty; Shift+Cmd+P opens it with > typed. Backspace over the > returns to go to.', 'In a follow-up step the action shows as a chip, and Backspace on an empty input goes back to the actions.'], 'column'),
 card('Split handle', 'The gap between two panes, which is also how they are resized.', [('at rest', handle('rest')), ('under the pointer', handle('hover')), ('while dragging', handle('drag'))],
  [('direction', 'across, down', 'across')], ['Drawn 6px wide, grabbed across 12px. A double click makes the two sides equal.']),
 card('Empty result', 'What the palette shows when nothing matches.', [('no action matches', '<span style="width: 480px; display: flex; border-radius: 8px; background: %s">%s</span>' % (OVER, empty()))],
  [('query', 'What was typed', 'required'), ('mode', 'go to, actions, choice', 'required')], ['Always says what to try next. Nothing runs on Return while this is showing.']),
]
board('Molecules.dc.html', 'Molecules', 'Design system · 3 of 5', 'Small groups of atoms that do one job.' + HOW, mol, 8400)
L = lambda a, c, b='': (a, c, b)
shell = [L('❯ ', ST['done'], 'just check'), L('ok  ', ST['done'], 'fmt-check   0.4s'), L('ok  ', ST['done'], 'lint        12.1s'), L('ok  ', ST['done'], 'test        8.7s')]
tests = [L('running 42 tests', MUTED), L('test layout::split_right ... ', INK, ''), L('test pane::resize_keeps ... ', INK, ''), L('26 of 42', MUTED)]
dep = [L('+ ', ST['done'], 'k3s_node.worker[2]'), L('~ ', ST['blocked'], 'load_balancer.ingress'), L('Apply these changes?', INK), L('Enter a value: ', ST['blocked'])]
win = lambda scene: '<span style="width: 504px; height: 315px; display: block; overflow: hidden; border-radius: 10px"><span style="display: block; width: 1440px; height: 900px; overflow: hidden; transform: scale(0.35); transform-origin: 0 0; pointer-events: none"><dc-import name="App" scene="%s" theme="{{theme}}" still="{{yes}}" hint-size="1440px,900px"></dc-import></span></span>' % scene
org = [
 card('Bar, on the side', 'Every session, with the open one\'s tabs under it.', [('one session open', sidebar())],
  [('sessions', 'The sessions on this host, in order', 'required'), ('open', 'Which session is open', 'required'), ('selected', 'Which of its tabs is selected', 'required')],
  ['232px wide. Built from the header, the session row and the tab, with the buttons for moving the bar and opening the palette at the top. New session is the + in the header; new tab is the + on the open session.', 'Only one session is open at a time; opening another closes this one.']),
 card('Bar, on top', 'The open session as a tinted track, with every other session as a chip beside it.', [('with room', topbar([('shell', None, 110), ('tests', 'working', 110), ('agent', 'blocked', 110)], [('asdlc', 'done'), ('homelab', None)], 1040)), ('short of room', topbar([('shell', None, 100), ('tests', 'working', 100), ('agent', 'blocked', 100), ('build', 'done', 100), ('server', 'error', 100)], [('as', 'done'), ('ho', None), ('do', 'blocked'), ('pl', None)], 1040))],
  [('sessions', 'The sessions on this host, in order', 'required'), ('open', 'Which session is open', 'required'), ('selected', 'Which of its tabs is selected', 'required')],
  ['40px high. Built from the tab, the session chip and icon buttons.', 'The bar is on the side or on top, never both. One button moves it.'], 'column'),
 card('Pane', 'One terminal: a caption and the program\'s screen.', [('active', pane('zsh', '~/src/steg', shell, True)), ('not active, working', pane('cargo test', 'steg-kernel', tests, status='working')), ('not active, blocked', pane('deploy', '~/src/pluto', dep, status='blocked')), ('active, with Increase Contrast', pane('zsh', '~/src/steg', shell, True, outline='border: 2px solid ' + ACC)), ('not active, with Increase Contrast', pane('cargo test', 'steg-kernel', tests, status='working', outline='border: 1px solid {{c.outline}}'))],
  [('active', 'yes, no', 'no'), ('program', 'What is running: title, directory, status', 'required')],
  ['The active pane is the lighter surface, with the ramp caption and a filled cursor. Other panes keep their text at full strength.', 'Outlines appear only when the macOS Increase Contrast setting is on.']),
 card('Pane layout', 'How the panes of one tab share the window: where a split comes from, where space goes back to, and how small a pane may get. A proposal; nothing here is in the spec yet.',
  [('one pane', mini([(4, 4, 232, 132, True, 'A')])),
   ('split right: the active pane halves, the new one is active', mini([(4, 4, 114, 132, False, 'A'), (122, 4, 114, 132, True, 'B')])),
   ('split down: only B is divided', mini([(4, 4, 114, 132, False, 'A'), (122, 4, 114, 64, False, 'B'), (122, 72, 114, 64, True, 'C')])),
   ('close C: its space returns to B, which it came from', mini([(4, 4, 114, 132, False, 'A'), (122, 4, 114, 132, True, 'B')])),
   ('smallest: the drag stops here, and B cannot be split', mini([(4, 4, 176, 132, True, 'A'), (184, 4, 52, 132, False, 'B')])),
   ('the window narrows: shares are kept', mini([(4, 4, 80, 132, False, 'A'), (88, 4, 80, 132, True, 'B')], w=172)),
   ('fill: B covers the tab, A keeps running', mini([(4, 4, 232, 132, True, 'B, filling')]))],
  [('split from', 'The active pane. It gives up half; the new pane takes that half, becomes active and starts in the same directory.', 'half and half'),
   ('smallest pane', '200px wide and 110px high: the caption and four lines of text.', 'proposal'),
   ('at the smallest', 'A drag stops. A split that would make either half smaller is refused, and the split button goes muted.', ''),
   ('close', 'The space returns to the pane it was split from. Nothing else moves.', ''),
   ('window resized', 'Every pane keeps its share. A pane that reaches the smallest size holds there and its neighbours give up the rest.', ''),
   ('fill', 'The active pane covers the tab. The others keep running and keep their shares for when it ends.', '')],
  ['Built from the Split primitive on the Layout board and the split handle. Splits nest: each split divides one pane and nothing else.', 'A pane that gets narrow drops parts of its caption in the order on the Pane caption card.', 'The window cannot be made smaller than its panes at their smallest sizes laid out as they are.', 'Open question: the layout is data the server holds. Whether two windows on the same session share one layout or each keep their own is not decided. It is the same question as where focus lives (DRAFT-3).'],
  heads=('Rule', 'What happens', 'Value')),
 card('Command palette', 'One box for going somewhere, running an action, and answering what an action asks.', [('go to', palette('goto')), ('actions', palette('actions')), ('follow-up step', palette('step')), ('nothing matches', palette('none'))],
  [('mode', 'go to, actions, help; chosen by the first character typed', 'go to'), ('step', 'An action waiting for an answer, or none', 'none')],
  ['640px wide in the window, 120px from the top; drawn narrower here. Built from the palette input, palette rows, labels and key caps.', 'With nothing typed, go to lists what needs you first, then what is recent.']),
 card('Status in and out of view', 'Which of the two status atoms shows, and when one becomes the other. This is the window\'s rule: it needs to know both the bar and the panes. A proposal; the motion is drawn on the Status board.',
  [('out of view: the ring, on the tab', tab('tests', status='working')), ('in view: the bar, in the pane header', caption('cargo test', 'steg-kernel', status='working', width=360)), ('out of view, waiting', tab('agent', status='blocked')), ('in view, waiting', caption('deploy', '~/src/pluto', status='blocked', width=360))],
  [('out of view', 'The pane\'s tab shows the ring. A closed session shows nothing; panes that need you are listed first in the palette.', ''),
   ('comes into view', 'The bar in the pane\'s header rolls out from a ring: by opening its tab, changing session, or leaving fill.', '250ms'),
   ('starts in view', 'A status that begins in a pane you are looking at rolls out the same way.', '250ms'),
   ('in view', 'Each pane header shows its own bar. The selected tab shows no ring: the bars say it.', ''),
   ('cleared', 'A key press or click in the pane clears finished, failed and the bell; finished also goes after 10 minutes and the bell after 30 seconds. Opening the tab is not enough. The bar rolls back up and goes.', '250ms'),
   ('reduce motion', 'The bar appears and goes without rolling.', '')],
  ['Built from the status ring and the status bar, the tab and the pane caption.', 'The ring is for what you cannot see; the bar is for what you can. They always agree, so rolling one into the other never changes what is being said.'],
  'column', heads=('When', 'What happens', 'Takes')),
 card('Window', 'The whole app: the bar and the open tab\'s panes. This is the template the organisms sit in.', [('bar on the side', win('e1')), ('bar on top', win('e2'))],
  [('bar', 'side, top', 'side'), ('theme', 'dark, light; follows macOS', 'dark')],
  ['These two are the live app from the Interactive app page, held still. Go there to use it, and to the Storyboards page to see it in sequence.']),
]
board('Organisms.dc.html', 'Organisms and the window', 'Design system · 4 and 5 of 5', 'Whole parts of the window, built from molecules and atoms, and then the window itself.' + HOW, org, 7200, [('App.dc.html', 'Open the interactive app'), ('Storyboards.dc.html', 'Open the storyboards'), ('Status.dc.html', 'Open the status board')])
# ---- states and flows: what the window shows when something is missing, ended, or being asked
def pal(inp, rows, foot, width=500, chip=None):
    """A command palette with any input text, rows and footer."""
    caret = '<span aria-hidden="true" style="display: inline-block; width: 1.5px; height: 17px; vertical-align: -3px; margin-left: 1px; background: %s"></span>' % ACC
    chipm = '<span style="font-size: 13px; color: %s; background: %s; border-radius: 8px; padding: 3px 10px; white-space: nowrap">%s</span>' % (INK2, QUIET, chip) if chip else ''
    head = '<span style="box-sizing: border-box; display: flex; align-items: center; gap: 10px; padding: 16px 16px 10px 16px"><span style="display: flex">%s</span>%s<span style="font-size: 15px; flex-grow: 1">%s%s</span>%s</span>' % (ic('search', MUTED), chipm, inp, caret, keycap('esc'))
    footer = '<span style="display: flex; align-items: center; gap: 14px; padding: 6px 16px 12px 16px; font-size: 12px; color: %s">%s</span>' % (MUTED, ''.join('<span style="flex-grow: %d">%s</span>' % (1 if i == 0 else 0, f) for i, f in enumerate(foot)))
    return '<span style="width: %dpx; display: flex; flex-direction: column; overflow: hidden; border-radius: 14px; background: %s">%s<span style="display: flex; flex-direction: column; gap: 2px; padding: 6px">%s</span>%s</span>' % (width, OVER, head, ''.join(rows), footer)
def window(side, main, w=520, h=280):
    return '<span style="width: %dpx; height: %dpx; box-sizing: border-box; display: flex; gap: 6px; padding: 6px; border-radius: 12px; background: %s"><span style="width: 150px; flex-shrink: 0; display: flex; flex-direction: column; gap: 2px; padding: 8px 4px">%s</span><span style="flex-grow: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; text-align: center">%s</span></span>' % (w, h, QUIET, side, main)
side_empty = shead(0)
empty_first = window(side_empty, '<span style="font-size: 15px; font-weight: 600">No sessions yet</span><span style="color: %s; max-width: 260px">A session is a group of tabs that keeps running when you close this window.</span><span style="display: flex; gap: 8px; padding-top: 6px">%s</span><span style="font-size: 12px; color: %s">%s to go to anything</span>' % (INK2, button('New session', key='⌘N'), MUTED, '⌘P'))
empty_gone = window(shead(0) + srow('homelab', width=None), '<span style="font-size: 15px; font-weight: 600">steg has ended</span><span style="color: %s; max-width: 260px">Its last tab was closed. Nothing else is open in this window.</span><span style="display: flex; gap: 8px; padding-top: 6px">%s</span>' % (INK2, button('New session', key='⌘N')))
ex_ok = [L('❯ ', ST['done'], 'just build'), L('ok  ', ST['done'], 'build       41.2s'), L('', MUTED), L('Program ended. ', MUTED, ''), L('↵ run it again   ⌘W close', MUTED, '')]
ex_bad = [L('❯ ', ST['done'], 'steg-server --listen'), L('error: ', ST['error'], 'address already in use'), L('', MUTED), L('Program ended with code 1. ', MUTED, ''), L('↵ run it again   ⌘W close', MUTED, '')]
def search_caption(width=440):
    field = '<span style="flex-grow: 1; height: 22px; display: flex; align-items: center; gap: 8px; padding: 0 8px; border-radius: 8px; background: %s">%s<span style="color: %s">split_right</span><span style="flex-grow: 1"></span><span style="color: %s">2 of 5</span></span>' % (G, ic('search', MUTED, 14), INK, MUTED)
    ctl = ''.join('<span style="width: 24px; height: 24px; display: flex; align-items: center; justify-content: center">%s</span>' % ic(k, INK2, 14 if k == 'close' else 16) for k in ('chevron-down', 'close'))
    return '<span style="height: 30px; width: %dpx; box-sizing: border-box; display: flex; align-items: center; gap: 8px; padding: 0 8px 0 14px; background: %s; font-size: 12px; color: %s"><span style="color: %s; font-weight: 600">cargo test</span>%s%s</span>' % (width, RAMP50, INK2, INK, field, ctl)
def hit(t, cur=False): return '<span style="border-radius: 3px; background: %s; color: %s">%s</span>' % (ACC if cur else QUIET, G if cur else INK, t)
search_pane = '<span style="width: 440px; height: 170px; display: flex; flex-direction: column; overflow: hidden; border-radius: 14px; background: %s">%s<span style="display: block; padding: 8px 14px; font-family: %s; font-size: 13px; line-height: 20px; white-space: pre">test layout::%s_halves_width ... ok\ntest layout::%s_keeps_focus ... ok\ntest layout::split_down_halves_height ... ok\ntest pane::%s_inherits_cwd ... ok\n<span style="color: %s">26 of 42</span></span></span>' % (RAISED, search_caption(), MONO, hit('split_right'), hit('split_right', True), hit('split_right'), MUTED)
def renaming(text, side=False):
    return '<span style="height: 30px; width: 180px; box-sizing: border-box; display: flex; align-items: center; gap: 6px; padding: 0 4px 0 %s; border-radius: 8px; background: %s"><span style="flex-grow: 1; height: 22px; display: flex; align-items: center; padding: 0 6px; margin-left: -6px; border-radius: 6px; background: %s; font-weight: 600"><span style="background: %s; border-radius: 3px">%s</span></span></span>' % ('28px' if side else '12px', RAISED, G, TINT, text)
states = [
 card('Empty window', 'What the window shows when there is nothing to show. It always says what a session is and offers the one thing to do next.',
  [('first run: no sessions', empty_first), ('the open session ended, others remain', empty_gone)],
  [('no sessions', 'The bar keeps its header with the + for a new session. The middle explains and offers the same action as a button with its shortcut.', ''), ('last tab closed', 'Closing a session\'s last tab ends the session. The window says so and does not jump to another session by itself.', ''), ('after ending', 'The other sessions stay listed in the bar; clicking one opens it.', '')],
  ['Built from the bar, the button and key caps. There is no illustration and no welcome tour.', 'The window never opens a session you did not ask for, so where you land is always your choice.'],
  'column', heads=('When', 'What happens', '')),
 card('A pane whose program has ended', 'The pane stays, with everything the program printed, and says what you can do with it.',
  [('ended cleanly', pane('just build', '~/src/steg', ex_ok, True, status='done', width=360, prompt=False)), ('ended with an error', pane('steg-server', '~/src/steg', ex_bad, True, status='error', width=360, prompt=False))],
  [('ended', 'The output stays. Two muted lines are added under it: that the program ended, with its code if not zero, and the two keys.', ''), ('status', 'Code zero is finished; any other code is failed. Ring and bar follow the usual rules and lifetimes.', ''), ('Return', 'Runs the same command again in the same pane and directory.', '↵'), ('close', 'Closes the pane; its space goes back as on the Pane layout card.', '⌘W'), ('a shell', 'A pane started as a plain shell closes when the shell exits, as terminals do. This card is for panes started with a command.', '')],
  ['The pane is never closed for you when a command ends: its output is usually the reason you ran it.', 'The added lines are steg\'s, not the program\'s, so they are muted and are not part of what can be copied or searched.'],
  heads=('When', 'What happens', 'Key')),
 card('Search in a pane', 'Finding text in what a pane has printed. The field takes the place of the directory in the pane\'s own caption.',
  [('searching, second of five matches', search_pane)],
  [('open', 'The directory in the caption becomes the field. The status bar and split buttons step aside while it is open.', '⌘F'), ('matches', 'Every match gets a quiet fill. The current one is filled in the session colour, with the text in the ground colour so it stays readable.', ''), ('next, previous', 'Moves the current match and scrolls it into view.', '↵  ⇧↵'), ('close', 'The caption returns to what it was. The scroll position stays where the last match was.', 'esc')],
  ['Search belongs to one pane, so it lives in that pane. It is not an overlay and does not use the palette.', 'This is the one place the session colour sits behind text; the text switches to the ground colour to keep its contrast.'],
  'column', heads=('When', 'What happens', 'Key')),
 card('Renaming a tab or a session', 'Names are edited where they are. Nothing opens.',
  [('a tab, in the top bar', renaming('integration')), ('a tab, in the side bar', renaming('integration', True)), ('a session', '<span style="height: 30px; width: 180px; box-sizing: border-box; display: flex; align-items: center; gap: 8px; padding: 0 4px 0 8px; border-radius: 8px; background: %s">%s<span style="flex-grow: 1; height: 22px; display: flex; align-items: center; padding: 0 6px; margin-left: -6px; border-radius: 6px; background: %s; font-weight: 600"><span style="background: %s; border-radius: 3px">steg</span></span></span>' % (TINT, ic('chevron-down', MUTED, 12), G, TINT))],
  [('start', 'Double click the name, or run "Tab: Rename" or "Session: Rename" from the palette.', ''), ('editing', 'The name becomes a field of the same size in the same place, with the old name selected.', ''), ('save', 'Keeps the new name. An empty name puts back the automatic one: the program in the active pane.', '↵'), ('cancel', 'Puts back the old name. Clicking elsewhere saves.', 'esc')],
  ['A name you give is kept until you clear it. A tab you never named follows the program in its active pane.'],
  heads=('When', 'What happens', 'Key')),
 card('Closing something that is still running', 'steg asks before it ends a program, using the palette, which is the only thing that ever opens over the window.',
  [('closing a tab with a running program', pal('', [prow('Keep it running', sel=True, weight=600), prow('Close anyway', right='ends cargo test', weight=400)], ['cargo test is still running', '↵ keep', 'esc keep'], chip='Close tab: tests')), ('ending a session: always asked', pal('', [prow('Keep the session', sel=True, weight=600), prow('End it', right='closes 5 tabs', weight=400)], ['2 programs are still running', '↵ keep', 'esc keep'], chip='End session: steg'))],
  [('asks', 'Closing a pane, tab or session that has a program running opens the palette at this step. The safe choice is selected.', ''), ('does not ask', 'A pane or tab at an idle shell prompt, or whose program has ended, closes at once.', ''), ('a session', 'Ending a session always asks, even when nothing in it is running: it closes every tab at once and cannot be undone.', ''), ('several', 'Closing a tab or session names how many programs are running and lists them in the footer.', ''), ('escape', 'Keeps everything, the same as choosing Keep it running.', 'esc')],
  ['Whether a program is running is the server\'s knowledge: a foreground process other than the shell.', 'The session keeps running if you close the window; this is only about closing the thing itself.'],
  'column', heads=('When', 'What happens', 'Key')),
 card('Palette: help', 'Typing ? lists what the palette can do. It is the third mode, beside go to and actions.',
  [('help', pal('?', [prow('Go to a session, tab or pane', right='type nothing first', weight=400, sel=True), prow('Run an action', right='type &gt; first', key='⇧⌘P', weight=400), prow('This list', right='type ? first', weight=400)], ['3 modes', '↵ switch to it', '⌫ to go to']))],
  [('open', 'Type ? as the first character.', '?'), ('choose', 'Return on a row clears the ? and puts the palette in that mode.', '↵')],
  ['Every mode is one character away from every other, and Backspace always leads back to go to.'],
  'column', heads=('When', 'What happens', 'Key')),
 card('Settings', 'There is no settings window. Settings are actions in the palette, and they are the same values the Python config file sets.',
  [('settings in the palette', pal('<span style="color: %s">&gt;</span>settings' % MUTED, [prow('Settings: Theme', right='follows macOS', weight=400, sel=True), prow('Settings: Bar position', right='side', weight=400), prow('Settings: Terminal font', right='JetBrains Mono 13', weight=400), prow('Settings: Open the config file', weight=400)], ['4 actions', '↵ change', '⌫ on &gt; to go to']))],
  [('open', 'Opens the palette with ">settings" already typed.', '⌘,'), ('a setting', 'Each row shows its current value on the right. Return opens a follow-up step with the choices.', '↵'), ('a change', 'Applies at once and lasts for this run of the server. It is an override on top of the config file, which is not rewritten.', ''), ('differs from the file', 'The row says so beside its value, for example "light, not in your config".', ''), ('keeping it', 'An action on that setting opens the config file at the right place, with the line to add.', ''), ('the file', 'There is one file, the Python config. The palette reads and changes the same values it sets; the last row opens it in a pane.', '')],
  ['The file is the only thing that lasts, and steg never writes to it: it is code a person wrote. This is how tools with a code config behave.', 'There is no second settings file.', 'Before the config file exists, in the first release, every setting is simply an override the server remembers.'],
  'column', heads=('When', 'What happens', 'Key')),
]
board('States.dc.html', 'States and flows', 'Design system · 4 of 5', 'What the window shows when something is missing, has ended, or needs an answer. Each is built from the organisms before it. Everything here is a proposal: none of it is in the spec yet.' + HOW, states, 5900, [('Organisms.dc.html', 'Open organisms')])
