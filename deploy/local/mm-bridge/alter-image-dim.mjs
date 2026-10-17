// alter-image-dim.mjs — 把 content_chunks.embedding_image 從 vector(1024) 改成
// vector(768)（EmbeddingGemma 2 維度）。安全前提：欄位必須全 NULL（無任何圖片
// 向量已存）。重用 gbrain 自己的嵌入式 PGlite 資產（同版本 WASM＋pgvector extension），
// 避免用不同版本的 pglite 開 gbrain 的資料目錄。
// 執行前確認沒有任何 gbrain 行程在跑，且 ~/.gbrain/brain.pglite 已備份。
import { PGlite } from '@electric-sql/pglite';

const DATA_DIR = process.env.PGLITE_DIR ?? 'C:/Users/charl/.gbrain/brain.pglite';
const TARGET = Number(process.env.TARGET_DIM ?? 768);
const GBRAIN_SRC =
  'file:///C:/Users/charl/.bun/install/global/node_modules/gbrain/src/core/pglite-embedded-assets.ts';

const { getEmbeddedPgliteOptions } = await import(GBRAIN_SRC);
const opts = await getEmbeddedPgliteOptions();
const db = await PGlite.create(DATA_DIR, opts);

const stat = await db.query(`
  SELECT atttypmod AS dim,
         (SELECT count(*) FROM content_chunks WHERE embedding_image IS NOT NULL) AS nonnull
  FROM pg_attribute
  WHERE attrelid = 'content_chunks'::regclass AND attname = 'embedding_image'
`);
const { dim, nonnull } = stat.rows[0];
console.log(`embedding_image 目前: vector(${dim}), 非空列數: ${nonnull}`);
if (Number(nonnull) !== 0) {
  console.error('ABORT: 欄位有資料，不可直接 ALTER。');
  process.exit(1);
}
if (Number(dim) === TARGET) {
  console.log(`已是 vector(${TARGET})，免改。`);
  await db.close();
  process.exit(0);
}

await db.exec('DROP INDEX IF EXISTS idx_chunks_embedding_image;');
await db.exec(`ALTER TABLE content_chunks ALTER COLUMN embedding_image TYPE vector(${TARGET}) USING NULL::vector(${TARGET});`);
await db.exec(`CREATE INDEX idx_chunks_embedding_image
  ON content_chunks USING hnsw (embedding_image vector_cosine_ops)
  WHERE embedding_image IS NOT NULL;`);

const after = await db.query(`
  SELECT atttypmod AS dim FROM pg_attribute
  WHERE attrelid = 'content_chunks'::regclass AND attname = 'embedding_image'
`);
console.log(`改後: vector(${after.rows[0].dim})`);
await db.close();
