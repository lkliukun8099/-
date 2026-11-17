#!/usr/bin/env node
/**
 * 生成应用基础图标（1024x1024 纯色圆角橙底 + 白色"M"条纹），供 `tauri icon` 使用。
 * 纯 Node 实现 PNG 编码（zlib deflate），无外部依赖。
 */
const fs = require("fs");
const path = require("path");
const zlib = require("zlib");

const SIZE = 1024;
const BG = [234, 88, 12]; // 品牌橙 #ea580c
const FG = [255, 255, 255];

function crc32(buf) {
  let table = crc32.table;
  if (!table) {
    table = crc32.table = new Uint32Array(256).map((_, n) => {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      return c >>> 0;
    });
  }
  let c = 0xffffffff;
  for (const b of buf) c = table[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

// 像素数据：橙底 + 居中白色 "M"（简单条纹近似）
const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
const RADIUS = 200;
for (let y = 0; y < SIZE; y++) {
  const rowStart = y * (SIZE * 4 + 1);
  raw[rowStart] = 0; // filter: none
  for (let x = 0; x < SIZE; x++) {
    // 圆角矩形外裁为透明
    const inX = Math.min(x, SIZE - 1 - x);
    const inY = Math.min(y, SIZE - 1 - y);
    let inside = true;
    if (inX < RADIUS && inY < RADIUS) {
      const dx = RADIUS - inX;
      const dy = RADIUS - inY;
      inside = dx * dx + dy * dy <= RADIUS * RADIUS;
    }
    let [r, g, b, a] = inside ? [...BG, 255] : [0, 0, 0, 0];
    // "M" 条纹：三条竖带 + 顶部连接斜带（粗略）
    if (inside && y > 300 && y < 724) {
      const leftBar = x > 300 && x < 380;
      const rightBar = x > 644 && x < 724;
      const midSlope =
        y - 300 < 260 &&
        ((x > 440 && x < 520 && x - 440 < (y - 300) * 0.62) ||
          (x > 504 && x < 584 && 584 - x < (y - 300) * 0.62));
      if (leftBar || rightBar || midSlope) [r, g, b] = FG;
    }
    const off = rowStart + 1 + x * 4;
    raw[off] = r;
    raw[off + 1] = g;
    raw[off + 2] = b;
    raw[off + 3] = a;
  }
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // color type RGBA
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", zlib.deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

const out = path.join(__dirname, "..", "app-icon.png");
fs.writeFileSync(out, png);
console.log(`已生成 ${out} (${png.length} bytes)`);
