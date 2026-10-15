import sharp from 'sharp';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { execFileSync } from 'node:child_process';

const out = resolve('artifacts/store-media');
await mkdir(out, { recursive: true });
const icon = (x,y,s) => `<g transform="translate(${x} ${y}) scale(${s/256})"><rect width="256" height="256" rx="56" fill="#147b70"/><g fill="none" stroke="#fff" stroke-width="12" stroke-linecap="round"><path d="M76 70v116M110 70v76M144 70v116M178 70v76"/></g></g>`;
const svg = (w,h,body) => `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}"><rect width="100%" height="100%" fill="#f3f6f4"/>${body}</svg>`;
const type = (x,y,text,size=50,color='#143f39') => `<text x="${x}" y="${y}" fill="${color}" font-family="Segoe UI, sans-serif" font-size="${size}" font-weight="600">${text}</text>`;
const render = async (name,source) => {await writeFile(resolve(out,`${name}.svg`),source);await sharp(Buffer.from(source)).png().toFile(resolve(out,`${name}.png`));};
await render('snap-banner',svg(1440,480,`${icon(90,105,270)}${type(420,205,'tickreel.',105)}${type(425,290,'Make your day a reel.',44)}${type(425,355,'LOCAL RECORDING  /  CAPTIONS  /  MUSIC',23,'#55786f')}`));
await render('box-art',svg(1080,1080,`${icon(330,150,420)}${type(235,750,'tickreel.',145)}${type(215,850,'Make your day a reel.',53)}`));
await render('poster-art',svg(720,1080,`${icon(180,150,360)}${type(105,690,'tickreel.',115)}${type(100,800,'Make your day',60)}${type(210,880,'a reel.',65)}`));
// No text or application UI: suitable for Microsoft Store hero artwork.
let film='';
for(let i=0;i<7;i++){let x=180+i*235,y=150+Math.sin(i*.85)*70;film+=`<g transform="translate(${x} ${y}) rotate(${(i-3)*5})"><rect width="200" height="540" rx="28" fill="${i%2?'#147b70':'#d3e8db'}"/><rect x="22" y="70" width="156" height="360" rx="12" fill="${i%2?'#e0f2dc':'#76b6a4'}"/>`;for(let j=0;j<4;j++)film+=`<rect x="${25+j*43}" y="22" width="22" height="18" rx="4" fill="#f3f6f4"/><rect x="${25+j*43}" y="485" width="22" height="18" rx="4" fill="#f3f6f4"/>`;film+='</g>';}
await render('hero',svg(1920,1080,film+`<path d="M900 800l150 85-150 85z" fill="#147b70"/>`));
await render('trailer-background',svg(1920,1080,`${icon(860,120,130)}${type(1030,215,'tickreel.',100)}${type(860,400,'Make your day',74)}${type(860,495,'a reel.',110)}${type(865,640,'Record locally.',40)}${type(865,715,'Add captions, music and your voice.',34)}${type(865,790,'Export your creative process.',34)}${type(865,960,'tickreel.ycccompany.jp',32,'#55786f')}`));
const ff = resolve('node_modules/ffmpeg-static/ffmpeg.exe');
execFileSync(ff,['-y','-loop','1','-i',resolve(out,'trailer-background.png'),'-i',resolve('artifacts/x-promo-v2/Tickreel-X-9x16-narrated.mp4'),'-filter_complex','[1:v]scale=608:1080,setsar=1[v];[0:v][v]overlay=100:0:shortest=1[out]','-map','[out]','-map','1:a:0','-c:v','libx264','-profile:v','high','-pix_fmt','yuv420p','-r','30','-g','15','-bf','2','-b:v','12M','-c:a','aac','-b:a','384k','-ar','48000','-ac','2','-movflags','+faststart','-t','20',resolve(out,'tickreel-trailer-1080p.mp4')],{stdio:'ignore'});
execFileSync(ff,['-y','-ss','4','-i',resolve(out,'tickreel-trailer-1080p.mp4'),'-frames:v','1',resolve(out,'trailer-thumbnail.png')],{stdio:'ignore'});
const narration = JSON.parse(await readFile('artifacts/x-promo-v2/narration-script.json','utf8'));
const stamp = t => {const ms=Math.round(t*1000);return `${String(Math.floor(ms/3600000)).padStart(2,'0')}:${String(Math.floor(ms/60000)%60).padStart(2,'0')}:${String(Math.floor(ms/1000)%60).padStart(2,'0')}.${String(ms%1000).padStart(3,'0')}`;};
await writeFile(resolve(out,'trailer-ja.vtt'),'WEBVTT\n\n'+narration.map((r,i)=>`${i+1}\n${stamp(r.start)} --> ${stamp(r.end)}\n${r.text}`).join('\n\n')+'\n');
console.log(out);
