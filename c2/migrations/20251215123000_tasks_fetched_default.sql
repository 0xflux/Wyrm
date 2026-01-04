-- Add migration script here
UPDATE tasks
SET fetched = FALSE
WHERE fetched IS NULL;

ALTER TABLE tasks
    ALTER COLUMN fetched SET DEFAULT FALSE;

ALTER TABLE tasks
    ALTER COLUMN fetched SET NOT NULL;
