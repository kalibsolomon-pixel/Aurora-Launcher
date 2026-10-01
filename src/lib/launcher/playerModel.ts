import type { HeadAvatar } from "../backend";

type Point = [number, number, number];
export type SkinFace = { points: Point[]; uv: [number, number, number, number]; mirror: boolean; shade: number };
export type PlayerModel = { pixels: number[]; height: number; faces: SkinFace[]; fallback: boolean };

/** Static textured cuboids, in Minecraft pixel units. No network, WebGL or animation. */
export function playerModel(avatar: HeadAvatar | null | undefined): PlayerModel {
  const valid = !!avatar && [32, 64].includes(avatar.skinHeight) && ["classic", "slim"].includes(avatar.model) &&
    avatar.skinRgba?.length === 64 * avatar.skinHeight * 4 && avatar.skinRgba.every(n => Number.isInteger(n) && n >= 0 && n <= 255);
  const pixels = valid ? avatar.skinRgba : defaultSkin();
  const height = valid ? avatar.skinHeight : 64;
  const modern = height === 64;
  const arm = valid && modern && avatar.model === "slim" ? 3 : 4;
  const faces: SkinFace[] = [];
  const radians = (degrees: number) => degrees * Math.PI / 180;
  function box(w: number, h: number, d: number, center: Point, u: number, v: number, pitch = 0, yaw = 0, mirror = false, grow = 0) {
    function point(x: number, y: number, z: number): Point {
      const a = radians(pitch), b = radians(yaw);
      const py = y * Math.cos(a) - z * Math.sin(a), pz = y * Math.sin(a) + z * Math.cos(a);
      const px = x * Math.cos(b) + pz * Math.sin(b), pzz = -x * Math.sin(b) + pz * Math.cos(b);
      return [px + center[0], py + center[1], pzz + center[2]];
    }
    const x = w / 2 + grow, z = d / 2 + grow, top = -grow, bottom = h + grow;
    const q = (points: Point[], uv: SkinFace["uv"], shade: number) => faces.push({ points, uv, mirror, shade });
    q([point(-x, top, -z), point(x, top, -z), point(x, bottom, -z), point(-x, bottom, -z)], [u+d,v+d,w,h], 1);
    q([point(x, top, z), point(-x, top, z), point(-x, bottom, z), point(x, bottom, z)], [u+d+w+d,v+d,w,h], .72);
    q([point(-x, top, z), point(-x, top, -z), point(-x, bottom, -z), point(-x, bottom, z)], [mirror ? u+d+w : u,v+d,d,h], .78);
    q([point(x, top, -z), point(x, top, z), point(x, bottom, z), point(x, bottom, -z)], [mirror ? u : u+d+w,v+d,d,h], .88);
    q([point(-x, top, z), point(x, top, z), point(x, top, -z), point(-x, top, -z)], [u+d,v,w,d], 1.04);
    q([point(-x, bottom, -z), point(x, bottom, -z), point(x, bottom, z), point(-x, bottom, z)], [u+d+w,v,w,d], .65);
  }
  box(8,8,8,[0,-16,0],0,0,0,-8);
  box(8,8,8,[0,-16,0],32,0,0,-8,false,.45);
  box(8,12,4,[0,-8,0],16,16);
  box(arm,12,4,[-4-arm/2,-8,0],40,16,10);
  box(arm,12,4,[4+arm/2,-8,0],modern?32:40,modern?48:16,-10,0,!modern);
  box(4,12,4,[-2,4,0],0,16,-8);
  box(4,12,4,[2,4,0],modern?16:0,modern?48:16,8,0,!modern);
  if (modern) {
    box(8,12,4,[0,-8,0],16,32,0,0,false,.25);
    box(arm,12,4,[-4-arm/2,-8,0],40,32,10,0,false,.25);
    box(arm,12,4,[4+arm/2,-8,0],48,48,-10,0,false,.25);
    box(4,12,4,[-2,4,0],0,32,-8,0,false,.25);
    box(4,12,4,[2,4,0],0,48,8,0,false,.25);
  }
  return { pixels, height, faces, fallback: !valid };
}

function defaultSkin(): number[] {
  const pixels = Array<number>(64*64*4).fill(0);
  const fill = (u: number,v: number,w: number,h: number,color: number[]) => {
    for(let y=v;y<v+h;y++) for(let x=u;x<u+w;x++) pixels.splice((y*64+x)*4,4,...color,255);
  };
  fill(0,0,32,16,[165,121,95]); fill(8,0,16,4,[62,43,33]);
  fill(16,16,24,16,[58,94,107]); fill(40,16,16,16,[165,121,95]); fill(32,48,16,16,[165,121,95]);
  fill(0,16,16,16,[49,55,82]); fill(16,48,16,16,[49,55,82]);
  fill(10,10,1,1,[221,221,228]); fill(13,10,1,1,[221,221,228]);
  return pixels;
}

/** Rendered model bounds shared by the Home preview layout. */
export const PLAYER_RENDER_WIDTH = 560;
export const PLAYER_RENDER_HEIGHT = 800;

/** Painter's-order rasterization of visible faces; nearest texels preserve skin detail. */
export function renderPlayer(context: CanvasRenderingContext2D, model: PlayerModel, yaw = -.42): void {
  const width = PLAYER_RENDER_WIDTH, height = PLAYER_RENDER_HEIGHT;
  const image = context.createImageData(width,height);
  const tilt = .10;
  function project([x,y,z]: Point) {
    const px=x*Math.cos(yaw)+z*Math.sin(yaw), pz=-x*Math.sin(yaw)+z*Math.cos(yaw);
    const py=y*Math.cos(tilt)-pz*Math.sin(tilt), depth=y*Math.sin(tilt)+pz*Math.cos(tilt);
    const scale=20/(1+depth/120);
    return { x:width/2+px*scale,y:height/2+py*scale,z:depth };
  }
  const visible = model.faces.map(face=>({face,p:face.points.map(project)})).filter(({p})=>
    (p[1].x-p[0].x)*(p[2].y-p[0].y)-(p[1].y-p[0].y)*(p[2].x-p[0].x)>0);
  visible.sort((a,b)=>b.p.reduce((s,p)=>s+p.z,0)-a.p.reduce((s,p)=>s+p.z,0));
  for(const {face,p} of visible) {
    const [u,v,w,h]=face.uv;
    const uv = [[0,0],[1,0],[1,1],[0,1]];
    for(const indices of [[0,1,2],[0,2,3]]) {
      const [a,b,c]=indices.map(i=>p[i]);
      const denominator=(b.y-c.y)*(a.x-c.x)+(c.x-b.x)*(a.y-c.y);
      if(Math.abs(denominator)<.001) continue;
      const minX=Math.max(0,Math.floor(Math.min(a.x,b.x,c.x))), maxX=Math.min(width-1,Math.ceil(Math.max(a.x,b.x,c.x)));
      const minY=Math.max(0,Math.floor(Math.min(a.y,b.y,c.y))), maxY=Math.min(height-1,Math.ceil(Math.max(a.y,b.y,c.y)));
      for(let y=minY;y<=maxY;y++) for(let x=minX;x<=maxX;x++) {
        const l=((b.y-c.y)*(x+.5-c.x)+(c.x-b.x)*(y+.5-c.y))/denominator;
        const m=((c.y-a.y)*(x+.5-c.x)+(a.x-c.x)*(y+.5-c.y))/denominator, n=1-l-m;
        if(l<0||m<0||n<0) continue;
        let tx=l*uv[indices[0]][0]+m*uv[indices[1]][0]+n*uv[indices[2]][0];
        if(face.mirror) tx=1-tx;
        const ty=l*uv[indices[0]][1]+m*uv[indices[1]][1]+n*uv[indices[2]][1];
        const source=((v+Math.min(h-1,Math.floor(ty*h)))*64+u+Math.min(w-1,Math.floor(tx*w)))*4;
        const alpha=model.pixels[source+3]/255;
        if(!alpha) continue;
        const dest=(y*width+x)*4, back=image.data[dest+3]/255, out=alpha+back*(1-alpha);
        for(let channel=0;channel<3;channel++) image.data[dest+channel]=(Math.min(255,model.pixels[source+channel]*face.shade)*alpha+image.data[dest+channel]*back*(1-alpha))/out;
        image.data[dest+3]=out*255;
      }
    }
  }
  context.putImageData(image,0,0);
}
