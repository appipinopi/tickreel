(async () => {
  const checks = []; const assert = (condition, label) => { if (!condition) throw new Error(label); checks.push(label); };
  const until = async (predicate) => { for (let attempt=0;attempt<100;attempt++) { if (await predicate()) return; await new Promise(resolve=>setTimeout(resolve,100)); } throw new Error('Timed out waiting for UI'); };
  const click = async id => { document.getElementById(id).click(); await until(()=>!working); };
  try {
    await until(()=>sources.length>0); await update();
    assert(document.querySelector('.sidebar')!==null,'original sidebar');
    assert(getComputedStyle(document.querySelector('.sidebar')).width==='230px','original sidebar geometry');
    assert(document.getElementById('title-input')!==null,'original recording controls');
    document.getElementById('title-input').value='UI接続テスト';
    await click('record-button'); await until(async()=>{await update();return state.current?.frames>0;});
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
    window.ipc.postMessage(JSON.stringify({id:0,method:'qaReport',args:[{success:true,checks}]}));
  } catch (error) { window.ipc.postMessage(JSON.stringify({id:0,method:'qaReport',args:[{success:false,checks,error:String(error)}]})); }
})();
