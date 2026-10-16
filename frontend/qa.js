(async () => {
  const checks = []; const assert = (condition, label) => { if (!condition) throw new Error(label); checks.push(label); };
  const until = async (predicate) => { for (let attempt=0;attempt<100;attempt++) { if (await predicate()) return; await new Promise(resolve=>setTimeout(resolve,100)); } throw new Error('Timed out waiting for UI'); };
  const click = async id => { document.getElementById(id).click(); await until(()=>!working); };
  try {
    await window.appSignatureReady;
    assert(document.querySelector('[data-signature-link="creator"]').textContent.includes('Made by appipinopi'),'Shared creator signature');
    const openedLinks = [], originalOpenLink = api.openPublicLink;
    api.openPublicLink = async id => { const result = await originalOpenLink(id); openedLinks.push(result); return result; };
    const appLocation = window.location.href;
    document.querySelector('[data-signature-link="creator"]').click();
    await until(() => openedLinks.length === 1);
    assert(openedLinks[0].url === 'https://github.com/appipinopi','Creator signature opens developer GitHub through native bridge');
    document.querySelector('[data-signature-link="website"]').click();
    await until(() => openedLinks.length === 2);
    assert(openedLinks[1].url === 'https://tickreel.ycccompany.jp/','Official site opens through native bridge');
    document.querySelector('[data-signature-link="works"]').click();
    await until(() => openedLinks.length === 3);
    assert(openedLinks[2].url === 'https://portal.88soundboard.workers.dev/','Creator works opens the shared app portal');
    assert(window.location.href === appLocation,'External links keep the workspace open');
    let rejectedLink = false;
    try { await originalOpenLink('file:///C:/Windows/system32/cmd.exe'); } catch { rejectedLink = true; }
    assert(rejectedLink,'Native bridge rejects unregistered URLs');
    api.openPublicLink = originalOpenLink;
    if (window.location.hash.includes('wayland')) {
      await until(() => captureCapabilities.portalSelection);
      await update();
      assert(sources.length===0,'Wayland does not request screen sharing at startup');
      assert(!document.getElementById('select-portal-sources').hidden,'Wayland screen chooser control');
      assert(document.getElementById('idle-speed').disabled && document.getElementById('idle-speed').value==='1','Wayland idle detection disabled');
      assert(document.getElementById('refresh-windows').disabled,'Wayland app redaction disabled');
      await click('select-portal-sources');
    }
    await update();
    assert(document.querySelector('.sidebar')!==null,'original sidebar');
    assert(getComputedStyle(document.querySelector('.sidebar')).width==='230px','original sidebar geometry');
    assert(document.getElementById('title-input')!==null,'original recording controls');const updater=await api.updateState();assert(updater.current==='1.0.2'&&['github','store'].includes(updater.channel),'Native updater status');assert(document.getElementById('update-check')!==null,'Updater controls');if(window.location.hash.includes('update')){await update();document.getElementById('toast').hidden=true;window.ipc.postMessage(JSON.stringify({id:0,method:'qaReport',args:[{success:true,checks}]}));return;}
    await until(()=>sources.length>0);
    document.getElementById('title-input').value='UI接続テスト';
    await click('pace-four-seconds');
    assert(document.getElementById('interval-select').value==='1' && document.getElementById('fps-select').value==='15' && document.getElementById('idle-speed').value==='1','One minute to four seconds preset');
    assert(document.getElementById('pace-estimate').textContent.includes('約4秒'),'Four second duration estimate');
    await click('record-button'); await until(async()=>{await update();return state.current?.frames>0;});
    assert(state.current.interval===1 && state.current.fps===15 && state.current.idleSpeed===1,'Native four second recording settings');
    assert(document.getElementById('pace-four-seconds').disabled,'Pace preset disabled while recording');
    assert(state.current.status==='recording','Rust recording starts');
    await click('pause-button'); assert(state.current.status==='paused','Rust pause');
    await click('pause-button'); assert(state.current.status==='recording','Rust resume');
    await click('record-button'); assert(state.current===null,'Rust stop and save');
    const record=state.recordings[0];await showDetail(record);
    assert(document.getElementById('detail-dialog').open,'original studio dialog');
    document.getElementById('caption-add').click();document.getElementById('caption-text').value='元のUIでRustを使う';document.getElementById('caption-text').dispatchEvent(new Event('input'));document.getElementById('caption-color').value='#ffcc00';document.getElementById('caption-color').dispatchEvent(new Event('input'));
    document.getElementById('theme-select').value='terminal';await saveEdit();
    assert(selectedDetail.edit.captions[0].text==='元のUIでRustを使う','Rust caption persistence');assert(selectedDetail.edit.captions[0].color==='#ffcc00','GUI caption color persistence');
    await startNarration();assert(recorder?.native,'Rust microphone interface');await stopNarration();
    assert(!document.getElementById('narration-audio').hidden,'Rust WAV preview route');
    const response=await fetch(await api.audioPreview(record.id), {headers:{Range:'bytes=0-43'}});assert(response.status===206,'Rust WAV seeking');
    const header=new Uint8Array(await response.arrayBuffer());assert(String.fromCharCode(...header.slice(0,4))==='RIFF','Rust WAV preview data');
    await generate(true);await update();checks.push('Rust MP4 generation from original studio');for(const format of ['mov','webm','mkv','avi','gif','wav','mp3','flac','m4a','ogg','aac']){const result=await api.export(record.id,{quick:true,format});assert(result.path.endsWith('.'+format),'Export '+format);} 
    if (!window.location.hash.includes('editor')) { closeDetail();showView(true);assert(!document.getElementById('library-view').hidden,'original library');await api.summary({period:'month',date:document.getElementById('summary-date').value,theme:'cinema'});checks.push('Rust summary generation');await api.delete(record.id);await update();assert(state.recordings.length===0,'Rust recording deletion');showView(false); }
    await update();document.getElementById('toast').hidden=true;if(document.getElementById('detail-dialog').open){await paintPreview(0);const r=document.querySelector('.caption-object').getBoundingClientRect(),screen=document.getElementById('editor-screen').getBoundingClientRect();assert(Math.abs((r.top+r.height/2-screen.top)/screen.height-captionDraft[0].y)<.03,'Preview caption matches exported coordinates');}
    if (window.location.hash.includes('wayland')) { await click('release-portal-sources'); assert(sources.length===0 && document.getElementById('preview-image').hidden,'Wayland sharing release clears preview'); }
    window.ipc.postMessage(JSON.stringify({id:0,method:'qaReport',args:[{success:true,checks}]}));
  } catch (error) { window.ipc.postMessage(JSON.stringify({id:0,method:'qaReport',args:[{success:false,checks,error:String(error)}]})); }
})();
