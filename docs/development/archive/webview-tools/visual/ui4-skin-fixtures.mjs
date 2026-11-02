import {mkdir,writeFile} from 'node:fs/promises';
import {deflateSync} from 'node:zlib';
import {fixtureSkin} from './l3-fixtures.ts';
const out='docs/ui-4/fixtures';await mkdir(out,{recursive:true});
function crc(bytes){let c=0xffffffff;for(const b of bytes){c^=b;for(let i=0;i<8;i++)c=c&1?(c>>>1)^0xedb88320:c>>>1;}return(c^0xffffffff)>>>0;}
function chunk(kind,bytes){const type=Buffer.from(kind),n=Buffer.alloc(4),checksum=Buffer.alloc(4);n.writeUInt32BE(bytes.length);checksum.writeUInt32BE(crc(Buffer.concat([type,bytes])));return Buffer.concat([n,type,bytes,checksum]);}
function png(index,height=64){const skin=fixtureSkin(index),header=Buffer.alloc(13);header.writeUInt32BE(64);header.writeUInt32BE(height,4);header[8]=8;header[9]=6;const rows=[];for(let y=0;y<height;y++)rows.push(Buffer.from([0]),Buffer.from(skin.skinRgba.slice(y*256,(y+1)*256)));return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',header),chunk('IDAT',deflateSync(Buffer.concat(rows))),chunk('IEND',Buffer.alloc(0))]);}
await writeFile(`${out}/Aurora-local-classic.png`,png(0));await writeFile(`${out}/Aurora-local-slim.png`,png(1));await writeFile(`${out}/Aurora-local-legacy.png`,png(2,32));await writeFile(`${out}/Invalid-skin.png`,Buffer.from('This is not a PNG. Synthetic rejection fixture.'));
console.log('Created synthetic PNG test files; no owner artwork.');
