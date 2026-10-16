const $ = id => document.getElementById(id), api = window.tickreel;
let captureCapabilities = { portalSelection: false, appRedaction: true, idleDetection: true };
let state = { current: null, recordings: [], exportState: null }, sources = [], selectedDetail = null, working = false, toastTimer, lastError;
let plan = null, playing = null, playStarted = 0, playPosition = 0, previewBusy = false, previewRequest = 0;
let recorder = null, micStream = null, micTimer = null, micStatusTimer = null, micStarted = 0, audioUrl = null;
const time = ms => [Math.floor(ms / 3600000), Math.floor(ms / 60000) % 60, Math.floor(ms / 1000) % 60].map(value => String(value).padStart(2, '0')).join(':');
const duration = seconds => seconds < 60 ? `${seconds.toFixed(1)}秒` : `${Math.floor(seconds / 60)}分${Math.round(seconds % 60)}秒`;
const selectedApps = () => Array.from(document.querySelectorAll('.privacy-choice:checked'), input => input.value);
const selectedScreens = () => [...new Set([$('source-select').value, ...Array.from(document.querySelectorAll('.monitor-choice:checked'), input => input.value)].filter(Boolean))];
function notify(message) { $('toast').textContent = message; $('toast').hidden = false; clearTimeout(toastTimer); toastTimer = setTimeout(() => $('toast').hidden = true, 6500); }
async function action(callback) { if (working) return; working = true; render(); try { await callback(); } catch (error) { notify(error.message.replace(/^Error invoking remote method '[^']+': Error: /, '')); } finally { await update(); working = false; render(); } }
function showView(library) { $('today-view').hidden = library; $('library-view').hidden = !library; $('nav-today').classList.toggle('active', !library); $('nav-library').classList.toggle('active', library); $('page-label').textContent = library ? 'ライブラリ' : '今日の記録'; }
function estimate() { $('pace-estimate').textContent = `1分の作業 → 約${(60 / Number($('interval-select').value) / Number($('fps-select').value)).toLocaleString('ja-JP', { maximumFractionDigits: 2 })}秒（無操作の追加早送り・エンドカードを除く）`;  $('estimate').textContent = `約${duration(8 * 3600 / Number($('interval-select').value) / Number($('fps-select').value))}の動画に`; }
function choice(value, label, className, checked = false) {
  const row = document.createElement('label'); row.className = 'choice-item'; const input = document.createElement('input'); input.type = 'checkbox'; input.value = value; input.className = className; input.checked = checked;
  const span = document.createElement('span'); span.textContent = label; span.title = label; row.append(input, span); return row;
}
async function loadSources(select = false) {
  const previous = $('source-select').value, checked = selectedScreens();
  sources = await (select ? api.selectSources(selectedApps()) : api.sources(selectedApps())); $('source-select').replaceChildren(); $('monitor-list').replaceChildren();
  for (const [index, source] of sources.entries()) {
    const option = document.createElement('option'); option.value = source.id; option.textContent = `ディスプレイ ${index + 1} · ${source.name}`; $('source-select').append(option);
    const row = choice(source.id, `ディスプレイ ${index + 1} · ${source.name}`, 'monitor-choice', checked.includes(source.id) && source.id !== previous); row.querySelector('input').onchange = showSource; $('monitor-list').append(row);
  }
  if (sources.some(source => source.id === previous)) $('source-select').value = previous;
  if (!sources.length) { const option = document.createElement('option'); option.value = ''; option.textContent = '画面が見つかりません'; $('source-select').append(option); }
  showSource();
}
async function loadWindows() {
  if (!captureCapabilities.appRedaction) { $('privacy-list').replaceChildren(); $('privacy-status').textContent = 'Waylandではアプリ別の黒塗りは利用できません'; return; }
  const previous = selectedApps(), windows = await api.windows(), apps = new Map();
  for (const item of windows) if (!apps.has(item.process)) apps.set(item.process, item.title);
  // Keep selected apps even when closed; future windows from the same app stay protected.
  for (const name of previous) if (!apps.has(name)) apps.set(name, '現在ウィンドウなし');
  $('privacy-list').replaceChildren();
  for (const [name, title] of [...apps.entries()].sort((a, b) => a[0].localeCompare(b[0]))) {
    const row = choice(name, `${name} · ${title}`, 'privacy-choice', previous.includes(name));
    row.querySelector('input').onchange = () => action(async () => { $('preview-image').hidden = true; await loadSources(); privacyStatus(); });
    $('privacy-list').append(row);
  }
  privacyStatus();
}
function privacyStatus() { $('privacy-status').textContent = selectedApps().length ? `${selectedApps().length}アプリを黒塗り · 検出失敗時は記録を停止` : '黒塗り対象なし'; }
function showSource() {
  if (state.current) return;
  const selected = sources.find(source => source.id === $('source-select').value), ids = selectedScreens();
  $('source-label').textContent = ids.length > 1 ? `${ids.length}画面を同時記録 · メインをプレビュー` : selected?.name || '画面を選択してください';
  $('preview-image').hidden = !selected; $('preview-empty').hidden = Boolean(selected); if (selected) $('preview-image').src = selected.thumbnail;
  for (const input of document.querySelectorAll('.monitor-choice')) { const primary = input.value === $('source-select').value; input.disabled = primary || Boolean(state.current) || working; input.closest('label').hidden = primary; }
}
function recordRow(item) {
  const row = document.createElement('article'); row.className = 'record-row';
  const icon = document.createElement('div'); icon.className = 'record-icon'; icon.textContent = item.category === 'development' ? '⌘' : '▥';
  const info = document.createElement('div'); info.className = 'record-info';
  const heading = document.createElement('h3'); heading.textContent = item.title;
  const description = document.createElement('p'); description.textContent = `${new Date(item.startedAt).toLocaleString('ja-JP', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })} · ${item.frames.toLocaleString()}枚 · 作業 ${Math.round(Math.max(0, item.activeMs - (item.idleMs || 0)) / 60000)}分 · ${(item.bytes / 1024 / 1024).toFixed(1)} MB`;
  info.append(heading, description);
  const status = document.createElement('span'); status.className = 'row-status'; status.textContent = ({ recording: '記録中', paused: '一時停止', complete: '保存済み', interrupted: '中断・保存済み' })[item.status] || '保存済み';
  const active = state.current?.id === item.id, exporting = Boolean(state.exportState);
  const view = document.createElement('button'); view.className = 'secondary'; view.textContent = '編集'; view.disabled = !item.frames || active || working || Boolean(recorder); view.onclick = () => action(() => showDetail(item));
  const formatSelect=document.createElement('select');formatSelect.setAttribute('aria-label','書き出し形式');formatSelect.innerHTML=$('export-format').innerHTML;formatSelect.value=localStorage.getItem('exportFormat')||'mp4';formatSelect.onchange=()=>localStorage.setItem('exportFormat',formatSelect.value);
  const generate = document.createElement('button'); generate.className = 'primary'; generate.textContent = state.exportState?.ids?.includes(item.id) ? `生成 ${state.exportState.progress}%` : '✦ ワンクリック生成'; generate.disabled = exporting || active || !item.frames || working || Boolean(recorder); generate.onclick = () => action(async () => { await api.export(item.id, { quick: true, format: formatSelect.value }); notify('ファイルを書き出しました。ビデオ／Tickreelに保存しています。'); });
  const remove = document.createElement('button'); remove.className = 'secondary danger'; remove.textContent = '削除'; remove.disabled = active || exporting || working || Boolean(recorder); remove.onclick = () => action(async () => { if (await api.delete(item.id)) notify('記録を削除しました。書き出したファイルは残っています。'); });
  row.append(icon, info, status, view, formatSelect, generate, remove); return row;
}
function renderList(id, items) {
  const element = $(id); element.replaceChildren();
  if (!items.length) { const empty = document.createElement('div'); empty.className = 'empty-library'; empty.textContent = 'まだ記録はありません。最初の一日を、ここに残しましょう。'; element.append(empty); }
  else items.forEach(item => element.append(recordRow(item)));
}
function summaryRecords() {
  const start = new Date(`${$('summary-date').value}T00:00:00`); if (!Number.isFinite(start.getTime())) return [];
  if ($('summary-period').value === 'month') start.setDate(1); else start.setDate(start.getDate() - ((start.getDay() + 6) % 7));
  const end = new Date(start); if ($('summary-period').value === 'month') end.setMonth(end.getMonth() + 1); else end.setDate(end.getDate() + 7);
  return state.recordings.filter(item => item.frames && item.id !== state.current?.id && new Date(item.startedAt) >= start && new Date(item.startedAt) < end);
}
function render() {
  const current = state.current, paused = current?.status === 'paused';
  $('top-status').lastChild.textContent = current ? paused ? '記録を一時停止しています' : '今日の作業を記録しています' : '記録の準備ができています';
  $('preview-dot').classList.toggle('live', Boolean(current && !paused));
  $('elapsed').textContent = time(current?.activeMs || 0); $('frames').textContent = `${(current?.frames || 0).toLocaleString()} 枚`; $('video-duration').textContent = duration((current?.frames || 0) / (current?.fps || Number($('fps-select').value)));
  $('record-button').textContent = current ? '■ 記録を終了' : '● 記録を開始'; $('record-button').classList.toggle('stop', Boolean(current)); $('record-button').disabled = working || (!current && !sources.length);
  $('pause-button').hidden = !current; $('pause-button').textContent = paused ? '▶ 記録を再開' : 'Ⅱ 一時停止'; $('pause-button').disabled = working;
  for (const id of ['title-input', 'category-select', 'source-select', 'interval-select', 'fps-select', 'pace-four-seconds', 'select-portal-sources', 'release-portal-sources', 'refresh-sources', 'refresh-windows', 'layout-select', 'idle-threshold', 'idle-speed']) $(id).disabled = Boolean(current) || working;
  for (const input of document.querySelectorAll('.monitor-choice,.privacy-choice')) input.disabled = Boolean(current) || working;
  if (!captureCapabilities.appRedaction) $('refresh-windows').disabled = true;
  if (!captureCapabilities.idleDetection) { $('idle-threshold').disabled = true; $('idle-speed').disabled = true; }
  $('action-note').textContent = current ? paused ? '再開すると、続きから記録します' : `${current.interval}秒ごとに保存 · 黒塗り${(current.protectedApps || []).length}アプリ` : '準備ができたら、記録を始めましょう';
  $('preview-overlay').hidden = !current; $('overlay-status').textContent = paused ? 'PAUSED' : 'REC'; $('overlay-time').textContent = time(current?.activeMs || 0);
  if (current) $('source-label').textContent = current.sourceName;
  $('library-count').textContent = state.recordings.length; $('recent-count').textContent = state.recordings.length;
  renderList('recent-list', state.recordings.slice(0, 3)); renderList('library-list', state.recordings);
  const blocked = working || Boolean(state.exportState) || Boolean(recorder) || state.current?.id === selectedDetail?.id;
  for (const id of ['detail-export', 'detail-save-as', 'save-edit', 'pick-bgm', 'remove-narration']) $(id).disabled = blocked;
  $('mic-button').disabled = working || Boolean(state.exportState) || state.current?.id === selectedDetail?.id;
  $('mic-button').textContent = recorder ? '■ 録音を停止して保存' : '● 録音を開始'; $('mic-button').classList.toggle('mic-live', Boolean(recorder));
  $('export-progress').textContent = state.exportState ? `動画を生成しています… ${state.exportState.progress}%` : '';
  $('summary-count').textContent = `${summaryRecords().length}件の記録をまとめます。記録を開始した日で集計します。${state.exportState ? ` 生成中 ${state.exportState.progress}%` : ''}`;
  $('summary-generate').disabled = working || Boolean(state.exportState) || Boolean(recorder) || !summaryRecords().length;
}
async function update() {
  try {
    state = await api.state(); render();
    if (state.captureError && lastError !== state.captureError) notify(state.captureError); lastError = state.captureError;
    if (state.current) { const preview = await api.preview(state.current.id); if (preview) { $('preview-image').src = preview; $('preview-image').hidden = false; $('preview-empty').hidden = true; } }
    else showSource();
  } catch (error) { notify(error.message); }
}
function parseClock(value) { const fields = value.trim().split(':'); if (fields.length > 3 || fields.some(field => !/^\d+(\.\d+)?$/.test(field))) throw new Error('時刻の形式を確認してください。'); return fields.reduce((sum, field) => sum * 60 + Number(field), 0); }
let captionDraft = [], captionIndex = -1;
function parseCaptions() { return captionDraft.filter(c=>c.text.trim()).map(c=>({...c})); }
function captionForm() {
  $('caption-fields').hidden=captionIndex<0; const c=captionDraft[captionIndex];
  if(c){$('caption-text').value=c.text;$('caption-start').value=c.start.toFixed(2);$('caption-end').value=(c.start+c.duration).toFixed(2);$('caption-size').value=c.size;$('caption-color').value=c.color;for(const id of ['caption-in-slider','caption-out-slider'])$(id).max=plan.duration;$('caption-in-slider').value=c.start;$('caption-out-slider').value=c.start+c.duration;}
  captionList();
}
function captionList() {
  $('caption-list').replaceChildren();
  captionDraft.forEach((c,i)=>{const button=document.createElement('button');button.className='caption-bar'+(i===captionIndex?' selected':'');const text=document.createElement('span');text.textContent=c.text||'新しいテロップ';const clock=document.createElement('small');clock.textContent=`${c.start.toFixed(1)}–${(c.start+c.duration).toFixed(1)}秒`;button.append(text,clock);button.onclick=()=>{stopPlayback();captionIndex=i;captionForm();$('preview-seek').value=c.start;paintPreview(c.start);};$('caption-list').append(button);});
}
function captionPaint(position) {
  const host=$('editor-caption');host.replaceChildren();host.className='';
  if(position>=plan.duration)return;
  captionDraft.forEach((c,i)=>{if(position<c.start||position>=c.start+c.duration)return;const el=document.createElement('div');el.className='editor-caption caption-object'+(i===captionIndex?' selected':'');el.textContent=c.text;el.style.left=`${c.x*100}%`;el.style.top=`${c.y*100}%`;el.style.color=c.color;el.style.fontSize=`${c.size*$('editor-screen').clientWidth/1920}px`;el.onpointerdown=e=>{stopPlayback();captionIndex=i;captionForm();el.setPointerCapture(e.pointerId);e.preventDefault();};el.onpointermove=e=>{if(!el.hasPointerCapture(e.pointerId))return;const r=$('editor-screen').getBoundingClientRect();c.x=Math.max(.02,Math.min(.98,(e.clientX-r.left)/r.width));c.y=Math.max(.02,Math.min(.98,(e.clientY-r.top)/r.height));el.style.left=`${c.x*100}%`;el.style.top=`${c.y*100}%`;};host.append(el);});
}
$('caption-add').onclick=()=>{if(!plan||captionDraft.length>=200)return;stopPlayback();const start=Math.min(Number($('preview-seek').value),Math.max(0,plan.duration-.01));captionDraft.push({start,duration:Math.min(5,plan.duration-start),text:'新しいテロップ',x:.5,y:.88,size:46,color:'#ffffff'});captionIndex=captionDraft.length-1;captionForm();$('preview-seek').value=start;paintPreview(start);$('caption-text').focus();$('caption-text').select();};
for(const id of ['caption-text','caption-start','caption-end','caption-size','caption-color']) $(id).oninput=()=>{const c=captionDraft[captionIndex];if(!c)return;c.text=$('caption-text').value;c.start=Math.max(0,Math.min(plan.duration-.01,Number($('caption-start').value)||0));const end=Math.min(plan.duration,Math.max(c.start+.01,Number($('caption-end').value)||c.start+.01));c.duration=end-c.start;$('caption-in-slider').value=c.start;$('caption-out-slider').value=end;c.size=Number($('caption-size').value);c.color=$('caption-color').value;captionList();$('preview-seek').value=c.start;paintPreview(c.start);};
for(const [slider,field] of [['caption-in-slider','caption-start'],['caption-out-slider','caption-end']])$(slider).oninput=()=>{$(field).value=$(slider).value;$(field).dispatchEvent(new Event('input'));};
$('export-format').value=localStorage.getItem('exportFormat')||'mp4';$('export-format').onchange=()=>localStorage.setItem('exportFormat',$('export-format').value);
$('caption-remove').onclick=()=>{captionDraft.splice(captionIndex,1);captionIndex=captionDraft.length?Math.min(captionIndex,captionDraft.length-1):-1;captionForm();paintPreview(Number($('preview-seek').value));};
$('caption-center').onclick=()=>{const c=captionDraft[captionIndex];if(c){c.x=.5;c.y=.88;paintPreview(Number($('preview-seek').value));}};
function readEdit() { return { theme: $('theme-select').value, bgm: $('bgm-select').value, stats: $('stats-check').checked, captions: parseCaptions(), narrationOffset: Number($('voice-offset').value), narrationVolume: Number($('voice-volume').value), bgmVolume: Number($('bgm-volume').value), idleSpeed: Number($('edit-idle-speed').value) }; }
async function saveEdit() { selectedDetail = await api.edit(selectedDetail.id, readEdit()); plan = await api.plan(selectedDetail.id, Number($('edit-idle-speed').value)); $('preview-seek').max = plan.totalDuration; await paintPreview(Number($('preview-seek').value)); }
async function refreshNarration() {
  $('narration-audio').pause(); if (audioUrl) URL.revokeObjectURL(audioUrl); audioUrl = null;
  const bytes = await api.audioPreview(selectedDetail.id); $('narration-audio').hidden = !bytes; $('remove-narration').hidden = !bytes;
  if (bytes) { $('narration-audio').src = bytes; }
  else $('narration-audio').removeAttribute('src');
}
async function showDetail(item) {
  stopPlayback(); selectedDetail = item; plan = await api.plan(item.id); const edit = item.edit || {};
  $('detail-title').textContent = item.title; $('theme-select').value = edit.theme || (item.category === 'development' ? 'terminal' : 'paper'); $('bgm-select').value = edit.bgm || 'auto'; $('stats-check').checked = edit.stats !== false;
  $('voice-offset').value = edit.narrationOffset || 0; $('voice-volume').value = edit.narrationVolume ?? 1; $('bgm-volume').value = edit.bgmVolume ?? 0.18; $('edit-idle-speed').value = item.idleSpeed || 1;
  captionDraft=(edit.captions||[]).map(c=>({x:.5,y:.88,size:46,color:'#ffffff',...c}));captionIndex=captionDraft.length?0:-1;captionForm();
  $('bgm-name').textContent = item.bgmName || '内蔵BGMをローカルで生成します。'; $('preview-seek').value = 0; $('preview-seek').max = plan.totalDuration;
  await refreshNarration(); $('detail-dialog').showModal(); await paintPreview(0); render();
}
async function paintPreview(position) {
  if (!selectedDetail || !plan) return;
  const request = ++previewRequest, end = position >= plan.duration;
  $('editor-endcard').hidden = !end; $('editor-screen').className = `editor-screen ${$('theme-select').value}`;
  $('editor-title').textContent = !end && position < 2.5 && $('theme-select').value !== 'clean' ? selectedDetail.title : '';
  $('editor-stats').textContent = $('stats-check').checked && !end ? `作業 ${Math.round(Math.max(0, selectedDetail.activeMs - (selectedDetail.idleMs || 0)) / 60000)}分 · 無操作 ${Math.round((selectedDetail.idleMs || 0) / 60000)}分 · ${selectedDetail.frames}枚 · ${(selectedDetail.sourceIds || [selectedDetail.sourceId]).length}画面` : '';
  captionPaint(position);
  $('preview-position').textContent = `${Math.floor(position / 60)}:${String(Math.floor(position % 60)).padStart(2, '0')}`;
  $('detail-info').textContent = `${selectedDetail.frames}枚 → ${plan.indices.length}枚（無操作を圧縮） / 動画 ${duration(plan.totalDuration)}（エンドカード3秒を含む）。映像・テロップと保存済みアフレコを確認できます。BGMとフェードの確認は生成した動画で。`;
  if (!end) { const index = plan.indices[Math.min(plan.indices.length - 1, Math.floor(position * selectedDetail.fps))]; if (index !== undefined) { const image = await api.preview(selectedDetail.id, index); if (request === previewRequest) $('detail-image').src = image; } }
}
function stopPlayback() { clearInterval(playing); playing = null; $('preview-play').textContent = '▶ プレビュー'; $('narration-audio').pause(); }
function startPlayback() {
  if (playing) { stopPlayback(); return; } if (!plan) return;
  if (Number($('preview-seek').value) >= plan.totalDuration) $('preview-seek').value = 0;
  playPosition = Number($('preview-seek').value); playStarted = performance.now(); $('preview-play').textContent = 'Ⅱ 停止';
  const audio = $('narration-audio'), offset = Number($('voice-offset').value); audio.volume = Math.min(1, Number($('voice-volume').value));
  if (!audio.hidden && playPosition >= offset && playPosition < plan.duration) { audio.currentTime = playPosition - offset; audio.play().catch(() => {}); }
  playing = setInterval(async () => {
    if (previewBusy) return; previewBusy = true;
    try {
      const position = Math.min(plan.totalDuration, playPosition + (performance.now() - playStarted) / 1000); $('preview-seek').value = position;
      if (!audio.hidden && audio.paused && position >= offset && position < plan.duration && position - offset < audio.duration) { audio.currentTime = position - offset; audio.play().catch(() => {}); }
      if (position >= plan.duration) audio.pause(); await paintPreview(position); if (position >= plan.totalDuration) stopPlayback();
    } catch (error) { stopPlayback(); notify(error.message); } finally { previewBusy = false; }
  }, 200);
}
async function startNarration() {
  if (!selectedDetail) return;
  await api.micStart(selectedDetail.id);
  recorder = { native: true, id: selectedDetail.id }; micStarted = Date.now();
  micTimer = setTimeout(() => action(stopNarration), 10 * 60 * 1000);
  micStatusTimer = setInterval(() => $('mic-status').textContent = `● 録音中 ${time(Date.now() - micStarted)}`, 500); render();
}
async function stopNarration() {
  if (!recorder) return;
  const item = await api.micStop(recorder.id); recorder = null; clearTimeout(micTimer); clearInterval(micStatusTimer);
  if (selectedDetail?.id === item.id) { selectedDetail = item; await refreshNarration(); }
  $('mic-status').textContent = 'マイク録音 · 最大10分'; notify('アフレコを保存しました。'); render();
}
window.addEventListener('tickreel-native-mic-stopped', async event => {
  recorder = null; clearTimeout(micTimer); clearInterval(micStatusTimer); $('mic-status').textContent = 'マイク録音 · 最大10分';
  if (event.detail.record && selectedDetail?.id === event.detail.record.id) { selectedDetail = event.detail.record; await refreshNarration(); }
  notify(event.detail.error || 'アフレコを保存しました。'); render();
});
function closeDetail() { if (recorder) { notify('録音を停止して保存してから閉じてください。'); return; } stopPlayback(); $('detail-dialog').close(); }
$('record-button').onclick = () => action(async () => {
  if (state.current) { await api.stop(); notify('記録を保存しました。「ワンクリック生成」で動画にできます。'); }
  else { await api.start({ title: $('title-input').value, sourceId: $('source-select').value, sourceIds: selectedScreens(), category: $('category-select').value, interval: Number($('interval-select').value), fps: Number($('fps-select').value), protectedApps: selectedApps(), layout: $('layout-select').value, idleThreshold: Number($('idle-threshold').value), idleSpeed: Number($('idle-speed').value) }); notify('記録を開始しました。'); }
});
$('pause-button').onclick = () => action(() => state.current?.status === 'paused' ? api.resume() : api.pause());
$('select-portal-sources').onclick = () => action(() => loadSources(true));
$('release-portal-sources').onclick = () => action(async () => { await api.releaseSources(); await loadSources(); });
$('refresh-sources').onclick = () => action(() => loadSources(captureCapabilities.portalSelection)); $('refresh-windows').onclick = () => action(loadWindows); $('source-select').onchange = showSource;
$('pace-four-seconds').onclick = () => { $('interval-select').value = '1'; $('fps-select').value = '15'; $('idle-speed').value = '1'; estimate(); };
$('interval-select').onchange = estimate; $('fps-select').onchange = estimate;
$('category-select').onchange = () => $('title-input').placeholder = $('category-select').value === 'development' ? '例：Webアプリ開発の一日' : '例：イラスト制作の一日';
$('open-folder').onclick = () => action(() => api.folder()); $('nav-today').onclick = () => showView(false); $('nav-library').onclick = $('view-all').onclick = () => showView(true);
$('close-detail').onclick = closeDetail; $('detail-dialog').addEventListener('cancel', event => { if (recorder) { event.preventDefault(); notify('録音を停止して保存してから閉じてください。'); } else stopPlayback(); });
$('detail-dialog').addEventListener('close', () => { stopPlayback(); if (audioUrl) URL.revokeObjectURL(audioUrl); audioUrl = null; });
$('preview-play').onclick = startPlayback; $('preview-seek').oninput = () => { stopPlayback(); paintPreview(Number($('preview-seek').value)).catch(error => notify(error.message)); };
for (const id of ['theme-select', 'stats-check']) $(id).addEventListener('input', () => paintPreview(Number($('preview-seek').value)).catch(error => notify(error.message)));
$('edit-idle-speed').onchange = () => action(async () => { stopPlayback(); plan = await api.plan(selectedDetail.id, Number($('edit-idle-speed').value)); $('preview-seek').max = plan.totalDuration; $('preview-seek').value = 0; await paintPreview(0); });
$('save-edit').onclick = () => action(async () => { await saveEdit(); notify('編集内容を保存しました。'); });
async function generate(quick) { stopPlayback(); await saveEdit(); await api.export(selectedDetail.id, { quick, format: $('export-format').value }); notify('ファイルを書き出しました。'); }
$('detail-export').onclick = () => action(() => generate(true)); $('detail-save-as').onclick = () => action(() => generate(false));
$('mic-button').onclick = () => action(() => recorder ? stopNarration() : startNarration());
$('remove-narration').onclick = () => action(async () => { selectedDetail = await api.removeAudio(selectedDetail.id, 'narration'); await refreshNarration(); notify('録音を削除しました。'); });
$('pick-bgm').onclick = () => action(async () => { const item = await api.pickBgm(selectedDetail.id); if (item) { selectedDetail = item; $('bgm-select').value = 'custom'; $('bgm-name').textContent = item.bgmName; } });
for (const id of ['summary-period', 'summary-date']) $(id).onchange = render;
$('summary-generate').onclick = () => action(async () => { await api.summary({ period: $('summary-period').value, date: $('summary-date').value, theme: $('summary-theme').value, format: $('summary-format').value }); notify('まとめ動画を生成しました。'); });
window.addEventListener('beforeunload', event => { if (recorder) { event.preventDefault(); event.returnValue = ''; } });
const today = new Date(); $('today-date').textContent = today.toLocaleDateString('ja-JP', { year: 'numeric', month: 'long', day: 'numeric', weekday: 'short' }); $('summary-date').value = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}-${String(today.getDate()).padStart(2, '0')}`;
estimate();
async function initializeCapture() {
  captureCapabilities = await api.captureCapabilities();
  if (captureCapabilities.portalSelection) {
    $('select-portal-sources').hidden = false;
    $('release-portal-sources').hidden = false;
    $('capture-notice').hidden = false;
    $('capture-notice').textContent = 'Wayland: OSの画面共有ダイアログで記録する画面を選びます。共有は「画面共有を終了」またはアプリ終了まで続きます。アプリ別の黒塗りと無操作の自動早送りは利用できません。';
    $('refresh-sources').title = '共有する画面を選び直す';
    $('refresh-sources').setAttribute('aria-label', '共有する画面を選び直す');
  }
  if (!captureCapabilities.idleDetection) $('idle-speed').value = '1';
  const results = await Promise.allSettled([loadSources(), loadWindows()]);
  results.forEach(result => { if (result.status === 'rejected') notify(result.reason.message); });
  await update();
}
initializeCapture().catch(error => notify(error.message));
window.appSignatureReady = (async () => {
  const [{ mountSignature }, signature] = await Promise.all([import('./app-signature.js'), api.publicLinks()]);
  return mountSignature($('app-signature'), { signature, onOpenLink: id => api.openPublicLink(id), onError: error => notify(error.message) });
})();
window.appSignatureReady.catch(error => notify(error.message));
let signatureConfig = '';
setInterval(async () => {
  try {
    const signature = await api.publicLinks(), serialized = JSON.stringify(signature);
    if (serialized === signatureConfig) return;
    const { mountSignature } = await import('./app-signature.js');
    mountSignature($('app-signature'), { signature, onOpenLink: id => api.openPublicLink(id), onError: error => notify(error.message) });
    signatureConfig = serialized;
  } catch { /* Keep the last displayed links when the backend is unavailable. */ }
}, 60000);
setInterval(update, 1500);

let updateStatus=null;
function paintUpdate(value){updateStatus=value;$('update-message').textContent=value.message;$('update-check').hidden=value.channel==='store';$('update-check').disabled=['checking','downloading','ready'].includes(value.phase);$('update-download').hidden=value.phase!=='available';$('update-download').disabled=Boolean(state.current||state.exportState||recorder||working);}
$('update-check').onclick=async()=>{try{paintUpdate({...updateStatus,phase:'checking',message:'更新を確認しています…'});paintUpdate(await api.updateCheck());}catch(e){notify(e.message);}};
$('update-download').onclick=async()=>{try{$('update-download').disabled=true;paintUpdate(await api.updateDownload());}catch(e){notify(e.message);}};
async function pollUpdate(){try{paintUpdate(await api.updateState());}catch(e){console.warn(e.message);}}
pollUpdate();setInterval(pollUpdate,2000);
