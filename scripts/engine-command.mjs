// Modified for IA'GS (2026-09-24); see docs/CHANGES_FROM_UPSTREAM.md.
// IA'GS modification: this distribution supports macOS arm64 only.
import { spawnSync } from 'node:child_process';
const action=process.argv[2];
if (!['setup','verify'].includes(action)) {console.error('Usage: engine-command.mjs setup|verify');process.exit(2);}
if(process.platform!=='darwin'||process.arch!=='arm64'){console.error('IA\'GS requires Apple Silicon macOS 15+.');process.exit(1);}
const result=spawnSync('bash',[`scripts/${action}-engines-macos.sh`],{stdio:'inherit'});
if(result.error)console.error(result.error.message);
process.exit(result.status??1);
