// IA'GS task navigation: a bounded recent list, all-name search, and a retained draft.
import { useState } from 'react';
import { ChevronDown, Plus, Search, Settings2, RotateCcw } from 'lucide-react';
import type { ProjectSummary } from '../types/pipeline';
import { useI18n } from '../i18n';
export function TaskSidebar({ projects, selected, draftName, busy, onNew, onSelect, onSettings, onRefresh }: {projects: ProjectSummary[];selected:string;draftName:string;busy:boolean;onNew:()=>void;onSelect:(id:string)=>void;onSettings:()=>void;onRefresh?:()=>void}) {
  const {locale} = useI18n();const L=(zh:string,en:string)=>locale==='zh-CN'?zh:en;
  const [query,setQuery]=useState('');const [expanded,setExpanded]=useState(false);
  const sorted=[...projects].sort((a,b)=> Date.parse(b.createdAt)-Date.parse(a.createdAt));
  const matches=sorted.filter(p=>p.name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  const visible=query.trim() || expanded ? matches : matches.slice(0,5);
  const status={completed:L('已完成','Completed'),running:L('运行中','Running'),failed:L('失败','Failed'),cancelled:L('已取消','Cancelled'),interrupted:L('已中断','Interrupted')};
  return <aside className="task-sidebar" aria-label={L('任务导航','Task navigation')}>
    <div className="studio-wordmark">IA’GS</div>
    <button className="new-task-button" disabled={busy} onClick={onNew}><Plus size={18}/>{L('新建任务','New task')}</button>
    <label className="task-search"><Search size={16}/><input aria-label={L('搜索任务','Search tasks')} placeholder={L('搜索任务','Search tasks')} value={query} onChange={e=>setQuery(e.target.value)}/></label>
    <nav aria-label={L('历史任务','Task history')}>
      {draftName && (!query.trim() || draftName.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())) && <button className={`task-nav-item draft ${selected==='draft'?'selected':''}`} onClick={()=>onSelect('draft')}><span className={`task-dot ${busy?'running':'draft'}`}/><span>{draftName}</span><small>{L('当前','Current')}</small></button>}
      {visible.map(project=><button key={project.id} className={`task-nav-item ${selected===project.id?'selected':''}`} aria-current={selected===project.id?'page':undefined} onClick={()=>onSelect(project.id)} title={project.name}><span className={`task-dot ${project.status}`} aria-label={status[project.status]}/><span>{project.name}</span></button>)}
      {query.trim() && matches.length===0 && <p className="search-empty">{L('没有找到任务','No tasks found')}</p>}
      {!query.trim() && sorted.length>5 && <button className="more-tasks" aria-expanded={expanded} onClick={()=>setExpanded(v=>!v)}><ChevronDown size={16} style={{transform:expanded?'rotate(180deg)':undefined}}/>{expanded?L('收起任务','Show less'):L(`更多任务（${sorted.length-5}）`,`More tasks (${sorted.length-5})`)}</button>}
    </nav>
    <div className="sidebar-footer"><button className="sidebar-settings" onClick={onSettings}><Settings2 size={17}/>{L('设置','Settings')}</button>{onRefresh && <button className="sidebar-refresh" disabled={busy} onClick={onRefresh} aria-label={L('刷新任务','Refresh tasks')} title={L('刷新任务','Refresh tasks')}><RotateCcw size={15}/></button>}</div>
  </aside>;
}
