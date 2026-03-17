-- Enable pgvector extension for vector similarity search
CREATE EXTENSION IF NOT EXISTS vector;

-- Enable pgcrypto for gen_random_uuid() used by later migrations
CREATE EXTENSION IF NOT EXISTS pgcrypto;
