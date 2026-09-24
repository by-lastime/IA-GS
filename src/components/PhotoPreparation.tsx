// IA'GS local photo preparation and mask review.
import { useEffect, useRef, useState, type PointerEvent } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { FolderOpen, Images, LoaderCircle, X } from 'lucide-react';
import './PhotoPreparation.css';
import { useI18n } from '../i18n';
export interface Photo { id: string; originalName: string; width: number; height: number; cameraKey: string; focal35?: number; digitalZoom?: number; instances: number; coverage: number; warnings: string[]; enabled: boolean; reviewed: boolean; }
export interface PhotoSession { root: string; manifest: { name: string; complete: boolean; photos: Photo[]; } }
interface Stroke { erase: boolean; radius: number; points: [number, number][]; }
interface Progress { current: number; total: number; message: string; }
export function PhotoPreparation({ taskName, projectsRoot, disabled, onReady, onInvalidate, onBusy }: { taskName: string; projectsRoot: string; disabled: boolean; onReady: (path: string) => Promise<void>; onInvalidate: () => void; onBusy: (value: boolean) => void; }) {
  const { locale } = useI18n();
  const L=(zh:string,en:string)=>locale==='zh-CN'?zh:en;
  const warningText = (warning: string) => {
    if (locale === 'zh-CN') return warning;
    const translations: Record<string,string> = {
      '中心不是主体，已选择最大前景；请检查':'The center is not the subject. The largest foreground was selected; please review.',
      '检测到多个主体，可点击重新选择':'Multiple subjects found. Click to select the intended subject.',
      '前景占比异常，请检查遮罩':'Unusual foreground coverage; review the mask.',
      '缺少可靠焦距信息：使用独立相机，由重建估计':'No reliable focal length. Camera intrinsics will be estimated independently.',
      '快门较慢，请检查运动模糊':'Slow shutter; check for motion blur.',
      '与之前照片完全重复，建议排除':'Exact duplicate of an earlier photo; consider excluding it.',
      '清晰度或纹理偏低，请检查；该提示不会自动排除照片':'Low sharpness or texture; review this photo. It has not been excluded.',
    };
    return translations[warning] ?? (warning.startsWith('自动抠图失败') ? 'Automatic masking failed. Correct the mask or exclude this photo.' : warning);
  };
  const [sourcePath,setSourcePath]=useState('');
  const [session, setSession] = useState<PhotoSession | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [progress, setProgress] = useState<Progress | null>(null);
  const [index, setIndex] = useState(0);
  const [editor, setEditor] = useState(false);
  const [mode, setMode] = useState<'select' | 'keep' | 'erase'>('erase');
  const [radius, setRadius] = useState(.018);
  const [strokes, setStrokes] = useState<Stroke[]>([]);
  const [revision, setRevision] = useState(0);
  const [approved, setApproved] = useState(false);
  const [ready, setReady] = useState(false);
  const [showOriginal, setShowOriginal] = useState(false);
  const canvas = useRef<HTMLCanvasElement>(null);
  const stroke = useRef<Stroke | null>(null);
  const photo = session?.manifest.photos[index];
  const groups = session ? new Set(session.manifest.photos.filter(p=>p.enabled).map(p=>p.cameraKey)).size : 0;
  const asset = (sub: string) => session && photo ? `${convertFileSrc(`${session.root}/${sub}/${photo.id}.png`)}?v=${revision}` : '';
  useEffect(() => {
    if (!('__TAURI_INTERNALS__' in window)) return;
    let disposed = false; let unlisten: (()=>void) | undefined;
    void listen<Progress>('photo-progress', ({payload})=>setProgress(payload)).then(fn=>{if(disposed)fn();else unlisten=fn;}).catch(e=>setError(String(e)));
    return ()=>{disposed=true;unlisten?.();};
  }, []);
  useEffect(()=>{onBusy(busy);},[busy,onBusy]);
  useEffect(()=>{
    const c=canvas.current; if(!c || !photo)return; const ctx=c.getContext('2d'); if(!ctx)return;
    ctx.clearRect(0,0,c.width,c.height);
    for(const s of strokes)draw(ctx,s,c.width,c.height);
  },[strokes,photo,editor]);
  const invalidate = ()=>{setReady(false);setApproved(false);onInvalidate();};
  async function perform<T>(operation: ()=>Promise<T>): Promise<T|undefined> {
    setBusy(true);setError('');
    try{return await operation();}catch(e){setError(String(e));return undefined;}finally{setBusy(false);}
  }
  async function importPhotos(existing=false) {
    const selected=await open({directory:true,multiple:false,title:existing?L('打开照片项目','Open prepared photos'):L('选择 JPG / JPEG / PNG 照片文件夹','Choose a JPG / JPEG / PNG photo folder')});
    if(typeof selected!=='string')return;
    setSourcePath(selected);
    invalidate();setProgress(null);setStrokes([]);setSession(null);
    const result=await perform(()=>invoke<PhotoSession>(existing?'open_preparation':'prepare_photos',existing?{root:selected}:{input:selected,projectsRoot,taskName}));
    if(result){setSession(result);setIndex(0);setRevision(v=>v+1);}
  }
  async function editPhoto(options: {select?:[number,number];enabled?:boolean;reviewed?:boolean}={}) {
    if(!session||!photo)return;
    invalidate();
    const result=await perform(()=>invoke<PhotoSession>('edit_photo',{root:session.root,edit:{id:photo.id,strokes,select:options.select??null,enabled:options.enabled??photo.enabled,reviewed:options.reviewed??true}}));
    if(result){setSession(result);setStrokes([]);setRevision(v=>v+1);}
  }
  async function finalize(){if(!session)return;const path=await perform(()=>invoke<string>('finalize_photos',{root:session.root,approveAll:approved}));if(path){const result=await perform(async()=>{await onReady(path);return true;});if(result)setReady(true);}}
  const position=(event:PointerEvent<HTMLCanvasElement>):[number,number]=>{const b=event.currentTarget.getBoundingClientRect();return [Math.max(0,Math.min(1,(event.clientX-b.left)/b.width)),Math.max(0,Math.min(1,(event.clientY-b.top)/b.height))];};
  function draw(ctx:CanvasRenderingContext2D,s:Stroke,w:number,h:number){ctx.strokeStyle=s.erase?'rgba(247,73,83,.55)':'rgba(41,192,132,.6)';ctx.lineWidth=s.radius*Math.max(w,h)*2;ctx.lineCap='round';ctx.lineJoin='round';ctx.beginPath();s.points.forEach((p,i)=>{if(i)ctx.lineTo(p[0]*w,p[1]*h);else{ctx.moveTo(p[0]*w,p[1]*h);ctx.lineTo(p[0]*w+.1,p[1]*h);}});ctx.stroke();}
  function down(event:PointerEvent<HTMLCanvasElement>){if(busy||disabled)return;const point=position(event);if(mode==='select'){if(strokes.length){setError('请先保存或撤销笔画，再重选主体');return;}void editPhoto({select:point});return;}event.currentTarget.setPointerCapture(event.pointerId);stroke.current={erase:mode==='erase',radius,points:[point]};const ctx=event.currentTarget.getContext('2d');if(ctx)draw(ctx,stroke.current,event.currentTarget.width,event.currentTarget.height);}
  function move(event:PointerEvent<HTMLCanvasElement>){if(!stroke.current||!event.currentTarget.hasPointerCapture(event.pointerId))return;const point=position(event);const last=stroke.current.points.at(-1)!;stroke.current.points.push(point);const ctx=event.currentTarget.getContext('2d');if(ctx)draw(ctx,{...stroke.current,points:[last,point]},event.currentTarget.width,event.currentTarget.height);}
  function up(event:PointerEvent<HTMLCanvasElement>){if(stroke.current){const finished=stroke.current;setStrokes(s=>[...s,finished]);stroke.current=null;}if(event.currentTarget.hasPointerCapture(event.pointerId))event.currentTarget.releasePointerCapture(event.pointerId);}
  const locked=disabled||busy;
  return <section className="photo-preparation" aria-label={L("照片准备","Photo preparation")}>
    <label className="field-label">{L("照片文件夹", "Photo folder")}</label>
    <div className="photo-import-actions"><button type="button" disabled={locked||!projectsRoot||!taskName} onClick={()=>void importPhotos().catch(e=>setError(String(e)))}><Images size={17}/>{L('导入照片并自动抠图','Choose photos & remove background')}</button><button type="button" disabled={locked} onClick={()=>void importPhotos(true).catch(e=>setError(String(e)))}><FolderOpen size={16}/>{L('打开照片项目','Open prepared photos')}</button></div>
    {sourcePath && <p className="source-folder-path">{sourcePath}</p>}
    {busy&&<div className="photo-busy" role="status"><LoaderCircle className="spin" size={17}/><span>{locale==='zh-CN' ? progress?.message??'正在处理照片…' : 'Processing photos…'}{progress&&` · ${progress.current}/${progress.total}`}</span><button type="button" onClick={()=>void invoke('cancel_preparation').catch(e=>setError(String(e)))}>{L('取消','Cancel')}</button></div>}
    {session&&<div className="photo-summary"><strong>{taskName || session.manifest.name}</strong><span>{session.manifest.photos.filter(p=>p.enabled).length} {L('张保留','photos')} · {groups} {L('组相机','cameras')}</span><button type="button" disabled={locked} onClick={()=>setEditor(true)}>{L('检查与修正遮罩','Review masks')}</button><label><input type="checkbox" checked={approved} disabled={locked} onChange={e=>setApproved(e.target.checked)}/>{L('我已检查保留照片的遮罩与视角','I have checked the masks and viewpoints')}</label><button type="button" disabled={locked||(!approved&&session.manifest.photos.some(p=>p.enabled&&!p.reviewed))} onClick={()=>void finalize()}>{ready?L('照片已就绪','Photos ready'):L('使用这些照片重建','Use these photos')}</button></div>}
    {error&&<p className="photo-error" role="alert">{error}</p>}
    {editor&&session&&photo&&<div className="photo-editor-backdrop" role="dialog" aria-modal="true" aria-label={L("检查与修正遮罩","Review masks")}>
      <section className="photo-editor"><header><div><strong>{photo.originalName}</strong><small>{index+1} / {session.manifest.photos.length} · {photo.width} × {photo.height} · {L('前景','Foreground')} {Math.round(photo.coverage*100)}%</small></div><button type="button" aria-label={L("关闭照片编辑","Close photo editor")} disabled={busy||strokes.length>0} onClick={()=>setEditor(false)}><X size={20}/></button></header>
        <div className="photo-editor-main"><div className="photo-checker"><div className="photo-image-wrap" style={{aspectRatio:`${photo.width}/${photo.height}`}}><img src={asset(showOriginal?'previews':'output')} alt={L("照片抠图预览","Foreground mask preview")} draggable={false}/><canvas ref={canvas} width={photo.width} height={photo.height} onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={up}/></div></div><aside>
          <label><input type="checkbox" checked={showOriginal} onChange={e=>setShowOriginal(e.target.checked)}/>{L('显示原图，便于补回','Show original to restore areas')}</label>
          <div className="photo-tools">{(['select','keep','erase'] as const).map(m=><button key={m} type="button" aria-pressed={mode===m} disabled={locked} onClick={()=>setMode(m)}>{({select:L('重选主体','Select subject'),keep:L('补回','Restore'),erase:L('擦除','Erase')})[m]}</button>)}</div>
          <label>{L('笔刷大小','Brush size')}<input type="range" min=".003" max=".10" step=".001" value={radius} onChange={e=>setRadius(Number(e.target.value))}/></label>
          <p>{mode==='select'?L('点在主体内部，重新生成遮罩。','Click inside the subject to select its mask.'):L('拖动画笔修改，保存后生效。','Drag to paint, then save your edits.')}</p>
          <button type="button" disabled={locked||strokes.length===0} onClick={()=>setStrokes([])}>{L('撤销未保存笔画','Discard unsaved strokes')}</button>
          <button type="button" disabled={locked} onClick={()=>void editPhoto()}>{L('保存并标记已检查','Save & mark reviewed')}</button>
          <label><input type="checkbox" checked={photo.enabled} disabled={locked||strokes.length>0} onChange={e=>void editPhoto({enabled:e.target.checked,reviewed:photo.reviewed})}/>{L('参与重建','Include in reconstruction')}</label>
          <p>{photo.focal35?`${L('等效焦距','Equivalent focal length')}: ${photo.focal35} mm`:L('焦距缺失，独立估计','Unknown focal length; estimated independently')}{photo.digitalZoom?` · ${photo.digitalZoom}×`:''}</p>
          {photo.warnings.map(w=><p className="photo-warning" key={w}>{warningText(w)}</p>)}
          <p>{L('细孔、反光和支架需人工检查。抠图无法修复模糊或被遮挡的结构。','Check holes, reflections and supports. Masks cannot repair blur or occluded geometry.')}</p>
          {busy&&<p role="status">{L('正在处理…','Processing…')}</p>}{error&&<p className="photo-error" role="alert">{error}</p>}
        </aside></div><footer><button type="button" disabled={locked||index===0||strokes.length>0} onClick={()=>setIndex(i=>i-1)}>{L('上一张','Previous')}</button><span>{photo.reviewed?L('已检查','Reviewed'):L('待检查','To review')}{photo.enabled?'':L(' · 已排除',' · Excluded')}</span><button type="button" disabled={locked||index===session.manifest.photos.length-1||strokes.length>0} onClick={()=>setIndex(i=>i+1)}>{L('下一张','Next')}</button></footer>
      </section>
    </div>}
  </section>;
}
