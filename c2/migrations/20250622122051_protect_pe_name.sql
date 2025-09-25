-- Add migration script here
ALTER TABLE agent_staging
    ADD CONSTRAINT uq_pe_name UNIQUE (pe_name);