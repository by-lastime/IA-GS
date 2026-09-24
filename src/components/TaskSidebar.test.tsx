// @vitest-environment jsdom
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { expect, it, vi } from 'vitest';
import { TaskSidebar } from './TaskSidebar';
import { LanguageProvider } from '../i18n';
import type { ProjectSummary } from '../types/pipeline';

it('limits recent history, expands it, searches collapsed tasks by name and navigates without leaking paths', async () => {
  (globalThis as typeof globalThis & {IS_REACT_ACT_ENVIRONMENT:boolean}).IS_REACT_ACT_ENVIRONMENT=true;
  localStorage.setItem('iags-language','zh-CN');
  const host=document.createElement('div');document.body.append(host);const root=createRoot(host);const onSelect=vi.fn();
  const projects=Array.from({length:9},(_,i)=>({id:String(i),name:i===0?'最早的玉器':`任务 ${i}`,createdAt:new Date(2026,8,i+1).toISOString(),status:'completed',projectPath:`/private/photos/${i}`} as ProjectSummary));
  await act(async()=>root.render(<LanguageProvider><TaskSidebar projects={projects} selected="draft" draftName="当前草稿" busy={false} onNew={()=>{}} onSelect={onSelect} onSettings={()=>{}}/></LanguageProvider>));
  expect(host.querySelectorAll('.task-nav-item:not(.draft)')).toHaveLength(5);
  expect(host.textContent).not.toContain('最早的玉器');
  await act(async()=>host.querySelector<HTMLButtonElement>('.more-tasks')!.click());
  expect(host.querySelectorAll('.task-nav-item:not(.draft)')).toHaveLength(9);
  await act(async()=>host.querySelector<HTMLButtonElement>('.more-tasks')!.click());
  await act(async()=>{const input=host.querySelector('input')!;Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value')!.set!.call(input,'玉器');input.dispatchEvent(new Event('input',{bubbles:true}));});
  expect(host.querySelectorAll('.task-nav-item')).toHaveLength(1);
  await act(async()=>host.querySelector<HTMLButtonElement>('.task-nav-item')!.click());
  expect(onSelect).toHaveBeenCalledWith('0');
  expect(host.innerHTML).not.toContain('/private/photos');
  await act(async()=>root.unmount());host.remove();localStorage.clear();
});
