// Developer-only workaround for locally renamed SDK headers. Never edits an SDK.
import fs from 'node:fs';
import path from 'node:path';
const [sdk,output]=process.argv.slice(2);
if(!sdk||!output)throw new Error('Usage: sdk-overlay.mjs SDK OUTPUT');
const roots=[];
for(const file of fs.readdirSync(sdk,{recursive:true,withFileTypes:true})){
 if(!file.isFile()||!file.name.endsWith('.h'))continue;
 const clean=file.name.replace(/( \d{2}\.\d{2}\.\d{2})+/g,'');
 if(clean===file.name)continue;
 const real=path.join(file.parentPath,file.name),virtual=path.join(file.parentPath,clean);
 if(fs.existsSync(virtual))continue;
 for(const name of new Set([virtual,virtual.replace(/\/Versions\/[A-Z]\//g,'/')]))roots.push({type:'file',name,'external-contents':real});
}
fs.writeFileSync(output,JSON.stringify({version:0,roots}));
if(roots.length)console.log(`Using ${roots.length} read-only SDK header aliases; system files remain untouched.`);
