// Synthetic pixels only. Never imported by the shipped application.
import type { HeadAvatar, CapePreview } from '../../src/lib/backend';
export function fixtureSkin(index = 0): HeadAvatar {
  const pixels = Array(64 * 64 * 4).fill(0);
  const colors = [[61, 167, 181], [155, 112, 217], [205, 141, 72], [76, 139, 97]];
  const coat = colors[index % colors.length];
  const fill = (x: number, y: number, w: number, h: number, color: number[]) => {
    for (let v = y; v < y + h; v++) for (let u = x; u < x + w; u++) pixels.splice((v * 64 + u) * 4, 4, ...color);
  };
  fill(0, 0, 32, 16, [194, 153, 125, 255]);
  fill(0, 0, 32, 8, [47, 43, 53, 255]);
  fill(8, 8, 8, 2, [47, 43, 53, 255]); fill(24, 8, 8, 8, [47, 43, 53, 255]);
  fill(10, 11, 1, 1, [228, 240, 246, 255]); fill(13, 11, 1, 1, [228, 240, 246, 255]);
  fill(10, 12, 1, 1, [43, 65, 85, 255]); fill(13, 12, 1, 1, [43, 65, 85, 255]);
  fill(11, 14, 2, 1, [145, 89, 80, 255]);
  for (const [x,y] of [[16,16],[40,16],[32,48]]) fill(x,y, x===16?24:16,16,[...coat,255]);
  fill(20, 20, 8, 2, [225, 230, 227, 255]); fill(23,22,2,10,[38,56,66,255]);
  if(index>=4) fill(32,22,1+(index%8),1,[225,230,227,255]);
  fill(44, 28, 4, 4, [194,153,125,255]); fill(36,60,4,4,[194,153,125,255]);
  for (const [x,y] of [[0,16],[16,48]]) { fill(x,y,16,16,[49,53,72,255]); fill(x,y+13,16,3,[31,34,45,255]); }
  // Transparent outer layers, with a semitransparent hat detail.
  fill(40, 8, 8, 2, [...coat,210]); fill(40,10,1,4,[...coat,210]);
  const head: number[] = [];
  for (let y=8;y<16;y++) for(let x=8;x<16;x++) head.push(...pixels.slice((y*64+x)*4,(y*64+x)*4+4));
  return {rgba:head, skinRgba:pixels,skinHeight:64,model:index%2?'slim':'classic',sha256:String(index).padStart(64,'0')};
}
export function fixtureCape(index = 0): CapePreview {
  const rgba = Array(64*32*4).fill(0), color = [[65,174,186],[161,112,215],[213,150,73]][index%3];
  for(let y=1;y<17;y++) for(let x=1;x<22;x++) {
    const edge = x===1 || x===10 || x===12 || x===21 || y===1 || y===16;
    const emblem = y>=5 && y<=11 && ((x%11>=4 && x%11<=7) || y===8);
    rgba.splice((y*64+x)*4,4,...(edge?[31,43,54]:emblem?[228,233,238]:color),255);
  }
  return {width:64,height:32,rgba};
}
