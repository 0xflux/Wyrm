-- Add migration script here
ALTER TABLE tasks
  ADD COLUMN completed   BOOLEAN    NOT NULL DEFAULT FALSE,
  ADD COLUMN agent_id    INTEGER    NOT NULL,
  ADD CONSTRAINT fk_tasks_agent
    FOREIGN KEY (agent_id)
    REFERENCES agents (id)
    ON DELETE CASCADE;

CREATE INDEX idx_tasks_incomplete
  ON tasks (agent_id)
  WHERE completed = FALSE;