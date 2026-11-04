const STATES = {
  "run": [
    "Running",
    "Running",
    "↻",
    "#68c2b5",
    "#1f625c",
    "#d8f2ee",
    "Actively working."
  ],
  "rev": [
    "Reviewing",
    "Review",
    "»",
    "#8eaee6",
    "#2e4f86",
    "#dfe8f8",
    "Explicit review work, not an inference."
  ],
  "wait": [
    "Needs answer",
    "Answer",
    "?",
    "#ffb547",
    "#f2a93b",
    "#1b1203",
    "Open questions; the count remains visible."
  ],
  "blk": [
    "Permission",
    "Permission",
    "!",
    "#ffb547",
    "#f2a93b",
    "#1b1203",
    "Blocked on permission; separate from a question."
  ],
  "fail": [
    "Failed",
    "Failed",
    "×",
    "#ff6b6b",
    "#d8363e",
    "#ffffff",
    "An observed failure, not a permission request."
  ],
  "done": [
    "Done",
    "Done",
    "✓",
    "#6c9673",
    "#2c6b4c",
    "#e3f2e9",
    "Explicit completion stays visible in green."
  ],
  "q": [
    "Queued",
    "Queued",
    "◷",
    "#8d939d",
    "#2a2e35",
    "#d2d6dd",
    "Waiting to start, not background work."
  ],
  "bg": [
    "Background",
    "Background",
    "⋯",
    "#b39ddb",
    "#3b3159",
    "#e6def8",
    "Background tasks remain; not idle."
  ],
  "cxl": [
    "Cancelled",
    "Cancelled",
    "−",
    "#89909b",
    "#24272c",
    "#c2c7cf",
    "Explicitly cancelled, not successful."
  ],
  "inf": [
    "Inferred done",
    "Inferred",
    "≈",
    "#8a9a8d",
    "#26312a",
    "#c4d3c8",
    "Completion inferred by the producer; not AI thinking."
  ],
  "lost": [
    "Lost",
    "Lost",
    "?",
    "#c08a8a",
    "#3a2526",
    "#e0b4b4",
    "Agent lost; distinguish from explicit failure."
  ],
  "stale": [
    "Stale",
    "Stale",
    "◴",
    "#89909b",
    "#1b1e23",
    "#c2c7cf",
    "Retained state is out of date; no death claim."
  ],
  "unknown": [
    "Unknown",
    "Unknown",
    "◇",
    "#c3c7cd",
    "#2a2e35",
    "#e2e5ea",
    "Keep the supplied unfamiliar code as plain text."
  ],
  "idle": [
    "Idle",
    "Idle",
    "○",
    "#8d939d",
    "#23272e",
    "#c2c7cf",
    "Native orchestrator idle; no background task claim."
  ]
};
function makeBadge(code,q){const key=code in STATES?code:'unknown',s=STATES[key];const glyph=key==='wait'?'?'+(q??''):s[2];return {cls:'state-badge st-'+key,t:glyph+' '+s[1],glyph,st:{}};}
class DesignFixture {
  state = { t: 0 };
  renderVals() {
    const T = this.state.t;
    const spinSt = { transform: 'rotate(' + ((T % 900) / 900 * 360).toFixed(1) + 'deg)' };
    const edgeSt = { opacity: 0.3 + 0.7 * (0.5 - 0.5 * Math.cos(2 * Math.PI * T / 1600)) };
    const bp = (T % 2400) / 2400;
    const ease = bp < 0.5 ? 2 * bp * bp : 1 - Math.pow(-2 * bp + 2, 2) / 2;
    const barSt = { left: (-28 + ease * 128).toFixed(2) + '%' };
    const TINT = { gemini: true, cursor: true };
    const ic = h => TINT[h]
      ? { WebkitMask: 'url(../icons/' + h + '.svg) center/contain no-repeat', mask: 'url(../icons/' + h + '.svg) center/contain no-repeat', background: '#eeeeef' }
      : { background: 'url(../icons/' + h + '.svg) center/contain no-repeat' };
    const NEED = { fail: true, blk: true, wait: true };
    const RANK = { fail: 7, blk: 6, wait: 5, rev: 4, run: 3, q: 2, done: 1 };
    const mark = (st,q) => makeBadge(st,q);

    const A = (role, h, st, act, x) => Object.assign({ role, h, st, act }, x || {});
    const SW = [
      { space: 'Test', title: 'Panorama as a herdr-app mod', h: 'codex', st: 'run', age: '4m', act: 'running /review-cycle on 3 PRs',  focused: true, threads: [
        { name: 'PR #142 sidebar mod', ph: 'dddf.', agents: [
          A('author', 'claude', 'done', 'pushed fix r1'),
          A('reviewer', 'codex', 'done', 'posted 0B 1H 1M'),
          A('fixer', 'claude', 'fail', 'npm test exited 1', { peek: true, kids: [A('explorer', 'claude', 'done', 'fetched the CI log')] }),
        ] },
        { name: 'PR #144 harness icons', ph: 'ddn..', agents: [
          A('author', 'claude', 'done', 'pushed r1'),
          A('reviewer', 'codex', 'rev', 'review r2 · 0B 1H so far', { kids: [
            A('explorer', 'claude', 'run', 'reading mods/agents/main.js', { hov: true, kids: [A('researcher', 'gemini', 'run', 'checking program.tint')] }),
          ] }),
        ] },
        { name: 'PR #145 retry banner copy', ph: 'w...', agents: [
          A('author', 'claude', 'wait', '"Retrying…" or "Reconnecting…"?', { q: 1 }),
        ] },
      ] },
      { space: 'TrafficLens', title: 'Data pipeline SDD', h: 'claude', st: 'blk', age: '2m', act: 'running the SDD plan, task 3/7',  doneThreads: 2, threads: [
        { name: 'Task 3/7 add retry cap', ph: 'ddw..', agents: [A('implementer', 'claude', 'blk', 'wants Bash(rm -rf build/)')] },
        { name: 'Task 4/7 dedupe tick events', ph: 'n...', agents: [
          A('implementer', 'cursor', 'run', 'running tests'),
          A('task-reviewer', 'claude', 'q', 'queued'),
        ] },
      ] },
      { space: 'Test', title: 'Herdr multi-account limit libraries', h: 'claude', st: 'done', age: '<1m', act: 'wrote the summary',  threads: [
        { name: 'Compare limiter crates', ph: 'ddd', agents: [A('researcher', 'claude', 'done', 'wrote the comparison')] },
        { name: 'Prototype token bucket', ph: 'dd', agents: [A('worker', 'claude', 'done', 'pushed a branch')] },
      ] },
      { space: '65net', title: 'Assess same-AZ proxy routing', h: 'codex', st: 'run', age: '22m', act: 'waiting on the hop measurements',  threads: [
        { name: 'Measure cross-AZ hops', ph: 'dn.', agents: [
          A('worker', 'codex', 'run', 'tracing eu-west-1 paths', { kids: [A('probe', 'claude', 'run', 'running mtr from 3 hosts')] }),
        ] },
        { name: 'Draft routing ADR', ph: '...', agents: [A('writer', 'gemini', 'q', 'queued until hops are in')] },
      ] },
      { space: 'TrafficLens', title: 'Docs refresh', h: 'amp', st: 'q', age: '1h', act: 'idle',  threads: [] },
    ];

    // V11: every agent carries the name its spawner gave it (Agent/Task description, Codex task name);
    // a root's children are its direct subagents. Threads remain in this data only for unchanged helpers.
    const NAME = {
      'pushed fix r1': 'Write PR #142 sidebar mod', 'posted 0B 1H 1M': 'Review PR #142', 'npm test exited 1': 'Fix PR #142 fold test',
      'fetched the CI log': 'Fetch the #142 CI log', 'pushed r1': 'Write PR #144 harness icons', 'review r2 · 0B 1H so far': 'Review PR #144 r2',
      'reading mods/agents/main.js': 'Explore icon call sites', 'checking program.tint': 'Check program.tint per harness',
      '"Retrying…" or "Reconnecting…"?': 'Write PR #145 retry banner', 'wants Bash(rm -rf build/)': 'Task 3/7: add retry cap',
      'running tests': 'Task 4/7: dedupe tick events', 'queued': 'Review task 4/7', 'wrote the comparison': 'Compare limiter crates',
      'pushed a branch': 'Prototype token bucket', 'tracing eu-west-1 paths': 'Measure cross-AZ hops', 'running mtr from 3 hosts': 'Run mtr from 3 hosts',
      'queued until hops are in': 'Draft routing ADR', 'mapped the icon call sites': 'Map icon call sites', 'tracing program.tint lookups': 'Trace program.tint lookups',
      'listed programs.json readers': 'List programs.json readers', 'checking the icon table on reload': 'Check icon table on reload',
      'paused · asks about the Gemini tile': 'Check the Gemini tile tint', 'rendering every icon at 14px': 'Render every icon at 14px',
      'compared tinted and full colour': 'Compare tinted and full colour', 'stopped: icons.spec.ts fails': 'Fix icons.spec.ts', 'scanning 6 icon sets': 'Scan 6 icon sets',
      'lobehub icons a…m': 'Index lobehub a–m', 'lobehub icons n…z': 'Index lobehub n–z', 'lucide: 41 icons': 'Index lucide', 'simple-icons: 12': 'Index simple-icons',
      'devicons: 7': 'Index devicons',
    };
    const nameAll = a => { a.name = a.name || (a.id === 's6' ? 'Index icon fonts' : NAME[a.act]) || (a.role[0].toUpperCase() + a.role.slice(1)); (a.kids || []).forEach(nameAll); return a; };
    const rootKids = o => o.threads.flatMap(t => t.agents).concat(Array.from({ length: o.doneThreads || 0 }, (_, k) =>
      A('implementer', 'claude', 'done', 'merged', { name: 'Task ' + (k + 1) + '/7: ' + ['schema', 'migrations'][k] }))).map(nameAll);
    SW.forEach(o => { o.kids = rootKids(o); });
    const all = a => [a].concat((a.kids || []).flatMap(all));
    const topOf = list => list.reduce((m, a) => (RANK[a.st] || 0) > (RANK[m] || 0) ? a.st : m, null);
    const qsum = list => list.filter(a => a.st === 'wait').reduce((s, a) => s + (a.q ?? 0), 0);
    const news = a => all(a).some(x => NEED[x.st]);
    const base = { cls: 'r', segs: [], hasIc: false, icCls: 'hic', icSt: {}, name: '', nameCls: 'nm', sub: '', subCls: 'sb', pips: [], hasPips: false, hasFold: false, foldCls: 'fold', chevCls: 'ui i-chev-d s11', foldN: '', age: '', marks: [], tiles: [], edge: false, edgeSt, bar: false, barSt };
    const row = o => Object.assign({}, base, o);
    const segs = (lead, last) => lead.map(l => l ? 'seg v' : 'seg').concat([last ? 'seg l' : 'seg t']);

    const secRow = (label, cls, n) => row({ cls: 'r sec', name: label, nameCls: 'secl ' + cls, sub: String(n), subCls: 'secn' });
    const orchRow = (o, folded, sel) => {
      const desc = o.threads.flatMap(t => t.agents.flatMap(all));
      const needs = [];
      if (desc.some(a => a.st === 'fail')) needs.push(mark('fail'));
      if (desc.some(a => a.st === 'blk') && o.st !== 'blk') needs.push(mark('blk'));
      const q = qsum(desc); if (q) needs.push(mark('wait', q));
      const own = mark(o.st === 'q' ? 'idle' : o.st);
      const top = topOf(desc);
      const n = o.threads.length + (o.doneThreads || 0);
      return row({
        cls: 'r orch state-' + (NEED[top] ? top : o.st === 'q' ? 'idle' : o.st) + (o.focused && sel ? ' sel' : ''), hasIc: true, icCls: 'hic h16', icSt: ic(o.h),
        name: o.space, nameCls: o.st === 'q' ? 'nm quiet' : 'nm', sub: '· ' + o.title,
        hasFold: folded && n > 0, chevCls: folded ? 'ui i-chev-r s11' : 'ui i-chev-d s11', foldN: folded ? String(n) : '',
        foldCls: folded ? 'fold ' + (NEED[top] ? 'need' : o.st === 'done' ? 'done' : '') : 'fold',
        age: o.age, marks: needs.map(m => ({...m,cls:m.cls+' mini',t:m.glyph})).concat([own]), edge: needs.length > 0 || o.st === 'blk',
        bar: desc.some(a => a.st === 'run' || a.st === 'rev'),
      });
    };
    const thRow = (t, sg) => {
      const fl = t.agents.flatMap(all);
      return row({ cls: 'r th state-' + topOf(fl), segs: sg, name: t.name, nameCls: 'nm grow th', subCls: 'nosb', hasPips: true, pips: t.ph.split('').map(c => 'pip ' + c), marks: [mark(topOf(fl), qsum(fl))] });
    };
    const agRow = (a, sg, mode, foldN) => row({
      cls: 'r ag state-' + a.st + (mode === 'full' && a.hov ? ' hov' : '') + (mode === 'peek' && a.peek ? ' pk' : ''),
      segs: sg, hasIc: true, icSt: ic(a.h), name: a.role, nameCls: 'nm ag', sub: '· ' + a.act, subCls: NEED[a.st] ? 'sb need' : 'sb',
      hasFold: !!foldN, chevCls: 'ui i-chev-r s11', foldN: foldN ? String(foldN) : '',
      marks: (mode === 'full' && a.hov ? [{ cls: 'mk pchip', t: 'Peek', st: {} }] : []).concat([mark(a.st, a.q)]),
      edge: !!NEED[a.st],
    });
    const entryRows = (o, sel) => {
      const r = orchRow(o, false, sel);
      const stc = r.cls.match(/state-[^ ]+/)[0];
      const own = r.marks[r.marks.length - 1];
      const act = o.threads.filter(t => t.agents.flatMap(all).some(a => a.st !== 'done'));
      const doneN = (o.doneThreads || 0) + o.threads.length - act.length;
      const tiles = act.map(t => { const fl = t.agents.flatMap(all), b = mark(topOf(fl), qsum(fl)); return { cls: b.cls + ' mini tile', t: b.glyph }; });
      if (doneN) tiles.push({ cls: 'state-badge st-done mini tile' + (doneN > 1 ? ' n' : ''), t: doneN > 1 ? '✓' + doneN : '✓' });
      const live = act.flatMap(t => t.agents.flatMap(all)).filter(a => a.st !== 'done').length;
      return [
        Object.assign({}, r, { cls: r.cls.replace(' sel', '') + ' entry-top', marks: [own], hasFold: false, edge: false, bar: false }),
        row({ cls: 'r tl entry-bot ' + stc, tiles, name: act.length + ' active · ' + live + ' agents', nameCls: 'nm tlc', subCls: 'nosb', bar: r.bar }),
      ];
    };
    const sumRow = (sg, text, sub) => row({ cls: 'r sum state-done', segs: sg, name: text, nameCls: 'nm sum', sub: sub ? '· ' + sub : '', subCls: sub ? 'sb' : 'nosb' });

    // Expanded: every agent and subagent.
    const treeFull = (list, lead, out) => list.forEach((a, i) => {
      const last = i === list.length - 1;
      out.push(agRow(a, segs(lead, last), 'full'));
      if (a.kids) treeFull(a.kids, lead.concat([!last]), out);
    });
    // Default: threads always; agents only on the path to something that needs you;
    // finished agents fold into one line, healthy subtrees into the fold count.
    const treeNews = (list, lead, out, mode) => {
      const shown = list.filter(news);
      const done = list.filter(a => a.st === 'done' && !news(a));
      const items = shown.map(a => ({ a })).concat(done.length ? [{ sum: done }] : []);
      items.forEach((it, i) => {
        const last = i === items.length - 1;
        if (it.sum) { out.push(sumRow(segs(lead, last), '✓ ' + it.sum.length + ' done', it.sum.map(a => a.role).join(', '))); return; }
        const a = it.a;
        const kidsNews = (a.kids || []).some(news);
        out.push(agRow(a, segs(lead, last), mode, kidsNews ? 0 : (a.kids || []).length));
        if (kidsNews) treeNews(a.kids, lead.concat([!last]), out, mode);
      });
    };
    const build = (mode, sel = true, data = SW) => {
      const out = [];
      const sections = [['Needs you', 'need', o => o.threads.some(t => t.agents.some(news)) || o.st === 'blk'], ['Done', 'done', o => o.st === 'done'], ['Working', 'working', o => o.st === 'run'], ['Idle', 'idle', o => o.st === 'q']];
      const used = new Set();
      sections.forEach(([label, cls, test]) => {
        const members = data.filter(o => !used.has(o) && test(o));
        if (!members.length) return;
        members.forEach(o => used.add(o));
        out.push(secRow(label, cls, members.length));
        members.forEach(o => {
          if (!o.focused) { out.push(...entryRows(o, sel)); return; }
          out.push(Object.assign(orchRow(o, false, sel), { cls: orchRow(o, false, sel).cls + ' entry-head' }));
          const kids = o.threads.map(t => ({ t })).concat(o.doneThreads ? [{ sum: o.doneThreads }] : []);
          kids.forEach((k, i) => {
            const last = i === kids.length - 1;
            if (k.sum) { out.push(sumRow(segs([], last), '✓ ' + k.sum + ' tasks done', 'schema, migrations')); return; }
            out.push(thRow(k.t, segs([], last)));
            if (mode === 'full') treeFull(k.t.agents, [!last], out);
            else if (k.t.agents.some(news)) treeNews(k.t.agents, [!last], out, mode);
          });
        });
      });
      return out;
    };

    const spaces = [
      { l: 'T', name: 'Test', repo: 'herdr-panorama', cls: 'sp on', tileSt: { background: '#22a99a' }, marks: [{ cls: 'mk chip need', t: '2', st: { marginLeft: '8px' } }, mark('run')] },
      { l: 'T', name: 'TrafficLens', repo: '', cls: 'sp', tileSt: { background: '#e3814f' }, marks: [{ cls: 'mk chip need', t: '1', st: {} }, mark('run')] },
      { l: '6', name: '65net', repo: '', cls: 'sp', tileSt: { background: '#5b8def' }, marks: [mark('run')] },
    ];

    const lines = [
      { t: '› Run the panorama mod PRs with /review-cycle (auto, github)', c: 'ln usr' },
      { t: ' ', c: 'ln' },
      { t: '• Spawned 3 threads: PR #142 sidebar mod, PR #144 harness icons,', c: 'ln hi' },
      { t: '  PR #145 retry banner copy.', c: 'ln hi' },
      { t: ' ', c: 'ln' },
      { t: '• Ran gh pr checks 142', c: 'ln hi' },
      { t: '  └ ✗ test   npm test exited 1 · 3 failing in sidebar.spec.ts', c: 'ln err' },
      { t: ' ', c: 'ln' },
      { t: '• Asked the operator (Q14): "Retrying…" or "Reconnecting…" for the banner?', c: 'ln hi' },
      { t: '  └ PR #145 waits on the answer; #142 and #144 go on meanwhile.', c: 'ln dim' },
      { t: ' ', c: 'ln' },
      { t: '• PR #144: the reviewer is on r2; its explorer reads mods/agents/main.js', c: 'ln hi' },
      { t: '  └ explorer → researcher (Gemini) is checking program.tint per harness', c: 'ln dim' },
      { t: ' ', c: 'ln' },
      { t: '• The #142 failure is real: tree rows are 26px and the fold test still', c: 'ln' },
      { t: '  expects 30px. I will send it back to the fixer after you look at it.', c: 'ln' },
      { t: ' ', c: 'ln' },
      { t: '◦ Working (2m 14s • esc to interrupt)', c: 'ln work' },
    ];

    // Frames D and D2: the Swarm tab, a herdr-app restyle of Swarm View 5 deep (5d):
    // orchestrators | threads | depth 1 | depth 2 | ... one column per depth of the selected thread's tree.
    const sh = (i, st, act, age, tools) => A('shard', 'codex', st, act, { id: 's' + i, model: 'mini', age, tools });
    const DEEP = [
      A('reviewer', 'codex', 'rev', 'review r2 · 0B 1H so far', { id: 'v1', model: 'gpt-5', age: '21m', tools: 52, kids: [
        A('explorer', 'claude', 'done', 'mapped the icon call sites', { id: 'e1', model: 'haiku', age: '19m', tools: 9 }),
        A('researcher', 'claude', 'run', 'tracing program.tint lookups', { id: 'r1', model: 'opus', age: '16m', tools: 38, kids: [
          A('explorer', 'claude', 'done', 'listed programs.json readers', { id: 'e3', model: 'haiku', age: '12m', tools: 7 }),
          A('researcher', 'gemini', 'run', 'checking the icon table on reload', { id: 'r2', model: 'gemini 2.5', age: '11m', tools: 24, kids: [
            A('checker', 'claude', 'wait', 'paused · asks about the Gemini tile', { id: 'c1', model: 'sonnet', age: '7m', tools: 11, q: 1, qs: ['Tint Gemini white like the other glyphs, or keep its purple tile?'], kids: [
              A('probe', 'claude', 'run', 'rendering every icon at 14px', { id: 'p1', model: 'haiku', age: '2m', tools: 4 }),
              A('probe', 'claude', 'done', 'compared tinted and full colour', { id: 'p2', model: 'haiku', age: '5m', tools: 6 }),
            ] }),
          ] }),
          A('fixer', 'cursor', 'fail', 'stopped: icons.spec.ts fails', { id: 'f1', model: 'cursor', age: '9m', tools: 14, err: 'npm test exited 1 · 2 failing' }),
        ] }),
        A('batch', 'codex', 'run', 'scanning 6 icon sets', { id: 'b1', model: 'gpt-5', age: '10m', tools: 19, kids: [
          sh(1, 'run', 'lobehub icons a…m', '4m', 12), sh(2, 'run', 'lobehub icons n…z', '4m', 9), sh(3, 'done', 'lucide: 41 icons', '9m', 31),
          sh(4, 'done', 'simple-icons: 12', '9m', 22), sh(5, 'done', 'devicons: 7', '8m', 27), sh(6, 'q', 'queued', '0m', 0),
        ] }),
      ] }),
      A('author', 'claude', 'done', 'pushed r1', { id: 'a2', model: 'sonnet', age: '44m', tools: 88 }),
    ];
    const T144 = { name: 'PR #144 harness icons', ph: 'dddn..', cur: 'review r2', agents: DEEP,
      phases: [['author', 'd', '09:02–09:20'], ['review r1', 'd', '09:24–09:38'], ['fix r1', 'd', '09:41–09:43'], ['review r2', 'n', 'since 09:43'], ['CI', 'x', ''], ['merge', 'x', '']] };
    const SW2 = SW.map((o, i) => i === 0 ? Object.assign({}, o, { threads: o.threads.map(t => t.name.startsWith('PR #144') ? T144 : Object.assign({ cur: t.name.startsWith('PR #142') ? 'CI · 3 failing' : 'author · waits on Q14' }, t)) }) : o);
    SW2.forEach(o => { o.kids = rootKids(o); });
    const spaceMarks = (data, name) => {
      const ag = data.filter(o => o.space === name).flatMap(o => o.threads.flatMap(t => t.agents.flatMap(all))), m = [];
      if (ag.some(a => a.st === 'fail')) m.push({ cls: 'state-badge st-fail mini', t: '×', st: {} });
      if (ag.some(a => a.st === 'blk') || data.some(o => o.space === name && o.st === 'blk')) m.push({ cls: 'state-badge st-blk mini', t: '!', st: {} });
      const q = qsum(ag); if (q) m.push({ cls: 'state-badge st-wait mini', t: '?' + q, st: {} });
      if (ag.some(a => a.st === 'run' || a.st === 'rev') || data.some(o => o.space === name && o.st === 'run')) m.push({ cls: 'state-badge st-run mini', t: '↻', st: {} });
      return m;
    };
    spaces.forEach(sp => { sp.marks = spaceMarks(SW, sp.name); });
    const spacesD = spaces.map(sp => Object.assign({}, sp, { marks: spaceMarks(SW2, sp.name) }));
    const LIVE = a => a.st !== 'done' && a.st !== 'cxl';
    const SHOWN = a => LIVE(a) || a.st === 'fail' || (a.kids || []).some(SHOWN);
    const depthOf = (list, d) => Math.max(0, ...list.filter(SHOWN).map(a => Math.max(d, depthOf(a.kids || [], d + 1))));
    const LBLT = { run: 'running', rev: 'reviewing', wait: 'needs answer', blk: 'blocked', fail: 'failed', done: 'done', q: 'queued' };
    const POD = st => 'pod ' + (st === 'wait' || st === 'blk' ? 'need' : st);
    const OW = 236, TW = 300, DW = 272, CG = 36, GW = 1230, CH = 676, VG = 10;
    const PHG = { d: '✓', n: '▸', f: '✗', w: '?', x: '·' };
    const deep = (selId, focusCol) => {
      const roots = SW2.filter(o => o.space === 'Test');
      const O = roots[0];
      const flat = [];
      const walk = (list, d, parent) => list.forEach(a => { a._d = d; a._p = parent; flat.push(a); walk(a.kids || [], d + 1, a); });
      walk(O.kids, 1, null);
      const selA = selId ? flat.find(a => a.id === selId) : null;
      const path = []; for (let a = selA; a; a = a._p) path.unshift(a);
      const onPath = new Set(path);
      const D = depthOf(O.kids, 1);
      const cols = [{ w: DW, t: 'Depth 0', n: roots.length }]
        .concat(Array.from({ length: D }, (_, i) => ({ w: DW, t: 'Depth ' + (i + 1), n: flat.filter(a => a._d === i + 1).length })));
      const last = cols.length - 1;
      const span = (lo, hi) => { let n = 0; for (let i = lo; i <= hi; i++) n += cols[i].w + (i > lo ? CG : 0); return n; };
      const f = focusCol, fit = GW - 24;
      let lo = f <= 1 ? 0 : f - 1, hi = Math.min(last, Math.max(f + 1, 2));
      while (hi > f && span(lo, hi) > fit) hi--;
      while (lo < f && span(lo, hi) > fit) lo++;
      while (hi + 1 <= last && span(lo, hi + 1) <= fit) hi++;
      while (lo > 0 && span(lo - 1, hi) <= fit) lo--;
      let x = lo === 0 ? 0 : 24;
      for (let i = lo; i <= hi; i++) { if (i > lo) x += CG; cols[i].x = x; x += cols[i].w; }
      const vis = i => i >= lo && i <= hi;
      const nodes = [], lines = [];
      const L = (cls, x, y, w, h) => lines.push({ cls, st: { left: x + 'px', top: y + 'px', width: Math.max(0, w) + 'px', height: Math.max(0, h) + 'px' } });
      const link = (px, py, cx, kids, onFn) => {
        if (!kids.length) return;
        const sx = cx - CG / 2;
        L(kids.some(onFn) ? 'dl on' : 'dl', px, py - 0.75, sx - px, 1.5);
        const ys = kids.map(k => k.y + k.mid);
        const top = Math.min(py, ys[0]), bot = Math.max(py, ...ys);
        L('dl', sx - 0.75, top, 1.5, bot - top);
        kids.forEach(k => L(onFn(k) ? 'dl on' : 'dl', sx, k.y + k.mid - 0.75, cx - sx, 1.5));
      };
      const N = o => {
        const n = Object.assign({ cls: 'dn', hasIc: false, icCls: 'hic', icSt: {}, pods: [], title: '', tCls: 'dn-t1', meta: '', label: '', labelCls: 'dn-lab none', marks: [], two: true, hasPips: false, pips: [], sub: '', sCls: 'gs', right: '', hasDetail: false, detail: [], hasActs: false, acts: [], foot: '', edge: false, bar: false, mid: 27 }, o);
        n.st = { left: n.x + 'px', top: n.y + 'px', width: n.w + 'px', height: n.h + 'px' };
        nodes.push(n); return n;
      };
      // depth 0: the space's root sessions, the same card as any agent; "tab N" marks the pane a click focuses
      roots.forEach((o, i) => {
        const r = orchRow(o, false, false), y = i * 64;
        if (vis(0)) N({ cls: 'dn ' + r.cls.match(/state-[^ ]+/)[0] + (o === O ? ' path' : '') + (o.st === 'done' ? ' dimmed' : ''), x: cols[0].x, y, w: DW, h: 54, hasIc: true, icSt: ic(o.h),
          title: o.title, tCls: 'dn-t1' + (o === O ? ' hi' : ''), meta: '', marks: r.marks,
          sub: o.act || (o.st === 'done' ? 'finished' : 'working'), right: 'tab ' + (i + 1) + ' · ' + o.age, edge: r.edge, bar: r.bar });
      });
      const detailOf = a => [{ cls: 'dd', k: 'now', t: a.act, r: '' }]
        .concat((a.qs || []).map(q => ({ cls: 'dd need', k: '?', t: q, r: '' })))
        .concat(a.err ? [{ cls: 'dd need', k: '✗', t: a.err, r: '' }] : []);
      const dH = a => detailOf(a).reduce((s, l) => s + Math.ceil(l.t.length / 34) * 17 + 3, 0);
      const cur = {}, trunk = {}, groups = [];
      const place = (list, d, y0, parent) => {
        const lv = list.filter(SHOWN), fin = list.filter(a => !SHOWN(a)), kids = [];
        const at = (n, hasKids) => {
          n.y = Math.max(cur[d] ?? 0, y0, hasKids && trunk[d + 1] != null ? trunk[d + 1] - n.mid + 16 : -1e9);
          cur[d] = n.y + n.h + VG; kids.push(n);
        };
        lv.forEach(a => {
          const n = { a, d, h: a === selA ? 88 + dH(a) : 54, mid: 27 };
          const hk = (a.kids || []).length > 0;
          at(n, hk);
          if (hk) place(a.kids, d + 1, n.y, n);
        });
        if (fin.length) at({ fold: fin, d, h: 40, mid: 20 }, false);
        const lk = kids[kids.length - 1];
        trunk[d] = Math.max(trunk[d] ?? -1e9, lk.y + lk.mid);
        groups.push({ parent, d, kids });
      };
      place(O.kids, 1, 0, null);
      const more = [];
      groups.forEach(g => g.kids.forEach(n => {
        const ci = n.d; if (!vis(ci)) return;
        const c = cols[ci];
        if (n.fold) {
          N({ cls: 'dn dfold state-done', x: c.x, y: n.y, w: c.w, h: 40, mid: 20, two: false, pods: n.fold.slice(0, 4).map(a => POD(a.st)),
            title: n.fold.length + ' finished · ' + n.fold.map(a => a.name).join(', '), tCls: 'dn-t1 fin' });
          return;
        }
        const a = n.a, isSel = a === selA, onP = onPath.has(a), kidsOff = (a.kids || []).some(SHOWN) && !vis(ci + 1);
        const meta = a.role + (a.id ? ' ' + a.id : '') + (a.model ? ' · ' + a.model : '');
        const o = { cls: 'dn state-' + a.st + (isSel ? ' sel' : onP ? ' path' : ''), x: c.x, y: n.y, w: c.w, h: n.h, hasIc: true, icSt: ic(a.h),
          title: a.name, tCls: 'dn-t1' + (isSel || onP ? ' hi' : ''), meta: '', marks: [mark(a.st, a.q)],
          sub: a.act, sCls: NEED[a.st] ? 'gs need' : 'gs', right: kidsOff ? '› ' + a.kids.length : [a.model, a.age].filter(Boolean).join(' · '),
          edge: !!NEED[a.st], bar: a.st === 'run' || a.st === 'rev' };
        if (isSel) Object.assign(o, { two: false, hasDetail: true, detail: [{ cls: 'dd', k: 'agent', t: [meta, a.age, a.tools ? a.tools + ' calls' : ''].filter(Boolean).join(' · '), r: '' }].concat(detailOf(a)), hasActs: true,
          acts: [{ cls: 'da solid', t: 'Peek', k: '⏎' }, { cls: 'da', t: 'Transcript', k: 't' }, { cls: a.st === 'wait' || a.st === 'blk' ? 'da' : 'da off', t: a.st === 'blk' ? 'Allow / deny' : 'Answer', k: 'a' }],
          meta: '', foot: '' });
        N(o);
      }));
      groups.forEach(g => {
        const ci = g.d; if (!vis(ci)) return;
        const c = cols[ci], onFn = k => !!(k.a && onPath.has(k.a));
        if (g.parent) { const pv = vis(ci - 1); link(pv ? cols[ci - 1].x + cols[ci - 1].w : 0, g.parent.y + g.parent.mid, c.x, g.kids, onFn); }
        else link(vis(0) ? cols[0].x + DW : 0, 27, c.x, g.kids, onFn);
      });
      for (let i = lo; i <= hi; i++) {
        const below = nodes.filter(n => n.x === cols[i].x && n.y + n.h > CH - 30).length;
        if (below) more.push({ t: '↓ ' + below + ' more below', st: { left: (cols[i].x + 10) + 'px', top: (CH - 26) + 'px' } });
      }
      const names = [O.title].concat(path.map(a => a.name));
      const nr = last - hi, total = span(0, last);
      return {
        left: lo > 0 ? '‹ ' + names.slice(0, lo).join(' › ') : '', leftN: lo > 0 ? lo + ' column' + (lo > 1 ? 's' : '') + ' left' : '',
        right: nr > 0 ? 'depth ' + (hi + 1) + (nr > 1 ? '–' + last : '') + ' · ' + nr + ' column' + (nr > 1 ? 's' : '') + ' right' : '', rightArrow: nr > 0 ? '›' : '',
        heads: cols.slice(lo, hi + 1).map(c => ({ t: c.t, n: String(c.n), st: { left: c.x + 'px' } })),
        lines, nodes, more, fadeL: lo > 0, fadeR: nr > 0,
        thumbSt: { left: ((lo === 0 ? 0 : span(0, lo - 1) + CG) / total * 100).toFixed(2) + '%', width: (span(lo, hi) / total * 100).toFixed(2) + '%' },
      };
    };
    const gD = deep('v1', 1), gD2 = deep('c1', 4);
    const tabsTerm = [
      { cls: 'tab on', hasIc: true, icSt: ic('codex'), label: '1', spin: true, close: true },
      { cls: 'tab', hasIc: true, icSt: ic('claude'), label: '2', dot: true },
    ];
    const tabsGraph = [
      { cls: 'tab on', swarm: true, hasIc: false, label: 'Swarm', chip: '4', close: false },
      { cls: 'tab', hasIc: true, icSt: ic('codex'), label: '1', spin: true },
      { cls: 'tab', hasIc: true, icSt: ic('claude'), label: '2', dot: true },
    ];
    // ---- V6 sidebar: the TUI dock (view/dock/row.rs, items.rs, tiles.rs, state.rs) ----
    const URG = { fail: 0, blk: 1, wait: 2, rev: 3, run: 4, bg: 5, q: 6, cxl: 8, done: 9 };
    const SVGG = { run: 'spin', rev: 'review', fail: 'x', done: 'check', q: 'ring', bg: 'orbit', cxl: 'minus', lost: 'tilde', stale: 'dashed', idle: 'ring' };
    const GL = st => SVGG[st] ? { cls: 'gl gsym g-' + SVGG[st] + ' h-' + st, t: '' } : { cls: 'gl h-' + st, t: st === 'blk' ? '!' : '?' };
    const TILE = (st, t) => { const b = mark(st); return { cls: b.cls + ' mini', t: t ?? b.glyph }; };
    // agent_glyphs: live agents most urgent first (up to n), then +N✓ for the finished
    const glyphRun = (list, n) => {
      const live = list.filter(a => a.st !== 'done').sort((x, y) => URG[x.st] - URG[y.st]), fin = list.length - live.length;
      return live.slice(0, n).map(a => GL(a.st)).concat(fin ? [{ cls: 'gl fin', t: '+' + fin + '✓' }] : []);
    };
    const threadTop = t => topOf(t.agents.flatMap(all));
    // dock_state: done > failed > blocked > working > needs you > waiting > idle
    const LABEL = { done: 'done', fail: 'failed', blk: 'blocked', run: 'working', wait: 'needs you', bg: 'waiting', idle: 'idle' };
    const dockState = o => {
      const ag = o.kids.flatMap(all), has = st => ag.some(a => a.st === st);
      const st = ag.length && ag.every(a => a.st === 'done') ? 'done' : has('fail') ? 'fail' : has('blk') || o.st === 'blk' ? 'blk'
        : has('run') || has('rev') || o.st === 'run' ? 'run' : has('wait') ? 'wait' : 'idle';
      const active = o.kids.filter(k => all(k).some(a => a.st !== 'done'));
      return { st, q: qsum(ag), active, done: o.kids.length - active.length, live: ag.filter(a => a.st !== 'done').length };
    };
    // Tree geometry (px from the row's left edge): header tile at H (18px), level-L leads at H + L*U (16px).
    // A parent's vertical runs through its lead's centre C(L); a child's elbow leaves it and stops 3px before the child.
    const H = 28, U = 18, RH = 20, CY = 10, LW = 1.5, MAXL = 2;  // two layers: the thread (1) and its agents (2); deeper agents fold into the layer-2 glyph run
    const C = L => L === 0 ? H + 8 : H + L * U + 7;
    const ln = (x, y, w, h, cls = 'tl-v') => ({ cls, st: { left: (x - LW / 2) + 'px', top: y + 'px', width: w + 'px', height: h + 'px' } });
    const treeLines = (L, last, cont, kids, glyph) => {
      const out = [];
      cont.forEach((c, j) => { if (c) out.push(ln(C(j), 0, LW, RH)); });                 // ancestors still going
      if (L >= 1) {
        const x = C(L - 1), end = H + L * U - 3;
        if (last) out.push(ln(x, 0, end - x + LW / 2, CY + LW / 2, 'tl-e'));               // └ rounded
        else { out.push(ln(x, 0, LW, RH)); out.push(ln(x, CY - LW / 2, end - x, LW, 'tl-h')); } // ├
      }
      if (kids) { const y = CY + (glyph ? 6 : 7); out.push(ln(C(L), y, LW, RH - y)); }      // down to its children
      return out;
    };
    const dock = (data, opt = {}) => {
      const out = [], tot = { wait: 0, blk: 0, fail: 0 };
      const SECT = [['Needs you', 'need', o => o.threads.some(t => t.agents.some(news)) || o.st === 'blk'], ['Done', 'done', o => o.st === 'done'], ['Working', 'working', o => o.st === 'run'], ['Idle', 'idle', o => o.st === 'q']];
      const used = new Set(), sections = SECT.map(([label, cls, test]) => { const m = data.filter(o => !used.has(o) && test(o)); m.forEach(o => used.add(o)); return [label, cls, m]; }).filter(x => x[2].length);
      sections.forEach(([label, cls, members]) => {
      out.push({ cls: 'dk sec', st: {}, lines: [], no: '', noCls: 'dk-no none', lead: [{ cls: 'dk-secl ' + cls, t: label }, { cls: 'dk-secn', t: String(members.length) }], hasIc: false, icSt: {}, name: '', nameCls: 'dk-c', right: [], bar: false });
      members.forEach(o => { const i = data.indexOf(o);
        const d = dockState(o), docked = !!o.focused, hov = opt.hov === i;
        o.threads.flatMap(t => t.agents.flatMap(all)).forEach(a => { if (a.st === 'wait') tot.wait += a.q ?? 0; if (a.st === 'blk') tot.blk++; if (a.st === 'fail') tot.fail++; });
        const ent = ' ent e-' + d.st + ' state-' + d.st + (docked ? ' docked' : '') + (hov ? ' hov' : '');
        const tile = d.st === 'run' ? TILE('run') : d.st === 'idle' ? TILE(o.st === 'q' ? 'idle' : 'q') : TILE(d.st, d.st === 'wait' ? '?' : undefined);
        const first = out.length;
        out.push({ cls: 'dk l1' + ent, st: {}, lines: [], no: hov ? '›' : String(o.tab ?? i + 1), noCls: hov ? 'dk-no hov' : 'dk-no', lead: [], hasIc: true, icSt: ic(o.h),  // the badge on the right carries the state; the harness icon leads
          name: o.title, nameCls: 'dk-n' + (docked ? ' b' : d.st === 'done' ? ' dim' : ''), right: (d.q && d.st !== 'wait' ? [{ cls: mark('wait', d.q).cls + ' mini', t: mark('wait', d.q).glyph }] : []).concat([mark(d.st, d.q)]), bar: false });
        const tiles = docked ? [] : d.active.map(k => { const top = topOf(all(k)); return TILE(top, top === 'wait' ? '?' : undefined); })
          .concat(!docked && d.done ? [TILE('done', d.done > 1 ? '✓' + d.done : '✓')] : []);
        out.push({ cls: 'dk l2' + ent, st: {}, lines: [], no: '', noCls: 'dk-no', lead: tiles, hasIc: false, icSt: {}, name: d.active.length + ' active · ' + d.live + ' agents', nameCls: 'dk-c', right: [], bar: d.st === 'run' || o.st === 'run' });
        if (docked) {
          // the docked entry's list: active threads, the agents that spawned others under them, then the done row
          const heads = d.active.map(a => ({ a })).concat(d.done ? [{ done: o.kids.filter(k => !d.active.includes(k)) }] : []);
          out[first].lines = [ln(C(0), 21, LW, 4)];
          out[first + 1].lines = [ln(C(0), 0, LW, 19)];
          out[first + 1].cls += ' sub';
          // one row component for every agent: own glyph, the name its spawner gave it, its children's states
          const agentRow = (a, L, last, cont) => {
            const shownKids = L < MAXL ? (a.kids || []).filter(SHOWN) : [];
            const below = L >= MAXL ? all(a).slice(1) : (a.kids || []).filter(k => !SHOWN(k));
            out.push({ cls: 'dk it spw' + ent + (opt.peek && a.peek ? ' pk' : ''), st: { paddingLeft: (H + L * U + 2) + 'px' }, lines: treeLines(L, last, cont, shownKids.length > 0, true),
              no: '', noCls: 'dk-no none', lead: [GL(a.st)], hasIc: false, icSt: {}, name: a.name, nameCls: 'dk-a', right: glyphRun(below, 6), bar: false });
            shownKids.forEach((k, n) => agentRow(k, L + 1, n === shownKids.length - 1, cont.concat([!last])));
          };
          heads.forEach((h, k) => {
            const last = k === heads.length - 1;
            if (h.done) { out.push({ cls: 'dk it' + ent, st: { paddingLeft: (H + U + 2) + 'px' }, lines: treeLines(1, last, [], false, true), no: '', noCls: 'dk-no none', lead: [GL('done')], hasIc: false, icSt: {}, name: h.done.length + ' finished · ' + h.done.map(a => a.name).join(', '), nameCls: 'dk-c', right: [], bar: false }); return; }
            agentRow(h.a, 1, last, []);
          });
        }
        out[first].cls += ' first';
        out[out.length - 1].cls += ' last';
      });
      });
      out.push({ cls: 'dk new', st: {}, lines: [], no: '+', noCls: 'dk-no plus', lead: [], hasIc: false, icSt: {}, name: 'New agent', nameCls: 'dk-c', right: [{ cls: 'dk-key', t: 'n' }], bar: false });
      out.tot = tot;
      return out;
    };
    // the sidebar lists the active space only, numbered by tab position within it
    const ACTIVE = 'Test';
    const order = data => data.filter(o => o.space === ACTIVE).map((o, k) => Object.assign({}, o, { tab: k + 1 }));
    const rowsA = dock(order(SW));
    const rowsD = dock(order(SW2));
    const rowsC = dock(order(SW), { peek: true });
    const rowsB = dock(order(SW), { hov: 1 });
    const wins = [
      { screen: 'A herdr-app window', pos: { left: '0px', top: '0px' }, clipSt: { width: '1920px', height: '1080px' }, winCls: 'win z12',
        label: 'A · herdr-app, default layout, with the panorama mod in the Agents slot (shown at the app\'s 1.2 zoom, like the screenshot)',
        note: 'Only the sidebar changes. Brand row, Spaces, tab bar, pane card and status bar are the shipped mods. Spaces gains one thing: how many items need you in that space, and a spinner when something works there. The Agents list becomes the swarm: orchestrators sorted into the shipped priority sections, their threads below, and agents only on the path to something that needs you. Codex runs the focused orchestrator; its workers are Claude, Cursor and Gemini.',
        spaces, tot: rowsA.tot, rows: rowsA, peek: false, thumb: false, tabs: tabsTerm, term: true, graph: false, g: gD },
      { screen: 'C Peek', pos: { left: '0px', top: '1240px' }, clipSt: { width: '1288px', height: '1800px' }, winCls: 'win z2',
        label: 'C · 2x, the same window cropped: clicking a subagent with no pane opens Peek',
        note: 'herdr-app peeks a pane-less sub-agent with terminal.peek. A Claude Task subagent has no terminal at all, so panorama opens its own popup overlay instead: status, error, last message, and a way back to the orchestrator that owns it.',
        spaces, tot: rowsC.tot, rows: rowsC, peek: true, thumb: false, tabs: tabsTerm, term: true, graph: false, g: gD },
      { screen: 'D Swarm tab', pos: { left: '0px', top: '3240px' }, clipSt: { width: '1920px', height: '1080px' }, winCls: 'win z12',
        label: 'D · Clicking a space opens its Swarm tab first: Swarm View 5 deep, restyled to herdr-app. One column per depth; the rest scrolls sideways',
        note: 'Columns: orchestrators of this space | threads of the selected orchestrator | depth 1 | depth 2 | … as deep as the selected thread goes. Columns keep their width and align left. The view shows the focused column, the one before it and, if it fits, the one after, then as many more as fit. Here the thread is selected, so orchestrators, threads and depths 1–2 show; depths 3–5 are off to the right (named above the columns, "› n" on cards whose children are off-screen, scrollbar below).',
        spaces: spacesD, tot: rowsD.tot, rows: rowsD, peek: false, thumb: true, tabs: tabsGraph, term: false, graph: true, g: gD },
      { screen: 'D2 Swarm tab scrolled', pos: { left: '0px', top: '4480px' }, clipSt: { width: '1920px', height: '1080px' }, winCls: 'win z12',
        label: 'D2 · The same tab after → → → → onto the checker at depth 4: the window scrolled right to depths 2–5',
        note: 'The selected path lights up (brighter borders and lines); the selected card opens with what it is doing, its question and actions (Peek, Transcript, Answer). Orchestrators, threads and depth 1 scrolled off to the left and are named on the breadcrumb line. Each column scrolls vertically on its own ("↓ n more below"). shift+wheel or a horizontal wheel scrolls without moving the selection; the next selection change scrolls back to it.',
        spaces: spacesD, tot: rowsD.tot, rows: rowsD, peek: false, thumb: true, tabs: tabsGraph, term: false, graph: true, g: gD2 },
    ];

    return { wins, spaces, rowsB, totB: rowsB.tot, lines, spinSt, edgeSt, barSt, icCodex: ic('codex'), icClaude: ic('claude') };
  }
}

window.PROPOSAL = new DesignFixture().renderVals();
