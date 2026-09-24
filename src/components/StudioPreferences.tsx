// IA'GS appearance preferences. User backgrounds never leave this device.
import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { ImagePlus, RotateCcw, X } from 'lucide-react';
import { useI18n } from '../i18n';

export const palettes = {
  blue: { zh: '星蓝', en: 'Blue', rgb: '112, 177, 255', color: '#70b1ff' },
  violet: { zh: '紫罗兰', en: 'Violet', rgb: '183, 153, 255', color: '#b799ff' },
  teal: { zh: '青碧', en: 'Teal', rgb: '99, 216, 205', color: '#63d8cd' },
  gold: { zh: '暖金', en: 'Gold', rgb: '229, 192, 120', color: '#e5c078' },
};
type Palette = keyof typeof palettes;
function saved(key: string, fallback: string) { try { return localStorage.getItem(key) ?? fallback; } catch { return fallback; } }
async function backgroundStorage(write?: Blob | null): Promise<Blob | undefined> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open('iags-appearance', 1);
    request.onupgradeneeded = () => request.result.createObjectStore('background');
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const db = request.result;
      const tx = db.transaction('background', write === undefined ? 'readonly' : 'readwrite');
      const store = tx.objectStore('background');
      const op = write === undefined ? store.get('image') : write === null ? store.delete('image') : store.put(write, 'image');
      let value: Blob | undefined;
      op.onsuccess = () => { if (write === undefined) value = op.result; };
      tx.oncomplete = () => { db.close(); resolve(value); };
      tx.onerror = () => { db.close(); reject(tx.error); };
      tx.onabort = () => { db.close(); reject(tx.error); };
    };
  });
}
export function useStudioAppearance() {
  const [palette, setPalette] = useState<Palette>(() => { const key = saved('iags-palette', 'blue'); return Object.hasOwn(palettes, key) ? key as Palette : 'blue'; });
  const [dim, setDim] = useState(() => { const value = Number(saved('iags-background-dim', '.22')); return Number.isFinite(value) ? Math.max(0, Math.min(.85, value)) : .22; });
  const [image, setImage] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const revision = useRef(0);
  const url = useRef<string | null>(null);
  const replaceImage = (blob?: Blob) => {
    if (url.current) URL.revokeObjectURL(url.current);
    url.current = blob ? URL.createObjectURL(blob) : null;
    setImage(url.current);
  };
  useEffect(() => {
    let disposed = false;
    const initialRevision = revision.current;
    void backgroundStorage().then(blob => { if (!disposed && revision.current === initialRevision) replaceImage(blob); }).catch(() => undefined);
    return () => { disposed = true; if (url.current) URL.revokeObjectURL(url.current); };
  }, []);
  useEffect(() => { try { localStorage.setItem('iags-palette', palette); localStorage.setItem('iags-background-dim', String(dim)); } catch { /* nonessential preference */ } }, [palette, dim]);
  async function chooseBackground(file: File | null) {
    if (!file) return;
    setError('');setBusy(true);revision.current++;
    let decoded = false;
    try {
      if (!['image/png', 'image/jpeg', 'image/webp'].includes(file.type) || file.size > 20 * 1024 * 1024) throw new Error('format');
      // Decode to validate; retain the user's original image, no upload or generative edit.
      const bitmap = await createImageBitmap(file);
      if (bitmap.width * bitmap.height > 60_000_000) { bitmap.close(); throw new Error('size'); }
      bitmap.close(); decoded = true;
      await backgroundStorage(file); replaceImage(file);
    } catch { setError(decoded ? 'storage' : 'background'); } finally { setBusy(false); }
  }
  async function resetBackground() {
    setBusy(true);setError('');revision.current++;
    try { await backgroundStorage(null); replaceImage(); setDim(.22); } catch { setError('storage'); } finally { setBusy(false); }
  }
  const style = { '--accent': palettes[palette].color, '--accent-rgb': palettes[palette].rgb, '--sky-dim': dim, ...(image ? { '--sky-image': `url("${image}")` } : {}) } as CSSProperties;
  return { palette, setPalette, dim, setDim, image, error, busy, chooseBackground, resetBackground, style };
}
export function StudioPreferences({ appearance, scale, onScale, onClose }: { appearance: ReturnType<typeof useStudioAppearance>; scale: number; onScale: (scale: number) => void; onClose: () => void }) {
  const { locale, toggleLocale } = useI18n();
  const L = (zh: string, en: string) => locale === 'zh-CN' ? zh : en;
  const fileInput = useRef<HTMLInputElement>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { dialog.current?.showModal(); }, []);
  return <dialog className="studio-settings" ref={dialog} onCancel={onClose} aria-labelledby="settings-heading">
    <header><h2 id="settings-heading">{L('设置','Settings')}</h2><button aria-label={L('关闭设置','Close settings')} onClick={onClose}><X size={20}/></button></header>
    <section><h3>{L('配色','Color')}</h3><div className="palette-list" role="radiogroup" aria-label={L('配色','Color')}>{Object.entries(palettes).map(([key, value]) => <button key={key} role="radio" aria-checked={appearance.palette === key} onClick={() => appearance.setPalette(key as Palette)}><span style={{background:value.color}}/>{locale === 'zh-CN' ? value.zh : value.en}</button>)}</div></section>
    <section><label className="settings-line">{L('语言','Language')}<select aria-label={L('语言','Language')} value={locale} onChange={e => { if (e.target.value !== locale) toggleLocale(); }}><option value="zh-CN">简体中文</option><option value="en">English</option></select></label></section>
    <section><h3>{L('背景','Background')}</h3><div className="background-actions"><button disabled={appearance.busy} onClick={() => fileInput.current?.click()}><ImagePlus size={17}/>{L('选择图片','Choose image')}</button><button disabled={appearance.busy} onClick={() => void appearance.resetBackground()}><RotateCcw size={16}/>{L('恢复星空','Restore stars')}</button></div><input ref={fileInput} hidden type="file" accept="image/png,image/jpeg,image/webp" aria-label={L('背景图片','Background image')} onChange={e => { void appearance.chooseBackground(e.target.files?.[0] ?? null); e.currentTarget.value=''; }}/><label className="settings-line">{L('背景压暗','Dim background')}<input type="range" min="0" max=".85" step=".01" aria-label={L('背景压暗','Dim background')} value={appearance.dim} onChange={e => appearance.setDim(Number(e.target.value))}/></label>{appearance.error && <p role="alert">{appearance.error === 'background' ? L('请选择 20 MB 以内、6000 万像素以内的 JPG、PNG 或 WebP 图片。','Choose a JPG, PNG or WebP under 20 MB and 60 megapixels.') : L('背景无法保存，请重试。','Could not save the background. Try again.')}</p>}</section>
    <section><label className="settings-line">{L('界面大小','Interface size')}<select aria-label={L('界面大小','Interface size')} value={scale} onChange={e => onScale(Number(e.target.value))}>{[80,90,100,110,120,130,140].map(n=><option key={n} value={n}>{n}%</option>)}</select></label></section>
  </dialog>;
}
