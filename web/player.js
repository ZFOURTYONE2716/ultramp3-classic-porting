import init, { WebPlayer } from './pkg/ultramp3.js';

const files = document.querySelector('#files');
const demo = document.querySelector('#demo');
const status = document.querySelector('#status');
const canvas = document.querySelector('#player');
const ready = 'Pilih atau seret file MP3 dan WAV ke pemutar.';
let context;
let player;
let importing = false;

async function loadTracks(tracks) {
  if (importing || !player || !tracks.length) return;
  importing = true;
  files.disabled = demo.disabled = true;
  status.textContent = 'Membaca audio…';
  const failed = [];
  try {
    context ??= new AudioContext();
    if (navigator.userActivation?.isActive) await context.resume();
    for (const track of tracks) {
      try {
        if (!/\.(mp3|wav)$/i.test(track.name)) throw new Error('Format tidak didukung.');
        const bytes = await track.arrayBuffer();
        const buffer = await context.decodeAudioData(bytes.slice(0));
        player.add_audio(context, buffer, track.name, new Uint8Array(bytes));
      } catch {
        failed.push(track.name);
      }
    }
    status.textContent = failed.length ? `Gagal membaca ${failed.join(', ')}. Gunakan MP3 atau WAV.` : context.state === 'running' ? ready : 'Klik pemutar untuk mengaktifkan audio.';
  } catch (error) {
    status.textContent = `Gagal mengaktifkan Web Audio. ${error.message}`;
  } finally {
    importing = false;
    files.disabled = demo.disabled = false;
    canvas.focus();
  }
}

canvas.addEventListener('pointerup', async () => {
  if (!context || context.state === 'running') return;
  try {
    await context.resume();
    status.textContent = ready;
  } catch (error) {
    status.textContent = error.message;
  }
});

files.addEventListener('change', () => {
  const selected = [...files.files];
  files.value = '';
  void loadTracks(selected);
});

demo.addEventListener('click', () => void loadTracks([{
  name: 'UltraDemo.mp3',
  async arrayBuffer() {
    const response = await fetch('./UltraDemo.mp3');
    if (!response.ok) throw new Error('Demo tidak tersedia.');
    return response.arrayBuffer();
  },
}]));

window.addEventListener('dragover', event => {
  if (event.dataTransfer.types.includes('Files')) event.preventDefault();
}, true);

window.addEventListener('drop', event => {
  const selected = [...event.dataTransfer.files];
  if (!selected.length) return;
  event.preventDefault();
  event.stopPropagation();
  void loadTracks(selected);
}, true);

try {
  await init();
  player = await WebPlayer.start(canvas);
  files.disabled = demo.disabled = false;
  status.textContent = ready;
} catch (error) {
  status.textContent = `UltraMP3 gagal dimuat. ${error.message ?? error}`;
  console.error(error);
}
