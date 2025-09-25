-- Add migration script here
ALTER TABLE public.tasks
  DROP COLUMN IF EXISTS uid;