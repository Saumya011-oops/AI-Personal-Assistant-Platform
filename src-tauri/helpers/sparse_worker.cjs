#!/usr/bin/env node

const http = require('node:http');
const elasticlunr = require('elasticlunr');

const port = Number(process.env.SPARSE_HELPER_PORT || process.argv[2] || 8741);

let docsById = new Map();
let index = buildIndex();

function buildIndex() {
  return elasticlunr(function () {
    this.setRef('chunkId');
    this.addField('title');
    this.addField('content');
    this.addField('tags');
    this.addField('author');
    this.addField('category');
    this.addField('source');
    this.saveDocument(false);
  });
}

function normalizeDocument(document) {
  return {
    chunkId: document.chunkId,
    documentId: document.documentId,
    ordinal: document.ordinal,
    source: document.sourceKind || document.source,
    title: document.title || '',
    content: document.content || '',
    tags: Array.isArray(document.tags) ? document.tags.join(' ') : '',
    author: document.author || '',
    category: document.category || '',
    createdAt: document.createdAt || null,
    updatedAt: document.updatedAt || null,
    metadata: document.metadata || {},
  };
}

function rebuild(documents) {
  docsById = new Map();
  index = buildIndex();
  for (const rawDocument of documents || []) {
    const document = normalizeDocument(rawDocument);
    docsById.set(document.chunkId, document);
    index.addDoc(document);
  }
}

function upsert(documents) {
  const merged = new Map(docsById);
  for (const rawDocument of documents || []) {
    const document = normalizeDocument(rawDocument);
    merged.set(document.chunkId, document);
  }
  rebuild(Array.from(merged.values()));
}

function matchesFilters(document, filters) {
  if (!filters) return true;

  if (filters.author?.length) {
    const author = String(document.author || '').toLowerCase();
    if (!filters.author.some((item) => author.includes(String(item).toLowerCase()))) return false;
  }

  if (filters.dateRange) {
    const candidate = document.updatedAt || document.createdAt || '';
    if (filters.dateRange.from && candidate < filters.dateRange.from) return false;
    if (filters.dateRange.to && candidate > filters.dateRange.to) return false;
  }

  return true;
}

async function readJson(req) {
  return await new Promise((resolve, reject) => {
    let body = '';
    req.on('data', (chunk) => {
      body += chunk;
    });
    req.on('end', () => {
      try {
        resolve(body ? JSON.parse(body) : {});
      } catch (error) {
        reject(error);
      }
    });
    req.on('error', reject);
  });
}

function writeJson(res, statusCode, payload) {
  res.writeHead(statusCode, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(payload));
}

const server = http.createServer(async (req, res) => {
  try {
    if (req.method === 'GET' && req.url === '/health') {
      return writeJson(res, 200, { ready: true, documents: docsById.size });
    }

    if (req.method !== 'POST') {
      return writeJson(res, 404, { error: 'Not found' });
    }

    const payload = await readJson(req);

    if (req.url === '/rebuild') {
      rebuild(payload.documents || []);
      return writeJson(res, 200, { ok: true, documents: docsById.size });
    }

    if (req.url === '/upsert') {
      upsert(payload.documents || []);
      return writeJson(res, 200, { ok: true, documents: docsById.size });
    }

    if (req.url === '/clear') {
      rebuild([]);
      return writeJson(res, 200, { ok: true });
    }

    if (req.url === '/search') {
      const query = String(payload.query || '').trim();
      const limit = Number(payload.limit || 20);
      if (!query) {
        return writeJson(res, 200, { results: [] });
      }

      const rawResults = index.search(query, {
        fields: {
          title: { boost: 2 },
          content: { boost: 3 },
          tags: { boost: 2 },
          author: { boost: 1.5 },
          category: { boost: 1.5 },
          source: { boost: 1 },
        },
        bool: 'OR',
        expand: true,
      });

      const filtered = rawResults
        .map((result) => ({
          chunkId: result.ref,
          score: result.score,
          document: docsById.get(result.ref),
        }))
        .filter((result) => result.document)
        .filter((result) => matchesFilters(result.document, payload.filters))
        .slice(0, limit)
        .map(({ chunkId, score }) => ({ chunkId, score }));

      return writeJson(res, 200, { results: filtered });
    }

    return writeJson(res, 404, { error: 'Not found' });
  } catch (error) {
    return writeJson(res, 500, { error: String(error?.message || error) });
  }
});

server.listen(port, '127.0.0.1');
