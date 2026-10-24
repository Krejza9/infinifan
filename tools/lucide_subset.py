"""Picks the Lucide icons InfiniFan uses: writes a subset font and src/icons.rs.

Usage: python tools/lucide_subset.py <folder with lucide.ttf + lucide.css from lucide-static> .
"""
import re, sys
from fontTools import subset

SRC, OUT = sys.argv[1], sys.argv[2]   # folder with lucide.ttf/css, infinifan crate
ICONS = [
    # group 1: navigation and status
    'fan', 'lightbulb', 'plug', 'settings', 'info', 'usb', 'unplug', 'cpu', 'monitor', 'gauge',
    'circle-alert', 'check', 'x', 'triangle-alert',
    # group 2: card titles
    'chart-spline', 'thermometer', 'languages', 'power', 'circuit-board', 'app-window', 'heart',
    'sliders-horizontal', 'layers', 'circle-dot', 'square',
    # group 3: buttons and options
    'scan-eye', 'download', 'shield', 'circle-stop', 'arrow-right', 'arrow-left', 'sun', 'palette', 'sparkles',
]
css = open(f'{SRC}/lucide.css', encoding='utf-8').read()
cp = {m.group(1): int(m.group(2), 16) for m in re.finditer(r'\.icon-([a-z0-9-]+):before\s*\{\s*content:\s*"\\([0-9a-f]+)"', css)}
missing = [i for i in ICONS if i not in cp]
assert not missing, missing

opts = subset.Options()
opts.layout_features = []
opts.name_IDs = ['*']
opts.notdef_outline = True
font = subset.load_font(f'{SRC}/lucide.ttf', opts)
sub = subset.Subsetter(opts)
sub.populate(unicodes=[cp[i] for i in ICONS])
sub.subset(font)
subset.save_font(font, f'{OUT}/assets/lucide/lucide.ttf', opts)

lines = [
    '//! Lucide icons (https://lucide.dev, ISC license, see assets/lucide/LICENSE).',
    '//! `assets/lucide/lucide.ttf` holds only these glyphs; the font is a fallback of the normal text',
    '//! font, so an icon can be written straight into any label, e.g. `format!("{CHECK} Save")`.',
    '//! To add an icon, add its name to research/lucide_subset.py and run it again.',
    '',
]
for i in ICONS:
    name = i.upper().replace('-', '_')
    lines.append(f'pub const {name}: &str = "\\u{{{cp[i]:x}}}";')
open(f'{OUT}/src/icons.rs', 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
print(len(ICONS), 'icons')
