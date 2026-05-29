'use strict';

import { updateGenStrength } from './generator.js';

// ── Embedded wordlist (~500 common, memorable English words) ──
const WORDS = ('able about above across act add age ago agree ahead aim air all allow almost alone along already also always amaze ankle apple arch arm army art ask atlas aunt away baby back bake ball band barn base bath bear beat bell bench best bird bite black blade blank blaze blend blink blue blur bold bomb bone book born brave break brew brick bridge bright bring broad brown build burn burst calm camp card care carry cast catch cave chain chalk charm chart chase check cheer chef chew chief child chill chip chop claim clam clap clash clay clean clear clerk click cliff climb clock close cloud coach coal coat code cold color cook cool copy core cost couch count court cover craft crash cream crop cross crow crowd crush cup curl curl damp dare dark dawn deal dear deck deep deer deny depth desk dig dip dirt disk dive dock dome door dot down drag draw dream dress drift drink drive drop drum dusk dust duty each earn ease east edge else emit empty end enjoy equal even ever evil exact exam face fact fade fail fair fall fame farm fast fate fear feed feel fell felt fence fiber field fight file fill find fine fire fish fist flag flair flap flat flaw flew flip flock floor flow foam fold folk fond font food foot force ford fork form fort found four free fresh front frost full fund fury gale game gape garden gaze gear gift give glad glow glue goal gold good grab grade grain grand grant graph grasp grave gray great green grew grid grin grip grow guard guide gulf gust hand hang hard harp have hawk heal heap hear heat held help here hero high hill hint hold hole home hook hope horn host hour huge hull hunt idea inch iron jack jolt jump just keep kind king knot lake lamp land lane last latch laud lead leaf lean leap left lend lens less life lift like lime line link lion live load lock loft lone long look loop lost loud love luck lung made main make many mark mask mast maze meal mean meet melt mesh mile milk mill mine mint miss mist mode moon more most move much must nail name near neck need news next nice nigh night nine noble noise none noon north nose note noun oak ocean offer often oil old once only open order other ought ounce out over owl own pack page paid pail pain paint pair pale pan pane panel pant paper part pass past path pause paw pay peace peach peak pear peck peel peep peg pen pencil penny people per pet phase phone piano pick picnic pie piece pig pile pill pin pine pink pipe pit place plain plan plane plant plate play plea please plot plow plug plum plus pocket poem poet point poke pole pond pony pool poor pop porch port post pot pouch pound pour power pray press price pride prim prime print prize proof prop proud prove prune puff pull pulp pump punch pupil puppy pure purge push put quake queen quest quick quiet quill quilt quit race rack raft rage rail rain raise rake ramp range rank rare rash rat rate raw ray reach read real rear red reed reef reel rest rice rich rid ride ridge right ring ripe rise risk river road roar rob robe rock rod rode roll roof room root rope rose rot rough round route row rub rude rug rule run rung rush rust sack sad safe sage sail sale salt same sand sash sat save saw say scale scar scene scent school scoop scoot score scorn scout scow scrap screw sea seal seam search season seat second seed seek seem seen seesaw self sell send sense sent set seven shade shadow shake shall shame shape share shark sharp shave shed sheep sheer sheet shelf shell shift shine ship shirt shock shoe shook shop shore short shot should shout show shut shy sick side sift sigh sight sign silk sill silly silo silver sin since sing sink sir sit six size skate ski skill skin skip skirt sky slack slam slap slate sleep sleet slice slide slight slim slip slit slope slow small smart smash smell smile smoke smooth snail snake snap sneak snow soak soap soar sock soft soil sold solo solve some song soon sore sorry sort soul sound soup south space spade span spare spark speak spear speed spell spend spice spike spill spin spit split spoil spoke spoon sport spot spout spray spread spring spur square squash squat stack staff stage stain stair stake stale stall stamp stand star stare start state stay steak steal steam steel steep steer stem step stew stick stiff still sting stir stock stone stood stool stoop stop store storm story stout stove strap straw stray stream street stress stretch strict strike string strip strive strong stuck study stuff stump stung style such suck sugar suit sum summer sun sung sunny super sure surf swamp swan sway sweep sweet swell swept swift swim swing switch sword swore sworn table tail take tale talk tall tame tank tap tape task taste tax tea teach team tear tease teeth tell ten tend tent term test text thank that thaw them then there these they thick thief thin thing think third this those though thread threat three threw throat throne thumb thus tide tie tier tiger tight tile tilt time tin tint tiny tip tip tip tire tired tit title toad toast today toe told toll ton tone tongue took tool tooth top torch tore torn toss total touch tough tour tower town toy trace track trade trail train tramp trap tray treasure treat tree trend trial tribe trick tried trim trip troop truck true trunk trust truth try tub tube tuck tug tube tuck tune turn tusk tutor twelve twin twist two type ugly uncle under unit until up upon upper urge use used user usual utter vain vale value valve vase vast vault veer veil vein vent verb verse very vest vet vex vice view vine visit voice void volt vote vow wade waft wag wage wagon waist wait wake walk wall wand want war warm warn wart wash wasp waste watch water wave wax way we weak wealth wear weave web weed week weep weight well welt went wept west wet whale wharf what wheat wheel when where which while whip whirl whisk white who whole whom whose why wick wide wife wild will win wind wine wing wink wipe wire wise wish wit witch with wolf woman women won wood wool word wore work world worm worn worry worse worst worth would wound wrap wren wrist write wrong wrote yard yarn yawn year yeast yell yellow yelp yes yet yield yoke you young your youth zeal zebra zero zest zone').split(' ');

const CONSONANTS = 'b c d f g h j k l m n p r s t v w'.split(' ');
const VOWELS = 'a e i o u'.split(' ');
const PR_SYMBOLS = ['*', '@', '!', '#', '$'];

const LEET = { a: '4', e: '3', i: '1', o: '0', s: '5', t: '7' };

const MN_COLORS = ['Red', 'Blue', 'Gold', 'Silver', 'Jade', 'Crimson', 'Cobalt', 'Amber', 'Ivory', 'Onyx'];
const MN_ANIMALS = ['Tiger', 'Fox', 'Eagle', 'Wolf', 'Raven', 'Lynx', 'Cobra', 'Falcon', 'Panda', 'Bison'];
const MN_PLACES = ['Paris', 'Storm', 'River', 'Summit', 'Delta', 'Dune', 'Ridge', 'Harbor', 'Vale', 'Tundra'];
const MN_SYMBOLS = ['!', '@', '#', '$', '%', '&', '*'];

function randInt(n) { const a = new Uint32Array(1); crypto.getRandomValues(a); return a[0] % n; }
function pick(arr) { return arr[randInt(arr.length)]; }
function randDigits(n) { let s = ''; for (let i = 0; i < n; i++) s += randInt(10); return s; }

function calcEntropy(pwd, poolSize) {
  if (poolSize) return Math.round(pwd.length * Math.log2(poolSize));
  let pool = 0;
  if (/[a-z]/.test(pwd)) pool += 26;
  if (/[A-Z]/.test(pwd)) pool += 26;
  if (/[0-9]/.test(pwd)) pool += 10;
  if (/[^a-zA-Z0-9]/.test(pwd)) pool += 32;
  return pool < 2 ? 0 : Math.round(pwd.length * Math.log2(pool));
}

function entropyBadgeHTML(bits) {
  let cls, label;
  if (bits < 40) { cls = 'sp-entropy-weak'; label = `~${bits} bits · Weak`; }
  else if (bits < 60) { cls = 'sp-entropy-fair'; label = `~${bits} bits · Fair`; }
  else if (bits < 80) { cls = 'sp-entropy-strong'; label = `~${bits} bits · Strong`; }
  else { cls = 'sp-entropy-excel'; label = `~${bits} bits · Excellent`; }
  return `<span class="sp-entropy ${cls}">${label}</span>`;
}

// Issue 3 fix: shared DOM helpers replacing inline HTML strings in row builders

function _entropyMeta(bits) {
  if (bits < 40) return { cls: 'sp-entropy-weak',   label: `~${bits} bits · Weak` };
  if (bits < 60) return { cls: 'sp-entropy-fair',   label: `~${bits} bits · Fair` };
  if (bits < 80) return { cls: 'sp-entropy-strong', label: `~${bits} bits · Strong` };
  return           { cls: 'sp-entropy-excel',  label: `~${bits} bits · Excellent` };
}

function _makeActBtn(action, idx, pwd, svgEl) {
  const btn = document.createElement('button');
  btn.className = 'sp-act-btn';
  btn.dataset.action = action;
  if (idx !== null) btn.dataset.idx = String(idx);
  if (pwd !== null) btn.dataset.pwd = pwd;
  btn.title = action === 'copy' ? 'Copy' : action === 'use' ? 'Use this' : 'Regenerate';
  btn.appendChild(svgEl);
  return btn;
}

function _svgRefresh() {
  const s = document.createElementNS('http://www.w3.org/2000/svg','svg');
  s.setAttribute('viewBox','0 0 24 24');
  const p = document.createElementNS('http://www.w3.org/2000/svg','polyline');
  p.setAttribute('points','23 4 23 10 17 10');
  const pa = document.createElementNS('http://www.w3.org/2000/svg','path');
  pa.setAttribute('d','M20.49 15a9 9 0 1 1-2.12-9.36L23 10');
  s.appendChild(p); s.appendChild(pa); return s;
}

function _svgCopy() {
  const s = document.createElementNS('http://www.w3.org/2000/svg','svg');
  s.setAttribute('viewBox','0 0 24 24');
  const r = document.createElementNS('http://www.w3.org/2000/svg','rect');
  r.setAttribute('x','9'); r.setAttribute('y','9'); r.setAttribute('width','13'); r.setAttribute('height','13'); r.setAttribute('rx','2'); r.setAttribute('ry','2');
  const pa = document.createElementNS('http://www.w3.org/2000/svg','path');
  pa.setAttribute('d','M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1');
  s.appendChild(r); s.appendChild(pa); return s;
}

function _svgCheck() {
  const s = document.createElementNS('http://www.w3.org/2000/svg','svg');
  s.setAttribute('viewBox','0 0 24 24');
  const p = document.createElementNS('http://www.w3.org/2000/svg','polyline');
  p.setAttribute('points','20 6 9 17 4 12');
  s.appendChild(p); return s;
}

function _svgSpeak() {
  const s = document.createElementNS('http://www.w3.org/2000/svg','svg');
  s.setAttribute('viewBox','0 0 24 24'); s.setAttribute('width','11'); s.setAttribute('height','11');
  const pg = document.createElementNS('http://www.w3.org/2000/svg','polygon');
  pg.setAttribute('points','11 5 6 9 2 9 2 15 6 15 11 19 11 5');
  const p1 = document.createElementNS('http://www.w3.org/2000/svg','path');
  p1.setAttribute('d','M19.07 4.93a10 10 0 0 1 0 14.14');
  const p2 = document.createElementNS('http://www.w3.org/2000/svg','path');
  p2.setAttribute('d','M15.54 8.46a5 5 0 0 1 0 7.07');
  s.appendChild(pg); s.appendChild(p1); s.appendChild(p2); return s;
}

function usePassword(pwd) {
  const out = document.getElementById('gen-output');
  if (out) {
    out.textContent = pwd;
    const disp = document.querySelector('.gen-display');
    if (disp) { disp.classList.remove('sp-pulse'); void disp.offsetWidth; disp.classList.add('sp-pulse'); }
    updateGenStrength(pwd, null, null);
  }
  const addModal = document.getElementById('modal-add-entry');
  if (addModal && addModal.classList.contains('open')) {
    const addPwd = document.getElementById('add-password');
    if (addPwd) {
      addPwd.value = pwd;
      import('./ui.js').then(m => m.updateAddStrength()).catch(() => {});
    }
    return;
  }
  const editModal = document.getElementById('modal-edit-entry');
  if (editModal && editModal.classList.contains('open')) {
    const editPwd = document.getElementById('edit-password');
    if (editPwd) editPwd.value = pwd;
  }
}

const hasSpeech = (function () {
  try { return 'speechSynthesis' in window && !!window.SpeechSynthesisUtterance; }
  catch (_) { return false; }
})();

function speak(text) {
  if (!hasSpeech) return;
  // Require explicit per-session consent; _spSpeakEnabled resets on page reload
  if (!window._spSpeakEnabled) return;
  try {
    window.speechSynthesis.cancel();
    const utt = new window.SpeechSynthesisUtterance(text);
    utt.rate = 0.85;
    window.speechSynthesis.speak(utt);
  } catch (_) { }
}

function doCopy(btn, text) {
  navigator.clipboard.writeText(text).then(() => {
    btn.classList.add('copied');
    btn.textContent = '✓';
    const flash = document.createElement('span');
    flash.className = 'sp-copied-flash';
    flash.textContent = 'Copied!';
    btn.appendChild(flash);
    setTimeout(() => {
      btn.classList.remove('copied');
      btn.textContent = '📋';
      if (flash.parentNode) flash.parentNode.removeChild(flash);
    }, 2000);
  }).catch(() => { });
}

let ppSep = '-';
const ppData = [null, null, null, null, null];

function genPassphrase() {
  const count = parseInt(document.getElementById('pp-word-count').value);
  const cap = document.getElementById('pp-capitalize').checked;
  const nums = document.getElementById('pp-numbers').checked;
  const sym = document.getElementById('pp-symbol').checked;
  const symbols = ['!', '@', '#', '$', '%', '&'];

  const words = [];
  for (let i = 0; i < count; i++) {
    let w = pick(WORDS);
    if (cap) w = w.charAt(0).toUpperCase() + w.slice(1);
    words.push(w);
  }

  let pwd = words.join(ppSep);
  if (nums) pwd += randDigits(2);
  if (sym) pwd += pick(symbols);

  let bits = Math.round(count * Math.log2(WORDS.length));
  if (nums) bits += Math.round(Math.log2(90));
  if (sym) bits += Math.round(Math.log2(6));

  const pattern = `${cap ? 'Word' : 'word'}${ppSep === '' ? '' : ppSep}...×${count}${nums ? '+##' : ''}${sym ? '+!' : ''}`;

  return { pwd, bits, pattern };
}

function buildPpRow(idx, data) {
  const { pwd, bits, pattern } = data;
  const row = document.createElement('div');
  row.className = 'sp-row'; row.id = `pp-row-${idx}`; row.dataset.idx = String(idx); row.dataset.pwd = pwd;

  const body = document.createElement('div'); body.className = 'sp-row-body';
  const pwdEl = document.createElement('div'); pwdEl.className = 'sp-pwd-text'; pwdEl.textContent = pwd;
  const entEl = document.createElement('span');
  const { cls: eCls, label: eLabel } = _entropyMeta(bits);
  entEl.className = `sp-entropy ${eCls}`; entEl.textContent = eLabel;
  const patEl = document.createElement('div'); patEl.className = 'sp-pattern'; patEl.textContent = pattern;
  body.appendChild(pwdEl); body.appendChild(entEl); body.appendChild(patEl);

  const actions = document.createElement('div'); actions.className = 'sp-row-actions';
  actions.appendChild(_makeActBtn('regen-pp', idx, null, _svgRefresh()));
  actions.appendChild(_makeActBtn('copy', null, pwd, _svgCopy()));
  actions.appendChild(_makeActBtn('use', null, pwd, _svgCheck()));
  row.appendChild(body); row.appendChild(actions);
  return row;
}


function renderPpList() {
  const list = document.getElementById('pp-list');
  if (!list) return;
  list.innerHTML = '';
  for (let i = 0; i < 5; i++) {
    ppData[i] = genPassphrase();
    list.appendChild(buildPpRow(i, ppData[i]));
  }
}

function genSyllable() {
  const c1 = pick(CONSONANTS);
  const v = pick(VOWELS);
  const c2 = Math.random() > 0.5 ? pick(CONSONANTS) : '';
  return c1 + v + c2;
}

const DIGIT_WORDS = ['zero', 'one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine'];
const SYMBOL_WORDS = { '*': 'star', '@': 'at', '!': 'bang', '#': 'hash', '$': 'dollar', '%': 'percent', '&': 'and' };

function pronunGuide(pwd) {
  return pwd.split('').map(ch => {
    if (/[0-9]/.test(ch)) return DIGIT_WORDS[parseInt(ch)];
    if (SYMBOL_WORDS[ch]) return SYMBOL_WORDS[ch];
    return ch;
  }).join(' ').replace(/\s+/g, ' ');
}

function genPronounce() {
  const count = parseInt(document.getElementById('pr-syl-count').value);
  const digits = document.getElementById('pr-digits').checked;
  const sym = document.getElementById('pr-symbol').checked;
  const mixed = document.getElementById('pr-mixed').checked;
  const inject = document.getElementById('pr-inject').checked;

  const syllables = [];
  for (let i = 0; i < count; i++) syllables.push(genSyllable());

  if (mixed) {
    const idx = randInt(syllables.length);
    syllables[idx] = syllables[idx].charAt(0).toUpperCase() + syllables[idx].slice(1);
  }

  let chars = syllables.join('').split('');

  if (inject) {
    const extras = [];
    if (digits) {
      const numDigits = randInt(2) + 1;
      for (let i = 0; i < numDigits; i++) extras.push(String(randInt(10)));
    }
    if (sym) {
      const numSyms = randInt(2) + 1;
      for (let i = 0; i < numSyms; i++) extras.push(pick(PR_SYMBOLS));
    }
    extras.forEach(ch => {
      const pos = randInt(chars.length - 1) + 1;
      chars.splice(pos, 0, ch);
    });
  } else {
    if (digits) chars.push(...randDigits(randInt(1) + 1).split(''));
    if (sym) chars.push(pick(PR_SYMBOLS));
  }

  const pwd = chars.join('');
  const guide = pronunGuide(pwd);

  const bits = Math.round(count * Math.log2(17 * 5 * 9))
    + (digits ? Math.round(Math.log2(100)) : 0)
    + (sym ? Math.round(Math.log2(5)) : 0)
    + (inject ? 8 : 0);

  const pattern = `syl×${count}${inject ? ' (injected)' : digits || sym ? ' (appended)' : ''}`;
  return { pwd, guide, bits, pattern };
}

function buildPrRow(idx, data) {
  const { pwd, guide, bits, pattern } = data;
  const row = document.createElement('div');
  row.className = 'sp-row'; row.id = `pr-row-${idx}`; row.dataset.idx = String(idx); row.dataset.pwd = pwd;

  const body = document.createElement('div'); body.className = 'sp-row-body';
  const pwdEl = document.createElement('div'); pwdEl.className = 'sp-pwd-text'; pwdEl.textContent = pwd;
  const entEl = document.createElement('span');
  const { cls: eCls, label: eLabel } = _entropyMeta(bits);
  entEl.className = `sp-entropy ${eCls}`; entEl.textContent = eLabel;

  const pronDiv = document.createElement('div');
  pronDiv.className = 'sp-pronun'; pronDiv.style.cssText = 'display:flex;align-items:center;gap:4px;';
  if (hasSpeech) {
    const speakBtn = document.createElement('button');
    speakBtn.className = 'sp-speak-btn'; speakBtn.dataset.guide = guide; speakBtn.dataset.action = 'speak';
    speakBtn.title = '⚠ Reads password aloud — use only in private';
    speakBtn.appendChild(_svgSpeak());
    pronDiv.appendChild(speakBtn);
  }
  const guideSpan = document.createElement('span'); guideSpan.style.fontStyle = 'italic'; guideSpan.textContent = guide;
  pronDiv.appendChild(guideSpan);

  const patEl = document.createElement('div'); patEl.className = 'sp-pattern'; patEl.textContent = pattern;
  body.appendChild(pwdEl); body.appendChild(entEl); body.appendChild(pronDiv); body.appendChild(patEl);

  const actions = document.createElement('div'); actions.className = 'sp-row-actions';
  actions.appendChild(_makeActBtn('regen-pr', idx, null, _svgRefresh()));
  actions.appendChild(_makeActBtn('copy', null, pwd, _svgCopy()));
  actions.appendChild(_makeActBtn('use', null, pwd, _svgCheck()));
  row.appendChild(body); row.appendChild(actions);
  return row;
}

const prData = [null, null, null, null, null];

function renderPrList() {
  const list = document.getElementById('pr-list');
  if (!list) return;
  list.innerHTML = '';
  for (let i = 0; i < 5; i++) {
    prData[i] = genPronounce();
    list.appendChild(buildPrRow(i, prData[i]));
  }
}

function leetEncode(str) {
  return str.split('').map(c => LEET[c.toLowerCase()] || c).join('');
}

function mnPatterns(words) {
  const w = words.map(w => w.trim()).filter(Boolean);
  if (!w.length) return null;

  const useDigits = document.getElementById('mn-opt-digits').checked;
  const useSymbols = document.getElementById('mn-opt-symbols').checked;
  const useLeet = document.getElementById('mn-opt-leet').checked;
  const usePad = document.getElementById('mn-opt-pad').checked;
  const minLen = parseInt(document.getElementById('mn-min-len').value) || 12;

  const activeSep = document.querySelector('#mn-sep-row .sp-sep-pill.active');
  const sep = activeSep ? activeSep.dataset.sep : '.';

  const d = () => useDigits ? randDigits(randInt(2) + 1) : '';
  const s = () => useSymbols ? pick(MN_SYMBOLS) : '';
  const cap = str => str.charAt(0).toUpperCase() + str.slice(1).toLowerCase();
  const low = str => str.toLowerCase();
  const rev = str => str.split('').reverse().join('');
  const pad = str => usePad ? pick(['>>', '<<', '||', '**']) + str + pick(['>>', '<<', '||', '**']) : str;

  const injectAt = (str, ch) => {
    if (!ch) return str;
    const pos = randInt(str.length - 1) + 1;
    return str.slice(0, pos) + ch + str.slice(pos);
  };

  const enforce = pwd => {
    let out = pwd;
    while (out.length < minLen) out += randDigits(1) + (useSymbols ? pick(MN_SYMBOLS) : '');
    return out;
  };

  const results = [];

  {
    let parts = w.map(cap);
    if (useDigits) parts.push(randDigits(2));
    if (useSymbols) parts = [pick(MN_SYMBOLS), ...parts, pick(MN_SYMBOLS)];
    results.push({ name: 'Separated caps', pwd: enforce(parts.join(sep)) });
  }

  {
    let base = w.map((word, i) => i === 0 ? low(word) : cap(word)).join('');
    if (useDigits) base = injectAt(base, randDigits(2));
    if (useSymbols) base = injectAt(base, pick(MN_SYMBOLS));
    results.push({ name: 'CamelCase inject', pwd: enforce(base) });
  }

  {
    const transform = useLeet
      ? w.map(word => leetEncode(cap(word))).join(sep || '-')
      : w.map(rev).join(sep || '_');
    const name = useLeet ? 'Leet speak' : 'Word reversal';
    let pwd = transform;
    if (useDigits) pwd = injectAt(pwd, randDigits(2));
    if (useSymbols) pwd += pick(MN_SYMBOLS);
    results.push({ name, pwd: enforce(pwd) });
  }

  {
    const initials = w.map(word => word.charAt(0).toUpperCase()).join('');
    const full = w.map(cap).join('');
    let pwd = initials + (sep || '') + full;
    if (useDigits) pwd = injectAt(pwd, randDigits(2));
    if (useSymbols) pwd += pick(MN_SYMBOLS);
    results.push({ name: 'Initials + full', pwd: enforce(pwd) });
  }

  {
    const core = w.map(cap).join(sep || '');
    let pwd = pad(core);
    if (useDigits) pwd = injectAt(pwd, randDigits(2));
    if (useSymbols) pwd = injectAt(pwd, pick(MN_SYMBOLS));
    results.push({ name: 'Padded block', pwd: enforce(pwd) });
  }

  {
    const acronym = w.map(word => word.charAt(0).toUpperCase() + word.slice(-1).toLowerCase()).join(sep || '');
    let pwd = acronym;
    if (useDigits) pwd += randDigits(3);
    if (useSymbols) pwd = injectAt(pwd, pick(MN_SYMBOLS));
    results.push({ name: 'First+last acronym', pwd: enforce(pwd) });
  }

  return results;
}

function buildMnRow(idx, pwd, name) {
  while (pwd.length < 12) pwd += randInt(10);
  const bits = calcEntropy(pwd, 0);
  const row = document.createElement('div');
  row.className = 'sp-row'; row.id = `mn-row-${idx}`; row.dataset.idx = String(idx); row.dataset.pwd = pwd;

  const body = document.createElement('div'); body.className = 'sp-row-body';
  const pwdEl = document.createElement('div'); pwdEl.className = 'sp-pwd-text'; pwdEl.textContent = pwd;
  const entEl = document.createElement('span');
  const { cls: eCls, label: eLabel } = _entropyMeta(bits);
  entEl.className = `sp-entropy ${eCls}`; entEl.textContent = eLabel;
  const patEl = document.createElement('div'); patEl.className = 'sp-pattern'; patEl.textContent = name;
  body.appendChild(pwdEl); body.appendChild(entEl); body.appendChild(patEl);

  const actions = document.createElement('div'); actions.className = 'sp-row-actions';
  actions.appendChild(_makeActBtn('copy', null, pwd, _svgCopy()));
  actions.appendChild(_makeActBtn('use', null, pwd, _svgCheck()));
  row.appendChild(body); row.appendChild(actions);
  return row;
}

function getMnWords() {
  return [
    document.getElementById('mn-word1').value,
    document.getElementById('mn-word2').value,
    document.getElementById('mn-word3').value,
  ].map(w => w.trim()).filter(Boolean);
}

function renderMnList() {
  const list = document.getElementById('mn-list');
  if (!list) return;
  const words = getMnWords();
  if (!words.length) {
    list.innerHTML = '';
    const empty = document.createElement('div'); empty.className = 'sp-empty-state';
    empty.textContent = '✏️ Enter 1–3 hint words above to generate memorable passwords';
    list.appendChild(empty);
    return;
  }
  list.innerHTML = '';
  const patterns = mnPatterns(words);
  if (!patterns) return;
  patterns.forEach((p, i) => list.appendChild(buildMnRow(i, p.pwd, p.name)));
}

const MN_CHIP_DATA = [MN_COLORS, MN_ANIMALS, MN_PLACES];

function renderMnChips(inputIdx) {
  const chipRow = document.getElementById('mn-chips');
  if (!chipRow) return;
  const pool = MN_CHIP_DATA[inputIdx] || MN_COLORS;
  const shown = [...pool].sort(() => Math.random() - 0.5).slice(0, 4);
  chipRow.innerHTML = shown.map(w =>
    `<button class="sp-chip" data-input-idx="${inputIdx}" data-word="${escAttrFull(w)}" data-action="chip">${escHTML(w)}</button>`
  ).join('');
}

function mnSurpriseMe() {
  document.getElementById('mn-word1').value = pick(MN_COLORS);
  document.getElementById('mn-word2').value = pick(MN_ANIMALS);
  document.getElementById('mn-word3').value = pick(MN_PLACES);
  renderMnList();
}

let ppDebounce = null, prDebounce = null, mnDebounce = null;

function debouncePp() {
  clearTimeout(ppDebounce);
  ppDebounce = setTimeout(renderPpList, 300);
}

function debouncePr() {
  clearTimeout(prDebounce);
  prDebounce = setTimeout(renderPrList, 300);
}

function debounceMn() {
  clearTimeout(mnDebounce);
  mnDebounce = setTimeout(renderMnList, 300);
}

export function wireSmartPanel() {
  document.querySelectorAll('.sp-tab').forEach(tab => {
    tab.addEventListener('click', () => {
      document.querySelectorAll('.sp-tab').forEach(t => t.classList.remove('active'));
      document.querySelectorAll('.sp-tab-content').forEach(c => c.classList.remove('active'));
      tab.classList.add('active');
      const pane = document.getElementById('sptab-' + tab.dataset.sptab);
      if (pane) pane.classList.add('active');
    });
  });

  const ppSlider = document.getElementById('pp-word-count');
  const ppSliderLbl = document.getElementById('pp-word-count-lbl');
  if (ppSlider) {
    ppSlider.addEventListener('input', () => {
      ppSliderLbl.textContent = ppSlider.value + ' words';
      debouncePp();
    });
  }

  document.querySelectorAll('#pp-sep-row .sp-sep-pill').forEach(pill => {
    pill.addEventListener('click', () => {
      document.querySelectorAll('#pp-sep-row .sp-sep-pill').forEach(p => p.classList.remove('active'));
      pill.classList.add('active');
      ppSep = pill.dataset.sep;
      debouncePp();
    });
  });

  ['pp-capitalize', 'pp-numbers', 'pp-symbol'].forEach(id => {
    document.getElementById(id)?.addEventListener('change', debouncePp);
  });

  document.getElementById('pp-regen-all')?.addEventListener('click', renderPpList);

  const prSlider = document.getElementById('pr-syl-count');
  const prSliderLbl = document.getElementById('pr-syl-count-lbl');
  if (prSlider) {
    prSlider.addEventListener('input', () => {
      const v = parseInt(prSlider.value);
      prSliderLbl.textContent = `${v} syllables (~${v * 3} chars)`;
      debouncePr();
    });
  }

  ['pr-digits', 'pr-symbol', 'pr-mixed', 'pr-inject'].forEach(id => {
    document.getElementById(id)?.addEventListener('change', debouncePr);
  });

  document.getElementById('pr-regen-all')?.addEventListener('click', renderPrList);

  ['mn-word1', 'mn-word2', 'mn-word3'].forEach((id, idx) => {
    const el = document.getElementById(id);
    if (el) {
      el.addEventListener('input', debounceMn);
      el.addEventListener('focus', () => renderMnChips(idx));
    }
  });

  ['mn-opt-digits', 'mn-opt-symbols', 'mn-opt-leet', 'mn-opt-pad'].forEach(id => {
    document.getElementById(id)?.addEventListener('change', debounceMn);
  });

  document.querySelectorAll('#mn-sep-row .sp-sep-pill').forEach(pill => {
    pill.addEventListener('click', () => {
      document.querySelectorAll('#mn-sep-row .sp-sep-pill').forEach(p => p.classList.remove('active'));
      pill.classList.add('active');
      debounceMn();
    });
  });

  const mnLenSlider = document.getElementById('mn-min-len');
  const mnLenLbl = document.getElementById('mn-min-len-lbl');
  if (mnLenSlider) {
    mnLenSlider.addEventListener('input', () => {
      mnLenLbl.textContent = mnLenSlider.value;
      debounceMn();
    });
  }

  document.getElementById('mn-shuffle')?.addEventListener('click', renderMnList);
  document.getElementById('mn-surprise')?.addEventListener('click', mnSurpriseMe);
  function handleSpClick(e) {
    const regenPp  = e.target.closest('[data-action="regen-pp"]');
    const regenPr  = e.target.closest('[data-action="regen-pr"]');
    const copyBtn  = e.target.closest('[data-action="copy"]');
    const useBtn   = e.target.closest('[data-action="use"]');
    const speakBtn = e.target.closest('[data-action="speak"]');
    const chip     = e.target.closest('[data-action="chip"]');

    if (regenPp) {
      const idx = parseInt(regenPp.dataset.idx, 10);
      const rowEl = document.getElementById(`pp-row-${idx}`);
      if (!rowEl) return;

      ppData[idx] = genPassphrase();

      const newRow = buildPpRow(idx, ppData[idx]);

      rowEl.classList.add('fading');

      setTimeout(() => {
        rowEl.parentNode && rowEl.parentNode.replaceChild(newRow, rowEl);
      }, 150);
    }
    if (regenPr) {
      const idx = parseInt(regenPr.dataset.idx, 10);
      const rowEl = document.getElementById(`pr-row-${idx}`);
      if (!rowEl) return;

      prData[idx] = genPronounce();

      const newRow = buildPrRow(idx, prData[idx]);

      rowEl.classList.add('fading');

      setTimeout(() => {
        rowEl.parentNode && rowEl.parentNode.replaceChild(newRow, rowEl);
      }, 150);
    }
    if (copyBtn) { doCopy(copyBtn, copyBtn.dataset.pwd); }
    if (useBtn) {
      usePassword(useBtn.dataset.pwd);
      const row = useBtn.closest('.sp-row');
      if (row) {
        const existing = row.querySelector('.sp-sent-label');
        if (existing) existing.parentNode.removeChild(existing);
        const lbl = document.createElement('span');
        lbl.className = 'sp-sent-label';
        lbl.textContent = '→ Sent to generator';
        row.querySelector('.sp-row-body').appendChild(lbl);
        setTimeout(() => { if (lbl.parentNode) lbl.parentNode.removeChild(lbl); }, 2000);
      }
    }
    if (speakBtn) {
      // Require explicit opt-in with clear warning each session; disabled by default
      if (!window._spSpeakEnabled) {
        const ok = window.confirm(
          '⚠️ PRIVACY WARNING\n\n' +
          'This feature will read the password aloud using your device speakers.\n\n' +
          'Anyone nearby can hear it. Screen recording and voice assistants may capture it.\n\n' +
          'Only enable this in a completely private space.\n\n' +
          'Enable speech for this session?'
        );
        if (!ok) return;
        window._spSpeakEnabled = true; // only set on explicit confirmation
      }
      speak(speakBtn.dataset.guide);
    }
    
    if (chip) {
      const inputIdx = parseInt(chip.dataset.inputIdx, 10);
      const inputs = ['mn-word1', 'mn-word2', 'mn-word3'];
      const el = document.getElementById(inputs[inputIdx]);
      if (el) { el.value = chip.dataset.word; renderMnList(); }
    }
  }

  // Wire event delegation to list containers — single registration each
  const ppList = document.getElementById('pp-list');
  const prList = document.getElementById('pr-list');
  const mnList = document.getElementById('mn-list');
  const chipRow = document.getElementById('mn-chips');

  ppList?.addEventListener('click', handleSpClick);
  prList?.addEventListener('click', handleSpClick);
  mnList?.addEventListener('click', handleSpClick);
  chipRow?.addEventListener('click', handleSpClick);

  const genNavItem = document.querySelector('#sidebar .nav-item[data-page="generator"]');
  if (genNavItem) {
    genNavItem.addEventListener('click', () => {
      setTimeout(() => {
        const list = document.getElementById('pp-list');
        if (list && !list.innerHTML) renderPpList();
      }, 60);
    });
  }
}

export function initSmartPanel() {
  wireSmartPanel();
  if (document.getElementById('page-generator')?.classList.contains('active')) {
    renderPpList();
  }
  document.addEventListener('sp:navigateToGenerator', () => {
    setTimeout(renderPpList, 60);
  });
}
