#!/usr/bin/env bun
/**
 * mm-bridge.mjs — 形狀轉譯 shim：gbrain 的 OpenAI-compat multimodal embeddings
 * （litellm recipe）→ llama.cpp llama-server 的 /v1/embeddings。
 *
 * gbrain 送：{ model, input: [ {type:'input_text',text} | {type:'image_url',
 *            image_url:{url}} | "plain string" ], input_type? }
 * llama.cpp（--mmproj 啟動）收：{ model, input: [ {content:[{type:'text',text} |
 *            {type:'image_url',image_url:{url}}] }, ... ] }（每元素一個 embedding）
 *
 * 其餘路徑（/v1/models、/health…）原樣透傳。監聽 127.0.0.1:8090 → 上游 127.0.0.1:8080。
 * 啟動：bun mm-bridge.mjs   （上游/埠可用環境變數 UPSTREAM / PORT 覆寫）
 */
import http from 'node:http';

const UPSTREAM = process.env.UPSTREAM ?? 'http://127.0.0.1:8080';
const PORT = Number(process.env.PORT ?? 8090);

function translateInput(el) {
  if (typeof el === 'string') {
    return { content: [{ type: 'text', text: el }] };
  }
  if (Array.isArray(el)) {
    return { content: el.map(translatePart) };
  }
  if (el && typeof el === 'object') {
    // gbrain 的元素層：{type:'input_text',text} / {type:'image_url',...} → 包進 content
    if (el.type === 'input_text' && typeof el.text === 'string') {
      return { content: [{ type: 'text', text: el.text }] };
    }
    if (el.content && Array.isArray(el.content)) {
      return { content: el.content.map(translatePart) }; // 已是 llama.cpp 形狀
    }
    return { content: [translatePart(el)] };
  }
  return el;
}

function translatePart(p) {
  if (typeof p === 'string') return { type: 'text', text: p };
  if (p && p.type === 'input_text' && typeof p.text === 'string') {
    return { type: 'text', text: p.text };
  }
  return p; // image_url 等原樣透傳
}

const server = http.createServer(async (req, res) => {
  const chunks = [];
  for await (const c of req) chunks.push(c);
  const body = Buffer.concat(chunks);

  let outBody = body;
  if ((req.url === '/embeddings' || req.url === '/v1/embeddings') && body.length) {
    try {
      const j = JSON.parse(body.toString('utf8'));
      if (Array.isArray(j.input)) {
        j.input = j.input.map(translateInput);
        delete j.input_type; // llama.cpp 的 embeddings schema 不收此欄位
        outBody = Buffer.from(JSON.stringify(j));
      }
    } catch {
      // 非 JSON——原樣透傳，讓上游回錯
    }
  }

  const url = req.url; // 保留 /v1 前綴——llama.cpp 的 /v1/embeddings 才是 OAI 相容端點（回 {data:[...]}）

  const upstreamReq = http.request(
    `${UPSTREAM}${url}`,
    { method: req.method, headers: { ...req.headers, host: '127.0.0.1:8080', 'content-length': outBody.length } },
    (ur) => {
      // llama.cpp 對 content-array 輸入回巢狀 embedding（[[…]]）——攤平成單向量，
      // 並維持 OAI 的 {data:[…]} 外殼（/embeddings 非 OAI 端點回裸陣列，轉正）。
      const isEmb = ur.url === '/embeddings' || ur.url === '/v1/embeddings';
      if (!isEmb) { res.writeHead(ur.statusCode ?? 502, ur.headers); ur.pipe(res); return; }
      const rchunks = [];
      ur.on('data', (c) => rchunks.push(c));
      ur.on('end', () => {
        try {
          const j = JSON.parse(Buffer.concat(rchunks).toString('utf8'));
          const data = Array.isArray(j) ? j : (j.data ?? []);
          for (const item of data) {
            if (Array.isArray(item?.embedding) && Array.isArray(item.embedding[0])) {
              item.embedding = item.embedding.flat();
            }
          }
          const out = JSON.stringify(Array.isArray(j) ? { object: 'list', data } : j);
          res.writeHead(ur.statusCode ?? 502, { 'content-type': 'application/json' });
          res.end(out);
        } catch {
          res.writeHead(ur.statusCode ?? 502, ur.headers);
          res.end(Buffer.concat(rchunks));
        }
      });
    },
  );
  upstreamReq.on('error', (e) => {
    res.writeHead(502, { 'content-type': 'application/json' });
    res.end(JSON.stringify({ error: { message: `mm-bridge upstream error: ${e.message}` } }));
  });
  upstreamReq.end(outBody);
});

server.listen(PORT, '127.0.0.1', () => {
  console.log(`[mm-bridge] 127.0.0.1:${PORT} → ${UPSTREAM}（gbrain multimodal 形狀轉譯）`);
});
