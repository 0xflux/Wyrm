ALTER TABLE agent_staging
    ALTER COLUMN num_downloads TYPE BIGINT;

ALTER TABLE agent_staging
    ALTER COLUMN num_downloads SET DEFAULT 0;