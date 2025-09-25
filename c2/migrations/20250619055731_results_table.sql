-- Add migration script here
CREATE TABLE completed_tasks (
    id SERIAL PRIMARY KEY,
    task_id INT NOT NULL,
    result TEXT,
    client_pulled_update BOOLEAN NOT NULL DEFAULT FALSE,
    time_completed TIMESTAMPTZ NOT NULL DEFAULT now()
);