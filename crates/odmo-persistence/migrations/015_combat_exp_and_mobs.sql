-- Add experience tracking columns for Feature 07 combat progression.
ALTER TABLE characters
    ADD COLUMN IF NOT EXISTS current_experience BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS partner_transcendence_exp BIGINT NOT NULL DEFAULT 0;
