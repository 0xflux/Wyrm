-- Add migration script here
ALTER TABLE agent_staging
    ADD CONSTRAINT uq_agent_name UNIQUE (agent_name),
    ADD CONSTRAINT uq_c2_endpoint UNIQUE (c2_endpoint),
    ADD CONSTRAINT uq_staged_endpoint UNIQUE (staged_endpoint);