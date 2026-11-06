// Calls the engine once and shows the answer. Plain script: the real front end comes in wave 3.
'use strict';
(async () => {
  const status = document.getElementById('status');
  const out = document.getElementById('answer');
  try {
    const csv = 'time,conc\n0,0\n1,8.1\n2,6.2\n4,3.3\n8,1.2\n';
    const request = JSON.stringify({ command: 'data.preview', params: { csv } });
    const text = await window.__TAURI__.core.invoke('engine_execute', { request });
    const answer = JSON.parse(text);
    status.textContent = answer.ok
      ? 'Answer of data.preview, sealed ' + (answer.seal ? answer.seal.sha256.slice(0, 12) : '(no seal)')
      : 'The engine refused: ' + answer.error.message;
    out.textContent = JSON.stringify(answer, null, 2);
  } catch (error) {
    status.textContent = 'The call failed: ' + error;
  }
})();
