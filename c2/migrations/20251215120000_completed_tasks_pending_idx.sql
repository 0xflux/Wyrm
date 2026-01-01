-- Add migration script here
CREATE INDEX IF NOT EXISTS idx_completed_tasks_agent_pending
  ON completed_tasks (agent_id)
  WHERE client_pulled_update = FALSE;

