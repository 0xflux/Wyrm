-- Add migration script here
CREATE TABLE agents (
    id SERIAL PRIMARY KEY,
    first_check_in TIMESTAMPTZ DEFAULT now()
);