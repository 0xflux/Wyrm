-- Add migration script here
CREATE TABLE agent_staging (
    id SERIAL PRIMARY KEY,
    date_added TIMESTAMPTZ DEFAULT now(),
    agent_name TEXT NOT NULL,
    host TEXT NOT NULL,
    c2_endpoint TEXT NOT NULL,
    staged_endpoint TEXT NOT NULL,
    sleep_time INT NOT NULL
);