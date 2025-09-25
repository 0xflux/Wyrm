-- Add migration script here
CREATE TABLE tasks (
    id SERIAL PRIMARY KEY,
    uid TEXT NOT NULL,
    data TEXT
);