"""Claude Code session scanning: discovery, tailing, lenient parsing, an order-independent fold.

Ported from zoetrope v0.2.0 (https://github.com/furkankly/zoetrope, MIT,
Copyright (c) 2026 Furkan Kalaycioglu; see LICENSES/zoetrope-MIT.txt), with the changes herdr-panorama needs (see
docs/DESIGN.md "Data layer"): completion notices from every carrier and every
file, ``killed``, SendMessage resumes, expiring abandoned tool calls, real
nesting, and several sessions at once. Module docstrings name the zoetrope
source each part follows.
"""
