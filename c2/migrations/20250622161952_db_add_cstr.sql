-- Add migration script here
ALTER TABLE operators
    ADD CONSTRAINT uq_username_operator UNIQUE (username);