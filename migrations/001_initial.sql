-- Initial database schema
-- TODO: Implement in TASK-006

-- Processed messages for deduplication
CREATE TABLE processed_messages (
    message_hash TEXT PRIMARY KEY,
    project_name TEXT,
    session_id TEXT,
    processed_at TEXT
);

-- Currency exchange rates cache
CREATE TABLE exchange_rates (
    base_currency TEXT,
    target_currency TEXT,
    rate REAL,
    fetched_at TEXT,
    PRIMARY KEY (base_currency, target_currency)
);

-- Model pricing data
CREATE TABLE model_pricing (
    model_name TEXT PRIMARY KEY,
    input_cost_per_mtok REAL,
    output_cost_per_mtok REAL,
    cache_cost_per_mtok REAL,
    last_updated TEXT
);

-- Usage analytics (aggregated)
CREATE TABLE usage_summary (
    date TEXT,
    project_name TEXT,
    model_name TEXT,
    total_input_tokens INTEGER,
    total_output_tokens INTEGER,
    total_cache_tokens INTEGER,
    total_cost_usd REAL,
    PRIMARY KEY (date, project_name, model_name)
);