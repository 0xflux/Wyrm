-- Add migration script here
CREATE TABLE operators (
    id SERIAL PRIMARY KEY,
    date_created TIMESTAMPTZ DEFAULT now(),
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    salt TEXT NOT NULL
);