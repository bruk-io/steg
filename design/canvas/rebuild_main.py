import re
def sub(s, old, new, n=1):
    assert s.count(old) == n, (old[:80], s.count(old)); return s.replace(old, new)
S = lambda d, w=2, extra='': '<path d="%s" fill="none" stroke="currentColor" stroke-width="%s" stroke-linecap="round" stroke-linejoin="round"%s></path>' % (d, w, extra)
F = lambda d: '<path d="%s" fill="currentColor" fill-rule="evenodd"></path>' % d
R = lambda x, y, w, h, rx, soft=False: '<rect x="%s" y="%s" width="%s" height="%s" rx="%s" fill="currentColor"%s></rect>' % (x, y, w, h, rx, ' opacity="0.35"' if soft else '')
INNER = {
 'working': '<circle cx="8" cy="8" r="6.5" fill="currentColor" opacity="FAINT"></circle><path d="M8 8V1.5A6.5 6.5 0 1 1 3.55 12.74z" fill="currentColor"></path>',
 'blocked': F('M6 2h4a4 4 0 0 1 4 4v4a4 4 0 0 1-4 4H6a4 4 0 0 1-4-4V6a4 4 0 0 1 4-4zM5.75 5.25h1.75v5.5H5.75zM8.5 5.25h1.75v5.5H8.5z'),
 'done': '<circle cx="8" cy="8" r="6.5" fill="currentColor"></circle>',
 'error': F('M8 1.8c.62 0 1.2.33 1.5.87l4.75 8.3c.66 1.16-.17 2.6-1.5 2.6h-9.5c-1.33 0-2.16-1.44-1.5-2.6l4.75-8.3c.3-.54.88-.87 1.5-.87zM7.15 5h1.7v4.2h-1.7zM7.15 10.3h1.7V12h-1.7z'),
 'plus': S('M8 3.5v9M3.5 8h9'), 'close': S('M4.5 4.5l7 7M11.5 4.5l-7 7'),
 'chevron-right': S('M6 3.5L10.5 8 6 12.5'), 'chevron-down': S('M3.5 6L8 10.5 12.5 6'),
 'bar-side': R(1.5, 2.5, 13, 11, 3, True) + F('M4.5 2.5h1.75v11H4.5a3 3 0 0 1-3-3v-5a3 3 0 0 1 3-3z'),
 'bar-top': R(1.5, 2.5, 13, 11, 3, True) + F('M4.5 2.5h7a3 3 0 0 1 3 3v1h-13v-1a3 3 0 0 1 3-3z'),
 'palette': S('M6 6h4v4H6zM6 6H4.5A1.5 1.5 0 1 1 6 4.5zM10 6V4.5A1.5 1.5 0 1 1 11.5 6zM10 10h1.5A1.5 1.5 0 1 1 10 11.5zM6 10v1.5A1.5 1.5 0 1 1 4.5 10z', 1.5),
 'search': '<circle cx="7" cy="7" r="4" fill="none" stroke="currentColor" stroke-width="2"></circle>' + S('M10.2 10.2l3.3 3.3'),
 'split-right': R(1.5, 2.5, 6, 11, 2.5, True) + R(8.5, 2.5, 6, 11, 2.5),
 'split-down': R(1.5, 2, 13, 5.5, 2.5, True) + R(1.5, 8.5, 13, 5.5, 2.5),
 'fill': S('M9.5 3.5h3v3M12.5 3.5L9 7M6.5 12.5h-3v-3M3.5 12.5L7 9'),
 'pane': R(1.5, 2.5, 13, 11, 3, True) + S('M4.5 6l2.5 2-2.5 2M8.75 10.25h2.75', 1.75),
}
LABEL = {'working': 'working, 62%', 'blocked': 'blocked', 'done': 'done', 'error': 'error'}
def icon(key, colour=None, size=16, faint='{{faint}}'):
    a = 'role="img" aria-label="%s"' % LABEL[key] if key in LABEL else 'aria-hidden="true"'
    col = '; color: %s' % colour if colour else ''
    return '<svg %s width="%d" height="%d" viewBox="0 0 16 16" style="flex-shrink: 0%s">%s</svg>' % (a, size, size, col, INNER[key].replace('FAINT', faint))

import math as _m
def wedge(deg):
    pts = ['50% 50%'] + ['%.1f%% %.1f%%' % (50 + 80 * _m.sin(_m.radians(deg + d)), 50 - 80 * _m.cos(_m.radians(deg + d))) for d in (0, 45, 90)]
    return 'polygon(' + ', '.join(pts) + ')'
def pill(pct, ramp, faint, width=28, label=None, scale=1):
    """Working, drawn as one object: a 13px ball with a turning quarter when pct is None, a pill that fills otherwise."""
    w, h = (13, 13) if pct is None else (width, 8)
    w, h = w * scale, h * scale
    fill = 'width: 100%%; clip-path: %s' % wedge(40) if pct is None else 'width: %d%%; background-size: %dpx 100%%' % (pct, w)
    lab = label or ('working' if pct is None else 'working, %d%%' % pct)
    return ('<span role="img" aria-label="%s" style="position: relative; display: block; flex-shrink: 0; width: %dpx; height: %dpx; border-radius: 999px; overflow: hidden">'
            '<span style="position: absolute; left: 0; top: 0; width: 100%%; height: 100%%; background: %s; opacity: %s"></span>'
            '<span style="position: absolute; left: 0; top: 0; height: 100%%; background: %s; %s"></span></span>') % (lab, w, h, ramp, faint, ramp, fill)

MARK = {'blocked': 'M6 5.6h1.3v4.8H6zM8.7 5.6H10v4.8H8.7z', 'error': 'M7.35 4.9h1.3v3.9h-1.3zM7.35 9.8h1.3v1.3h-1.3z', 'done': 'M5.3 8.2l.9-.9 1.3 1.3 2.4-2.6.95.9-3.3 3.5z',
        'bell': 'M8 1.5a4.25 4.25 0 0 0-4.25 4.25v2.4L2.5 10.6a.9.9 0 0 0 .8 1.4h9.4a.9.9 0 0 0 .8-1.4l-1.25-2.45v-2.4A4.25 4.25 0 0 0 8 1.5zM6.2 13h3.6a1.8 1.8 0 0 1-3.6 0z'}
CIRC = 37.699
SECT = ('r1', 'r2', 'error', 'r3', 'blocked', 'done')  # the six sections of a running ring, in order
def donut(kind, p, col, size=16, anim=False):
    """One status ring: a grey track, a coloured arc for how far along, and a mark in the middle when the state needs one.
    kind: working (p 0..1 or None), blocked, done, error, bell. col: track, r1, r2, r3, blocked, done, error."""
    if kind == 'working' and p is not None: arcs = [(k * 60, max(0.0, min(1 / 6, p - k / 6)) * 100, col[key]) for k, key in enumerate(SECT)]
    elif kind == 'working': arcs = [(40, 8.4, col['r1']), (70, 8.4, col['r2']), (100, 8.4, col['r3'])]
    else:
        c = col['blocked' if kind == 'bell' else kind]
        arcs = [(0, (1 if (p is None or kind in ('done', 'bell')) else p) * 100, c)]
    label = kind if kind != 'working' else ('working' if p is None else 'working, %d%%' % round(p * 100))
    out = '<svg role="img" aria-label="%s" width="%d" height="%d" viewBox="0 0 16 16" style="flex-shrink: 0"><circle cx="8" cy="8" r="6" fill="none" stroke="%s" stroke-width="2"></circle>' % (label, size, size, col['track'])
    spin = anim and kind == 'working' and p is None
    if spin: out += '<g class="an-spin">'
    for start, pct, c in arcs:
        if pct > 0: out += '<circle cx="8" cy="8" r="6" fill="none" stroke="%s" stroke-width="2" stroke-dasharray="%.2f %.2f" transform="rotate(%d 8 8)"></circle>' % (c, pct / 100 * CIRC, CIRC, start - 90)
    if spin: out += '</g>'
    if kind in MARK:
        tf = ' transform="translate(8 8.3) scale(0.5) translate(-8 -7.4)"' if kind == 'bell' else ''
        out += '<path d="%s" fill="%s"%s></path>' % (MARK[kind], col['blocked' if kind == 'bell' else kind], tf)
    return out + '</svg>'

def flat(kind, p, col, ramp, width=120, anim=False):
    """The status ring rolled out flat for a pane header. Running is one unbroken bar. A stopped state (waiting, failed, finished, bell)
    breaks the bar round its mark, which sits in the middle as it does in the ring; progress skips the gap."""
    label = kind if kind != 'working' else ('working' if p is None else 'working, %d%%' % round(p * 100))
    if kind == 'working':
        if p is None: fill = 'left: 35%%; width: 22%%; background: linear-gradient(90deg, %s, %s, %s)' % (col['r1'], col['r2'], col['r3'])
        else:
            pct = round(p * 100)
            fill = 'left: 0; width: %d%%; background: %s; background-size: %d%% 100%%' % (pct, ramp, 10000 / max(pct, 1))
        return '<span role="img" aria-label="%s" style="position: relative; display: block; flex-shrink: 0; width: %dpx; height: 4px; border-radius: 999px; overflow: hidden; background: %s"><span%s style="position: absolute; top: 0; height: 100%%; border-radius: 999px; %s"></span></span>' % (label, width, col['track'], ' class="an-slide"' if (anim and p is None) else '', fill)
    c = col['blocked' if kind == 'bell' else kind]
    hw = (width - 20) / 2
    tot = (1 if (p is None or kind in ('done', 'bell')) else p) * 2 * hw
    half = lambda side, w: '<span style="position: absolute; %s: 0; top: 5px; width: %dpx; height: 4px; border-radius: 999px; overflow: hidden; background: %s"><span style="display: block; width: %.1fpx; height: 4px; border-radius: 0 2px 2px 0; background: %s"></span></span>' % (side, hw, col['track'], w, c)
    mark = '<svg aria-hidden="true" width="14" height="14" viewBox="%s" style="position: absolute; left: %dpx; top: 0"><path d="%s" fill="%s"></path></svg>' % ('0 0 16 16' if kind == 'bell' else '3.5 3.5 9 9', width / 2 - 7, MARK[kind], c)
    broken = ''
    lw, rw = min(hw, tot), max(0, tot - hw)
    if kind == 'error':
        xe = round(width - hw + rw if rw > 0 else lw)
        if rw > 0: rw -= 4
        else: lw -= 4
        broken = '<svg aria-hidden="true" width="%d" height="14" viewBox="0 0 %d 14" shape-rendering="crispEdges" style="position: absolute; left: 0; top: 0"><path d="M%d 5h2v2h-2zM%d 7h4v2h-4z" fill="%s"></path></svg>' % (width, width, xe - 4, xe - 4, c)
    out = half('left', lw) + half('right', rw)
    if kind == 'error': out = out.replace('border-radius: 0 2px 2px 0', 'border-radius: 0')
    return '<span role="img" aria-label="%s" style="position: relative; display: block; flex-shrink: 0; width: %dpx; height: 14px">%s%s%s</span>' % (label, width, out, broken, mark)
