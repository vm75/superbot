// Thin client: all state lives on the server. Live updates come over /api/events.

// Bot avatars: soft "Squish" robots. One body color, plus a dark visor with two eyes.
// Each shape is (color, face) => SVG. face is the visor function, or none() for the bare silhouette.
const DARK = "#141414";
const rect = (x, y, w, h, rx, f) => `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${rx}" fill="${f}"/>`;
const circ = (cx, cy, r, f) => `<circle cx="${cx}" cy="${cy}" r="${r}" fill="${f}"/>`;
const ell = (cx, cy, rx, ry, f, rot = 0) => `<ellipse cx="${cx}" cy="${cy}" rx="${rx}" ry="${ry}" fill="${f}" transform="rotate(${rot} ${cx} ${cy})"/>`;
const path = (d, f, round = 0) => `<path d="${d}" fill="${f}"${round ? ` stroke="${f}" stroke-width="${round}" stroke-linejoin="round"` : ""}/>`;
const tilt = (deg, s) => `<g transform="rotate(${deg} 24 24)">${s}</g>`;
const nub = (x, y, h, f) => rect(x - 1.3, y - h, 2.6, h + 2, 1.3, f) + circ(x, y - h, 3, f);
const visor = (k) => (x, y, w, h) => {
  const s = h / 12, g = w * 0.22, eye = (ex) => rect(ex - 1.5 * s, y - 2.9 * s, 3 * s, 5.8 * s, 1.5 * s, k);
  return rect(x - w / 2, y - h / 2, w, h, h / 2, DARK) + eye(x - g) + eye(x + g);
};
const none = () => "";
const GEAR = Array.from({ length: 97 }, (_, i) => {
  const a = (i / 96) * Math.PI * 2, r = 18.5 + 2.6 * Math.cos(8 * a);
  return `${i ? "L" : "M"}${(24 + r * Math.sin(a)).toFixed(2)} ${(24 - r * Math.cos(a)).toFixed(2)}`;
}).join("") + "Z";

// Order matters: the New Bot window shows them in this order.
const SHAPES = {
  waddle: (k, f) => nub(24, 9, 3, k) + ell(16.5, 41, 5.5, 3, k) + ell(31.5, 41, 5.5, 3, k) + circ(24, 24, 16.5, k) + f(24, 24, 23, 12),
  drop: (k, f) => path("M24 6c9 0 15 12 17 22 2 10-6 15-17 15S5 38 7 28C9 18 15 6 24 6z", k) + f(24, 27, 22, 12),
  cloud: (k, f) => nub(26, 12, 6, k) + circ(15, 27, 10, k) + circ(26, 22, 12, k) + circ(35, 29, 9, k) + rect(5, 27, 38, 15, 7.5, k) + f(24, 30, 22, 11),
  bean: (k, f) => nub(20, 9, 5, k) + ell(24, 26, 20, 14, k, -12) + f(24, 26, 23, 12),
  onigiri: (k, f) => path("M24 9L41 38H7Z", k, 12) + f(24, 30, 20, 11),
  peanut: (k, f) => nub(24, 11, 4, k) + circ(15, 26, 12, k) + circ(33, 26, 12, k) + rect(15, 17, 18, 18, 0, k) + f(24, 25, 26, 12),
  pebble: (k, f) => path("M9 16C13 8 30 4 38 11s7 22-3 29-26 5-29-7c-1-6 1-13 3-17z", k) + f(25, 25, 23, 12),
  square: (k, f) => tilt(12, nub(24, 9, 4, k) + rect(8, 9, 32, 32, 12, k) + f(24, 25, 22, 12)),
  jelly: (k, f) => tilt(18, ell(24, 25, 13.5, 19, k) + f(24, 22, 19, 11)),
  gem: (k, f) => path("M24 7L40.5 18.5L34.5 40H13.5L7.5 18.5Z", k, 9) + f(24, 26, 22, 12),
  gear: (k, f) => path(GEAR, k) + f(24, 24, 24, 12),
  heart: (k, f) => path("M24 42L7 25A9.5 9.5 0 0 1 24 13A9.5 9.5 0 0 1 41 25Z", k, 6) + f(24, 25, 22, 11),
};
// Shapes from before the robots, so existing Bots keep a similar look.
const OLD_SHAPES = { round: "waddle", star: "gear", flower: "gear", scallop: "gear", splat: "gear", blob: "pebble", egg: "drop", wide: "bean", triangle: "onigiri", pentagon: "gem", hexagon: "gem" };

// ring: draw a selection outline that follows the shape.
function avatar(color, shape = "waddle", size = 48, ring = false) {
  const draw = SHAPES[shape] ?? SHAPES[OLD_SHAPES[shape]] ?? SHAPES.waddle;
  const scaled = (k, s) => `<g transform="translate(24 24) scale(${k}) translate(-24 -24)">${s}</g>`;
  const body = draw(color, visor(color));
  return `<svg class="avatar" viewBox="0 0 48 48" width="${size}" height="${size}" style="stroke:none;width:${size}px;height:${size}px;overflow:visible">
    ${ring ? scaled(1.1, draw("#4a4a4a", none)) + scaled(1.02, draw("#1f1f1f", none)) + scaled(.86, body) : body}
  </svg>`;
}

const $ = (id) => document.getElementById(id);
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

// Small, safe Markdown: escape first, then add formatting.
function md(src) {
  const blocks = [];
  let s = esc(src).replace(/```[^\n]*\n([\s\S]*?)```/g, (_, code) => `\u0000${blocks.push(code) - 1}\u0000`);
  const inline = (t) => t
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/(^|[^*])\*([^*\n]+)\*/g, "$1<em>$2</em>")
    .replace(/\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>')
    .replace(/(^|[\s(])(https?:\/\/[^\s<)]+)/g, '$1<a href="$2" target="_blank" rel="noopener">$2</a>');
  return s.split(/\n{2,}/).map((para) => {
    const lines = para.split("\n");
    if (/^\u0000\d+\u0000$/.test(para.trim())) return para.trim();
    if (lines.every((l) => /^\s*[-*] /.test(l))) return `<ul>${lines.map((l) => `<li>${inline(l.replace(/^\s*[-*] /, ""))}</li>`).join("")}</ul>`;
    if (lines.every((l) => /^\s*\d+[.)] /.test(l))) return `<ol>${lines.map((l) => `<li>${inline(l.replace(/^\s*\d+[.)] /, ""))}</li>`).join("")}</ol>`;
    const h = para.match(/^#{1,4} (.*)$/);
    if (h && lines.length === 1) return `<p><strong>${inline(h[1])}</strong></p>`;
    return `<p>${lines.map(inline).join("<br>")}</p>`;
  }).join("").replace(/\u0000(\d+)\u0000/g, (_, i) => `<pre><code>${blocks[i]}</code></pre>`);
}

async function api(method, path, body) {
  const res = await fetch(path, {
    method,
    headers: body ? { "content-type": "application/json" } : {},
    body: body ? JSON.stringify(body) : undefined,
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
  return data;
}

// State
let me = { name: "" };
let harnesses = [];
let installedHarnesses = [];
let defaultHarness = "claude";
let bots = [];
const messages = {}; // botId -> array, loaded on first open
const drafts = {}; // botId -> streaming text
let activeId = localStorage.getItem("activeBot");
let filter = "";
let showHidden = false;

const bot = (id) => bots.find((b) => b.id === id);
const primary = () => bots.find((b) => b.primary);

function upsertBot(b) {
  const i = bots.findIndex((x) => x.id === b.id);
  if (i >= 0) bots[i] = { ...bots[i], ...b }; else bots.push(b);
}

// Sidebar
function renderSidebar() {
  const p = primary();
  $("primary-bot").innerHTML = p ? `<div class="avatar-wrap">${avatar(p.color, p.shape, 82)}
    <span class="badge"><svg viewBox="0 0 24 24"><path d="M12 2l3 7 7 .6-5.3 4.7 1.6 7.2L12 17.8 5.7 21.5l1.6-7.2L2 9.6 9 9z"/></svg></span>
    ${p.unread && p.id !== activeId ? '<span class="unread"></span>' : ""}</div>
    <span>${esc(p.name)}</span>` : "";
  $("primary-bot").classList.toggle("active", p?.id === activeId);

  const q = filter.toLowerCase();
  const list = bots.filter((b) => !b.primary && b.hidden === showHidden)
    .filter((b) => !q || b.name.toLowerCase().includes(q) || (b.preview || "").toLowerCase().includes(q));
  $("bot-list").innerHTML = list.map((b) => `<button class="bot-row ${b.id === activeId ? "active" : ""}" data-id="${b.id}">
      ${avatar(b.color, b.shape, 52)}
      <div class="meta"><div class="name">${esc(b.name)}</div>
        <div class="preview">${b.working ? `<em>${esc(b.status || "Working")}…</em>` : esc(b.preview || "")}</div></div>
      ${b.unread && b.id !== activeId ? '<span class="unread"></span>' : ""}
    </button>`).join("") || (showHidden ? '<div class="empty">No hidden Bots.</div>' : "");

  const hidden = bots.filter((b) => b.hidden).length;
  $("hidden-label").textContent = showHidden ? "Back to Bots" : "Hidden Bots";
  $("hidden-count").textContent = showHidden ? "" : hidden;
  $("me-name").textContent = me.name;
  $("me-avatar").textContent = (me.name || "?")[0].toUpperCase();
}

// Chat
const ACTIONS = `<div class="actions">
  <button title="Copy" data-act="copy"><svg viewBox="0 0 24 24"><rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"/></svg></button>
</div>`;

function when(ts) {
  const d = new Date(ts);
  const today = new Date(); today.setHours(0, 0, 0, 0);
  const day = new Date(d); day.setHours(0, 0, 0, 0);
  const diff = (today - day) / 86400000;
  const time = d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  if (diff === 0) return `Today ${time}`;
  if (diff === 1) return `Yesterday ${time}`;
  return `${d.toLocaleDateString([], { month: "short", day: "numeric" })} ${time}`;
}

function renderMessage(m, prev) {
  let out = !prev || m.at - prev.at > 30 * 60000 ? `<div class="timestamp">${when(m.at)}</div>` : "";
  if (m.role === "routine") return out + `<div class="timestamp routine">Routine · ${esc(m.text)}</div>`;
  if (m.role === "peer" || m.role === "reply") {
    const who = bot(m.fromBot)?.name || "another Bot";
    const label = m.role === "peer" ? `From ${esc(who)}` : `Reply from ${esc(who)}`;
    return out + `<div class="msg-row peer"><div class="bubble"><div class="from">${label}</div>${md(m.text)}</div></div>`;
  }
  if (m.role === "user") return out + `<div class="msg-row user"><div class="bubble">${esc(m.text).replace(/\n/g, "<br>")}</div></div>`;
  if (m.text) {
    out += `<div class="msg-row bot" data-id="${m.id}"><div class="bubble ${m.error ? "error" : ""}">${md(m.text)}</div>${ACTIONS}</div>`;
  }
  if (m.question) {
    const q = m.question;
    out += `<div class="msg-row bot"><div class="bubble question ${q.picked != null ? "answered" : ""}" data-id="${m.id}">
      <h4>${esc(q.title)}</h4>
      ${q.handoff && q.picked == null ? `<button class="open-screen">Open screen</button>` : ""}
      ${q.options.map((o, k) => `<button class="option ${q.picked === k ? "picked" : ""}" data-k="${k}">
        <span class="key">${"ABCDEF"[k]}</span>${esc(o)}</button>`).join("")}
    </div></div>`;
  }
  return out;
}

function renderChat(keepScroll) {
  const b = bot(activeId);
  if (!b) { $("messages").innerHTML = ""; return; }
  const el = $("messages");
  const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  $("bot-pill").innerHTML = `${avatar(b.color, b.shape, 30)}<span>${esc(b.name)}</span><span class="harness">${esc(b.harness)}</span>`;
  $("input").placeholder = `Message ${b.name}`;
  const list = messages[b.id] || [];
  let html = list.map((m, i) => renderMessage(m, list[i - 1])).join("");
  if (b.working) {
    const draft = drafts[b.id];
    if (draft) html += `<div class="msg-row bot"><div class="bubble">${md(draft)}</div></div>`;
    html += `<div class="msg-row bot"><div class="bubble typing"><span></span><span></span><span></span>
      <em class="status">${esc(b.status || "Working")}</em><button class="stop" id="stop">Stop</button></div></div>`;
  }
  el.innerHTML = html;
  if (!keepScroll || atBottom) scrollToBottom(); else updateScrollBtn();
}

function scrollToBottom() {
  const el = $("messages");
  el.scrollTop = el.scrollHeight;
  updateScrollBtn();
}

function updateScrollBtn() {
  const el = $("messages");
  $("scroll-down").classList.toggle("show", el.scrollHeight - el.scrollTop - el.clientHeight > 80);
}

async function open(id) {
  if (!bot(id)) id = primary()?.id;
  if (!id) return;
  activeId = id;
  localStorage.setItem("activeBot", id);
  if (!messages[id]) messages[id] = await api("GET", `/api/bots/${id}/messages`);
  markRead();
  render();
  if (details.open) openDetails(); // load the new Bot's routines
  if (!$("screen").hidden) loadScreen();
}

function markRead() {
  const b = bot(activeId);
  if (b?.unread && document.hasFocus()) {
    b.unread = false;
    api("POST", `/api/bots/${b.id}/read`).catch(() => {});
  }
}

async function sendText(text, answer) {
  const b = bot(activeId);
  if (!b || !text) return;
  try {
    await api("POST", `/api/bots/${b.id}/messages`, { text, answer });
  } catch (err) {
    alert(err.message);
  }
}

function render(keepScroll) {
  renderSidebar();
  renderChat(keepScroll);
  renderModelBar();
  if (details.open && document.activeElement?.id !== "d-name") renderDetails();
}

// Model and effort selector. Never reuse one harness's catalog for another.
const modelLists = {}; // harness -> Promise<models>
let modelBarKey = "";
const cap = (s) => s.charAt(0).toUpperCase() + s.slice(1);

async function renderModelBar() {
  const first = bot(activeId);
  if (!first) { $("model-bar").hidden = true; return; }
  const harness = first.harness;
  modelLists[harness] ??= api("GET", "/api/harnesses/" + encodeURIComponent(harness) + "/models")
    .catch((err) => { delete modelLists[harness]; throw err; });
  let list;
  try {
    list = await modelLists[harness];
  } catch (err) {
    const b = bot(activeId);
    if (!b || b.id !== first.id || b.harness !== harness) return;
    $("model-select").innerHTML = '<option value="">Model discovery failed</option>';
    $("model-select").disabled = true;
    $("model-select").title = err.message + " (check the CLI and authentication on the server)";
    $("effort-select").hidden = true;
    $("model-bar").hidden = false;
    modelBarKey = "";
    return;
  }
  const b = bot(activeId);
  if (!b || b.id !== first.id || b.harness !== harness) return;
  const key = [b.id, b.harness, b.model, b.effort, JSON.stringify(list)].join("|");
  if (key === modelBarKey) return;
  modelBarKey = key;
  const all = [...new Set(list.flatMap((m) => m.efforts || []))];
  const models = [{ id: "", label: "Default model", efforts: all }, ...list];
  const isCurrent = (m) => m.id === (b.model || "") || (b.model && (m.aliases || []).includes(b.model));
  const unknown = !!b.model && !list.some(isCurrent);
  const current = models.find(isCurrent) || models[0];
  const select = $("model-select");
  select.innerHTML = (unknown
    ? '<option value="!unavailable" disabled selected>Saved model unavailable — choose a model</option>'
    : '') + models.map((m) => '<option value="' + esc(m.id) + '"' +
      (!unknown && m === current ? ' selected' : '') + '>' + esc(m.label) + '</option>').join("");
  select.disabled = false;
  select.title = unknown ? "The saved model (" + b.model + ") is not in the " + harness + " catalog. Choose a model to replace it." : "Model";
  const efforts = current.efforts || [];
  $("effort-select").innerHTML = ["", ...efforts].map((e) => '<option value="' + esc(e) + '"' +
    (e === (b.effort || "") ? ' selected' : '') + '>' + (e ? cap(e) : "Default effort") + '</option>').join("");
  $("effort-select").hidden = !efforts.length;
  $("model-bar").hidden = false;
}

async function saveModel(patch) {
  const b = bot(activeId);
  if (!b) return;
  try { upsertBot(await api("PATCH", "/api/bots/" + encodeURIComponent(b.id), patch)); }
  catch (err) { alert(err.message); }
  modelBarKey = "";
  renderModelBar();
}

$("model-select").addEventListener("change", async (e) => {
  const b = bot(activeId);
  if (!b || e.target.value === "!unavailable") return;
  const list = await modelLists[b.harness].catch(() => []);
  const m = list.find((x) => x.id === e.target.value);
  const effort = (!m || (b.effort && !(m.efforts || []).includes(b.effort))) ? null : b.effort;
  saveModel({ model: e.target.value || null, effort });
});
$("effort-select").addEventListener("change", (e) => saveModel({ effort: e.target.value || null }));

// Live screen: the Bot's own desktop, streamed by voxl. View only until you take over.
let screenTimer = null;
function setTakeover(on) {
  $("screen-shield").hidden = on;
  $("screen-take").classList.toggle("on", on);
  $("screen-take").textContent = on ? "Give back" : "Take over";
}
async function loadScreen() {
  const id = activeId;
  $("screen-title").textContent = `${bot(id)?.name || "Bot"}'s screen`;
  const frame = $("screen-frame");
  try {
    const { url } = await api("GET", `/api/bots/${id}/screen`); // also keeps it running while you watch
    if (id !== activeId || $("screen").hidden) return;
    if (frame.dataset.url !== url) { frame.dataset.url = url; frame.src = url; setTakeover(false); }
    $("screen-msg").hidden = true;
  } catch (err) {
    frame.removeAttribute("src"); delete frame.dataset.url;
    $("screen-msg").textContent = err.message;
    $("screen-msg").hidden = false;
  }
}
function openScreen() {
  if (details.open) closeDetails(); // one right-hand panel at a time
  $("screen").hidden = false;
  loadScreen();
  clearInterval(screenTimer);
  screenTimer = setInterval(loadScreen, 60000);
}
function closeScreen() {
  $("screen").hidden = true;
  clearInterval(screenTimer);
  $("screen-frame").removeAttribute("src");
  delete $("screen-frame").dataset.url;
}
$("screen-btn").addEventListener("click", () => ($("screen").hidden ? openScreen() : closeScreen()));
$("screen-close").addEventListener("click", closeScreen);
$("screen-take").addEventListener("click", () => {
  const on = !$("screen-shield").hidden;
  setTakeover(on);
  if (on) $("screen-frame").focus();
});

// Live updates
function connect() {
  const es = new EventSource("/api/events");
  es.onmessage = (ev) => {
    const e = JSON.parse(ev.data);
    if (e.type === "resync") return void load(true);
    if (e.type === "message") {
      const list = messages[e.message.botId];
      if (list && !list.some((m) => m.id === e.message.id)) list.push(e.message);
      if (e.message.role === "bot") delete drafts[e.message.botId];
      if (e.bot) upsertBot(e.bot);
      if (e.message.botId === activeId) markRead();
    } else if (e.type === "bot") {
      upsertBot(e.bot);
    } else if (e.type === "botDeleted") {
      bots = bots.filter((b) => b.id !== e.botId);
      if (activeId === e.botId) return void open(primary()?.id);
    } else if (e.type === "draft") {
      drafts[e.botId] = e.text;
    } else if (e.type === "status") {
      const b = bot(e.botId);
      if (b) { b.working = e.working; b.status = e.status; }
      if (!e.working) delete drafts[e.botId];
    }
    render(true);
  };
  // Resync after a reconnect, in case events were missed.
  let opened = false;
  es.onopen = () => { if (opened) load(true); opened = true; };
}

async function load(refresh) {
  const state = await api("GET", "/api/state");
  me = state.me;
  harnesses = state.harnesses;
  installedHarnesses = state.installedHarnesses || state.harnesses;
  defaultHarness = state.defaultHarness;
  bots = state.bots;
  if (refresh) for (const id of Object.keys(messages)) messages[id] = await api("GET", `/api/bots/${id}/messages`);
  await open(activeId);
}

// Events
$("bot-list").addEventListener("click", (e) => {
  const row = e.target.closest(".bot-row");
  if (row) open(row.dataset.id);
});
$("primary-bot").addEventListener("click", () => open(primary()?.id));

$("messages").addEventListener("click", (e) => {
  if (e.target.closest("#stop")) return void api("POST", `/api/bots/${activeId}/stop`).catch(() => {});
  const copy = e.target.closest('[data-act="copy"]');
  if (copy) {
    const m = messages[activeId]?.find((x) => x.id === +copy.closest(".msg-row").dataset.id);
    if (m) navigator.clipboard.writeText(m.text);
    return;
  }
  if (e.target.closest(".open-screen")) return void openScreen();
  const opt = e.target.closest(".option");
  if (!opt) return;
  const card = opt.closest(".question");
  const m = messages[activeId]?.find((x) => x.id === +card.dataset.id);
  if (!m || m.question.picked != null) return;
  m.question.picked = +opt.dataset.k;
  render(true);
  sendText(m.question.options[m.question.picked], { messageId: m.id, option: m.question.picked });
});

$("composer").addEventListener("submit", (e) => {
  e.preventDefault();
  const text = $("input").value.trim();
  if (!text) return;
  $("input").value = "";
  sendText(text);
});

// New Bot window
const COLORS = ["#f5f5f5", "#9a6b3f", "#e5484d", "#e8702e", "#e9a23b", "#4cc06a", "#4fb5a5", "#3b82f6", "#8b5cf6", "#e64a9c", "#7a7a7a"];
const HARNESS_NAMES = { claude: "Claude Code", codex: "Codex", pi: "Pi" };
const nb = { color: COLORS[0], shape: "waddle", harness: "claude" };
const pickOne = (a) => a[Math.floor(Math.random() * a.length)];

function renderNewBot() {
  $("nb-preview").innerHTML = avatar(nb.color, nb.shape, 150);
  $("nb-shapes").innerHTML = Object.keys(SHAPES).map((s) =>
    `<button type="button" class="nb-shape" data-shape="${s}" title="${s}">${avatar(nb.color, s, 60, s === nb.shape)}</button>`).join("");
  $("nb-colors").innerHTML = COLORS.map((c) =>
    `<button type="button" class="nb-color ${c === nb.color ? "on" : ""}" data-color="${c}" style="background:${c}" title="${c}"></button>`).join("");
  const list = [...new Set([...(harnesses.length ? harnesses : Object.keys(HARNESS_NAMES)), nb.harness])];
  $("nb-harness").innerHTML = list.map((h) =>
    `<option value="${h}" ${h === nb.harness ? "selected" : ""}>${HARNESS_NAMES[h] || h}${installedHarnesses.includes(h) ? "" : " (not installed)"}</option>`).join("");
  $("nb-create").disabled = !$("nb-name").value.trim();
}

function openNewBot() {
  Object.assign(nb, { color: pickOne(COLORS), shape: pickOne(Object.keys(SHAPES)), harness: defaultHarness });
  $("nb-name").value = "";
  $("nb-error").hidden = true;
  renderNewBot();
  $("nb").hidden = false;
  $("nb-name").focus();
}
const closeNewBot = () => { $("nb").hidden = true; };

$("new-btn").addEventListener("click", openNewBot);
$("nb-close").addEventListener("click", closeNewBot);
$("nb-cancel").addEventListener("click", closeNewBot);
$("nb").addEventListener("mousedown", (e) => { if (e.target === $("nb")) closeNewBot(); });
document.addEventListener("keydown", (e) => { if (e.key === "Escape" && !$("nb").hidden) closeNewBot(); });
$("nb-name").addEventListener("input", () => { $("nb-create").disabled = !$("nb-name").value.trim(); });
$("nb-form").addEventListener("click", (e) => {
  const t = e.target.closest("[data-shape], [data-color]");
  if (!t) return;
  if (t.dataset.shape) nb.shape = t.dataset.shape;
  if (t.dataset.color) nb.color = t.dataset.color;
  renderNewBot();
});
$("nb-harness").addEventListener("change", (e) => { nb.harness = e.target.value; });
$("nb-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const name = $("nb-name").value.trim();
  if (!name) return;
  $("nb-create").disabled = true;
  try {
    const b = await api("POST", "/api/bots", { name, color: nb.color, shape: nb.shape, harness: nb.harness });
    upsertBot(b);
    closeNewBot();
    open(b.id);
  } catch (err) {
    $("nb-error").textContent = err.message;
    $("nb-error").hidden = false;
    $("nb-create").disabled = false;
  }
});

// Bot details: a right-hand panel, opened by clicking the name at the top of the chat.
const ICONS = {
  clock: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/></svg>',
  job: '<svg viewBox="0 0 24 24"><rect x="4" y="7" width="16" height="12" rx="2"/><path d="M9 7V5.5A1.5 1.5 0 0 1 10.5 4h3A1.5 1.5 0 0 1 15 5.5V7"/></svg>',
  chevron: '<svg viewBox="0 0 24 24"><path d="M9 6l6 6-6 6"/></svg>',
  trash: '<svg viewBox="0 0 24 24"><path d="M5 7h14M10 7V5h4v2M7 7l1 12h8l1-12"/></svg>',
};
const details = { open: false, tab: "activity", routines: [] };

async function openDetails() {
  details.open = true;
  if (!$("screen").hidden) closeScreen(); // one right-hand panel at a time
  details.routines = [];
  renderDetails();
  try { details.routines = await api("GET", `/api/bots/${activeId}/routines`); } catch {}
  renderDetails();
}
function closeDetails() { details.open = false; renderDetails(); }

function renderDetails() {
  const el = $("details");
  const b = bot(activeId);
  el.hidden = !details.open || !b;
  if (el.hidden) return;
  const toggle = (on, attrs) => `<button type="button" class="switch ${on ? "on" : ""}" ${attrs}><span></span></button>`;
  const routines = details.routines.length
    ? details.routines.map((r) => `<div class="d-item"><div class="d-text"><div>${esc(r.name)}</div><div class="d-sub">${esc(r.cron)}</div></div>
        ${toggle(r.enabled, `data-routine="${r.id}" title="${r.enabled ? "Pause" : "Resume"}"`)}</div>`).join("")
    : `<div class="d-item d-empty">No routines yet.</div>`;
  const activity = `
    <div class="d-section"><span>Suggested</span></div>
    <div class="d-card">
      <button class="d-item d-link" data-prefill="Set up a routine to ">${ICONS.clock}<span>Create a routine</span>${ICONS.chevron}</button>
      <button class="d-item d-link" data-prefill="Your job: ">${ICONS.job}<span>Give it a job</span>${ICONS.chevron}</button>
    </div>
    <div class="d-section"><span>Routines</span></div>
    <div class="d-card">${routines}</div>
    <div class="d-section"><span>Computer</span></div>
    <div class="d-card"><div class="d-item d-computer">
      <div class="d-text"><div>Bot's computer</div><div class="d-sub">${b.working ? esc(b.status || "Working") : "Asleep"}</div></div>
      <div class="d-screen ${b.working ? "awake" : ""}">${avatar(b.color, b.shape, 40)}${b.working ? "" : "<i>z</i>"}</div>
    </div></div>`;
  const list = [...new Set([...(harnesses.length ? harnesses : Object.keys(HARNESS_NAMES)), b.harness])];
  const settings = `
    <div class="d-section"><span>Bot</span></div>
    <div class="d-card">
      <label class="d-item"><span class="d-text">Runs on</span>
        <select id="d-harness">${list.map((h) => `<option value="${h}" ${h === b.harness ? "selected" : ""}>${HARNESS_NAMES[h] || h}${installedHarnesses.includes(h) ? "" : " (not installed)"}</option>`).join("")}</select></label>
      <div class="d-item"><span class="d-text">Hide from sidebar</span>${toggle(b.hidden, 'id="d-hide"')}</div>
    </div>
    ${b.primary ? "" : `<div class="d-card"><button class="d-item d-link d-danger" id="d-delete">${ICONS.trash}<span>Delete Bot</span></button></div>`}`;
  el.innerHTML = `
    <button class="d-close" id="d-close" title="Close"><svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg></button>
    <div class="d-hero">${avatar(b.color, b.shape, 120)}
      <input class="d-name" id="d-name" value="${esc(b.name)}" maxlength="60" title="Rename" spellcheck="false" /></div>
    <div class="d-tabs">
      <button data-tab="activity" class="${details.tab === "activity" ? "on" : ""}">Activity</button>
      <button data-tab="settings" class="${details.tab === "settings" ? "on" : ""}">Settings</button>
    </div>
    ${details.tab === "activity" ? activity : settings}`;
}

async function patchBot(body) {
  try { upsertBot(await api("PATCH", `/api/bots/${activeId}`, body)); render(true); renderDetails(); }
  catch (err) { alert(err.message); }
}

$("bot-pill").addEventListener("click", () => (details.open ? closeDetails() : openDetails()));
$("details").addEventListener("click", async (e) => {
  const t = e.target.closest("button");
  if (!t) return;
  if (t.id === "d-close") return closeDetails();
  if (t.dataset.tab) { details.tab = t.dataset.tab; return renderDetails(); }
  if (t.dataset.prefill) { $("input").value = t.dataset.prefill; $("input").focus(); return; }
  if (t.id === "d-hide") return patchBot({ hidden: !bot(activeId).hidden });
  if (t.dataset.routine) {
    const r = details.routines.find((x) => x.id === t.dataset.routine);
    try { Object.assign(r, await api("PATCH", `/api/routines/${r.id}`, { enabled: !r.enabled })); } catch (err) { alert(err.message); }
    return renderDetails();
  }
  if (t.id === "d-delete") {
    const b = bot(activeId);
    if (confirm(`Delete ${b.name}? This erases its chat, memory, and routines. Files on the computer stay.`)) {
      try { await api("DELETE", `/api/bots/${b.id}`); closeDetails(); } catch (err) { alert(err.message); }
    }
  }
});
$("details").addEventListener("change", (e) => { if (e.target.id === "d-harness") patchBot({ harness: e.target.value }); });
$("details").addEventListener("keydown", (e) => { if (e.target.id === "d-name" && e.key === "Enter") e.target.blur(); });
$("details").addEventListener("focusout", (e) => {
  if (e.target.id !== "d-name") return;
  const name = e.target.value.trim();
  if (name && name !== bot(activeId)?.name) patchBot({ name });
  else e.target.value = bot(activeId)?.name ?? "";
});
document.addEventListener("keydown", (e) => { if (e.key === "Escape" && details.open && $("nb").hidden) closeDetails(); });

$("hidden-btn").addEventListener("click", () => { showHidden = !showHidden; renderSidebar(); });
$("search-btn").addEventListener("click", () => {
  const s = $("search");
  s.hidden = !s.hidden;
  if (!s.hidden) s.focus(); else { filter = ""; s.value = ""; renderSidebar(); }
});
$("search").addEventListener("input", (e) => { filter = e.target.value; renderSidebar(); });
$("messages").addEventListener("scroll", updateScrollBtn);
$("scroll-down").addEventListener("click", () => $("messages").scrollTo({ top: $("messages").scrollHeight, behavior: "smooth" }));
window.addEventListener("focus", () => { markRead(); renderSidebar(); });

load().then(connect).catch((err) => { $("messages").innerHTML = `<div class="timestamp">Can't reach the server: ${esc(err.message)}</div>`; });


// Administrator settings. Custom harness definitions are stored in SQLite, not source files.
let adapterItems = [];
let editingAdapterId = null;
const adapterFields = ["args", "model-args", "effort-args", "start-args", "resume-args", "model-command"];
const lines = (s) => s.split(/\r?\n/).map((v) => v.trim()).filter(Boolean);
const showSettingsError = (text) => {
  $("settings-error").textContent = text || "";
  $("settings-error").hidden = !text;
};
const choices = (value, items) => {
  const all = [...items];
  if (value && !all.some((i) => i.id === value)) all.push({ id: value, name: value + " (unavailable)" });
  return all.map((i) => '<option value="' + esc(i.id) + '"' +
    (i.id === value ? ' selected' : '') + '>' + esc(i.name) + '</option>').join("");
};

function renderSettings() {
  const enabled = adapterItems.filter((a) => a.enabled && a.available);
  const p = primary();
  $("setting-default").innerHTML = choices(defaultHarness, enabled);
  $("setting-primary").innerHTML = choices(p?.id,
    bots.map((b) => ({ id: b.id, name: b.name })));
  $("setting-primary-harness").innerHTML = choices(p?.harness, enabled);
  $("adapter-list").innerHTML = adapterItems.map((a) => {
    const state = a.enabled ? (a.available ? "Installed" : "Executable missing") : "Disabled";
    return '<div class="adapter-row"><div class="adapter-info"><strong>' + esc(a.name) +
      '</strong><small>' + esc(a.id) + ' · ' + esc(a.program) + ' · ' + state +
      (a.kind === "builtin" ? " · Built in" : " · Custom") + '</small></div>' +
      '<button type="button" data-adapter-act="check" data-adapter-id="' + esc(a.id) + '">Check</button>' +
      (a.kind === "custom" ?
        '<button type="button" data-adapter-act="edit" data-adapter-id="' + esc(a.id) + '">Edit</button>' +
        '<button type="button" data-adapter-act="clone" data-adapter-id="' + esc(a.id) + '">Clone</button>' +
        '<button type="button" class="danger" data-adapter-act="delete" data-adapter-id="' + esc(a.id) + '">Delete</button>' :
        '') + '</div>';
  }).join("");
}

async function loadAdapters() {
  adapterItems = await api("GET", "/api/adapters");
  renderSettings();
}
async function openSettings() {
  $("settings-modal").hidden = false;
  showSettingsError("");
  try { await loadAdapters(); } catch (e) { showSettingsError(e.message); }
}
const closeSettings = () => {
  $("settings-modal").hidden = true;
  $("adapter-editor").hidden = true;
};
$("settings-btn").addEventListener("click", openSettings);
$("settings-close").addEventListener("click", closeSettings);
$("settings-modal").addEventListener("mousedown", (e) => {
  if (e.target === $("settings-modal")) closeSettings();
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !$("settings-modal").hidden) closeSettings();
});

$("setting-default").addEventListener("change", async (e) => {
  try {
    const value = e.target.value;
    await api("PATCH", "/api/settings", { defaultHarness: value });
    defaultHarness = value;
    renderSettings();
  } catch (err) { showSettingsError(err.message); renderSettings(); }
});
$("setting-primary").addEventListener("change", async (e) => {
  try {
    await api("PATCH", "/api/settings", { primaryBotId: e.target.value });
    await load();
    renderSettings();
  } catch (err) { showSettingsError(err.message); renderSettings(); }
});
$("setting-primary-harness").addEventListener("change", async (e) => {
  const p = primary();
  if (!p) return;
  try {
    const previous = p.harness;
    const changed = await api("PATCH", "/api/bots/" + encodeURIComponent(p.id), { harness: e.target.value });
    upsertBot(changed);
    delete modelLists[previous];
    delete modelLists[changed.harness];
    modelBarKey = "";
    render();
    renderSettings();
  } catch (err) { showSettingsError(err.message); renderSettings(); }
});

function openAdapterEditor(spec = {}, existing = false) {
  editingAdapterId = existing ? spec.id : null;
  $("adapter-editor").hidden = false;
  $("adapter-editor-title").textContent = editingAdapterId ? "Edit " + spec.name : "Create adapter";
  $("adapter-id").disabled = !!editingAdapterId;
  $("adapter-name").value = spec.name || "";
  $("adapter-id").value = spec.id || "";
  $("adapter-program").value = spec.program || "";
  $("adapter-output").value = spec.output || "text";
  $("adapter-session").value = spec.session || "none";
  $("adapter-model-format").value = spec.model_format || "lines";
  $("adapter-stdin").checked = !!spec.stdin;
  $("adapter-enabled").checked = spec.enabled !== false;
  for (const name of adapterFields) {
    const field = name.replaceAll("-", "_");
    $("adapter-" + name).value = (spec[field] || []).join("\n");
  }
  $("adapter-models").value = (spec.model_list || []).map((m) =>
    m.id + (m.label && m.label !== m.id || m.efforts?.length
      ? " | " + (m.label || m.id) + " | " + (m.efforts || []).join(",")
      : "")).join("\n");
  $("adapter-rules").value = spec.event_rules?.length ? JSON.stringify(spec.event_rules, null, 2) : "";
  $("adapter-editor").scrollIntoView({ behavior: "smooth", block: "start" });
  $("adapter-name").focus();
}
function readAdapterEditor() {
  const model_list = lines($("adapter-models").value).map((l) => {
    const columns = l.split("|").map((p) => p.trim());
    return { id: columns[0], label: columns[1] || columns[0], efforts: (columns[2] || "").split(",").map((e) => e.trim()).filter(Boolean), aliases: [] };
  });
  let event_rules;
  try {
    event_rules = JSON.parse($("adapter-rules").value.trim() || "[]");
    if (!Array.isArray(event_rules)) throw new Error("Rules must be an array.");
  } catch (e) { throw new Error("Invalid JSONL event rules: " + e.message); }
  const spec = {
    id: $("adapter-id").value.trim(), name: $("adapter-name").value.trim(),
    program: $("adapter-program").value.trim(), output: $("adapter-output").value,
    session: $("adapter-session").value, model_format: $("adapter-model-format").value,
    stdin: $("adapter-stdin").checked, enabled: $("adapter-enabled").checked,
    model_list, event_rules
  };
  for (const name of adapterFields) spec[name.replaceAll("-", "_")] = lines($("adapter-" + name).value);
  return spec;
}
$("adapter-new").addEventListener("click", () => openAdapterEditor({
  args: ["{{full_prompt}}"], model_args: ["--model", "{{model}}"],
  output: "text", session: "none", enabled: true
}));
$("adapter-agy").addEventListener("click", () => openAdapterEditor({
  id: "agy", name: "Agy", program: "agy",
  args: ["--print", "{{full_prompt}}"], model_args: ["--model", "{{model}}"],
  // Starter only: verify agy's installed version, CLI flags, and authentication before use.
  output: "text", session: "none", enabled: true
}));
$("adapter-cancel").addEventListener("click", () => { $("adapter-editor").hidden = true; });
$("adapter-editor").addEventListener("submit", async (e) => {
  e.preventDefault();
  try {
    const spec = readAdapterEditor();
    await api(editingAdapterId ? "PATCH" : "POST",
      editingAdapterId ? "/api/adapters/" + encodeURIComponent(editingAdapterId) : "/api/adapters", spec);
    delete modelLists[spec.id];
    modelBarKey = "";
    $("adapter-editor").hidden = true;
    showSettingsError("");
    await loadAdapters();
    await load();
  } catch (err) { showSettingsError(err.message); }
});
$("adapter-list").addEventListener("click", async (e) => {
  const b = e.target.closest("[data-adapter-act]");
  if (!b) return;
  const action = b.dataset.adapterAct;
  const id = b.dataset.adapterId;
  const item = adapterItems.find((a) => a.id === id);
  if (!item) return;
  if (action === "edit") return openAdapterEditor(item.spec, true);
  if (action === "clone") return openAdapterEditor({
    ...item.spec, id: item.id + "-copy", name: item.name + " copy"
  });
  if (action === "check") {
    try {
      const result = await api("POST", "/api/adapters/" + encodeURIComponent(id) + "/check");
      showSettingsError(item.name + ": " + result.message);
    } catch (err) { showSettingsError(err.message); }
    return;
  }
  if (action === "delete" && confirm("Delete adapter " + item.name + "? Bots must be reassigned first.")) {
    try {
      await api("DELETE", "/api/adapters/" + encodeURIComponent(id));
      delete modelLists[id];
      modelBarKey = "";
      showSettingsError("");
      await loadAdapters();
    } catch (err) { showSettingsError(err.message); }
  }
});
