-- pgvector provides the semantic index. Vectors are an index over canonical
-- data, never the canonical store itself.
CREATE EXTENSION IF NOT EXISTS vector;
