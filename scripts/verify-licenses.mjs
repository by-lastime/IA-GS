// Modified for IA'GS (2026-09-24); see docs/CHANGES_FROM_UPSTREAM.md.
// IA'GS license verification: preserve upstream notices; use the fork's own identity.
import {readFileSync,existsSync} from 'node:fs';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
const read=p=>readFileSync(new URL('../'+p,import.meta.url),'utf8');
const license=read('LICENSE').replace(/\r\n?/g,'\n').replace(/\n+$/g,'');
assert.equal(createHash('sha256').update(license).digest('hex').toUpperCase(),'58D1E17FFE5109A7AE296CAAFCADFDBE6A7D176F0BC4AB01E12A689B0499D8BD');
assert(read('NOTICE').includes('Copyright 2026 ooolabdev'));
assert(read('NOTICE').includes("IA'GS"));
assert(read('TRADEMARK_POLICY.md').includes('does not grant a license'));
assert.equal(JSON.parse(read('package.json')).license,'Apache-2.0');
assert(read('src-tauri/Cargo.toml').includes('license = "Apache-2.0"'));
const config=JSON.parse(read('src-tauri/tauri.conf.json'));
assert.equal(config.identifier,'app.iags.desktop');
assert.equal(config.bundle.license,'Apache-2.0');
for(const file of ['THIRD_PARTY_NOTICES.txt','FFmpeg-LGPL-2.1.txt','COLMAP-LICENSE.txt','Brush-LICENSE.txt','PlayCanvas-MIT.txt','Mediabunny-MPL-2.0.txt']){
 assert(existsSync(new URL('../licenses/'+file,import.meta.url)));
 assert(config.bundle.resources['../licenses/'+file]);
}
assert.equal(read('config/telemetry-endpoint.txt').trim(),'');
assert(!read('src-tauri/Cargo.toml').includes('reqwest ='));
console.log("IA'GS license, attribution, identity and no-telemetry checks passed.");
