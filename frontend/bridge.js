(() => {
  let next = 0; const pending = new Map();
  window.__tickreelResponse = (id, result, error) => {
    const callback = pending.get(id); if (!callback) return; pending.delete(id);
    if (error) callback.reject(new Error(error)); else callback.resolve(result);
  };
  function invoke(method, ...args) { return new Promise((resolve, reject) => { const id = ++next; pending.set(id, { resolve, reject }); window.ipc.postMessage(JSON.stringify({ id, method, args })); }); }
  window.tickreel = Object.fromEntries(['state','sources','windows','start','pause','resume','stop','plan','preview','edit','delete','export','summary','folder','audioPreview','removeAudio','pickBgm','micStart','micStop','updateState','updateCheck','updateDownload'].map(method => [method, (...args) => invoke(method, ...args)]));
  window.addEventListener('error', event => window.ipc.postMessage(JSON.stringify({id:0, method:'frontendError', args:[event.message]})));
  window.addEventListener('unhandledrejection', event => window.ipc.postMessage(JSON.stringify({id:0, method:'frontendError', args:[String(event.reason)]})));
})();
