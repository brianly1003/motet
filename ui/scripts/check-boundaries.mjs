import {readdirSync,readFileSync} from 'node:fs';
import {join} from 'node:path';
function walk(dir){return readdirSync(dir,{withFileTypes:true}).flatMap(e=>e.isDirectory()?walk(join(dir,e.name)):[join(dir,e.name)]);}
for(const path of [...walk('app'),...walk('lib')]){
 const source=readFileSync(path,'utf8');
 if(path!=='lib/ipc.ts'&&source.includes('@tauri-apps/'))throw Error(`Tauri bridge outside ipc.ts: ${path}`);
 if(/next\/(headers|server)|next\/font\/google|['"]use server['"]/.test(source))throw Error(`Server or remote font API: ${path}`);
}
