-- Add migration script here
BEGIN;

ALTER TABLE tasks
  DROP CONSTRAINT IF EXISTS fk_tasks_agent;
DROP INDEX IF EXISTS idx_tasks_incomplete;

ALTER TABLE agents
  ADD CONSTRAINT uq_agents_uid UNIQUE(uid);

ALTER TABLE tasks
  ADD COLUMN new_agent_id TEXT NOT NULL DEFAULT '';

UPDATE tasks
SET new_agent_id = agents.uid
FROM agents
WHERE tasks.agent_id = agents.id;

ALTER TABLE tasks
  DROP COLUMN agent_id;
ALTER TABLE tasks
  RENAME COLUMN new_agent_id TO agent_id;

ALTER TABLE tasks
  ADD CONSTRAINT fk_tasks_agent
    FOREIGN KEY (agent_id)
    REFERENCES agents(uid)
    ON DELETE CASCADE;

CREATE INDEX idx_tasks_incomplete 
  ON tasks (agent_id)
  WHERE completed = FALSE;

COMMIT;