ALTER TABLE submissions ADD COLUMN edit_code_ciphertext TEXT;
ALTER TABLE submissions ADD COLUMN edit_code_version INTEGER NOT NULL DEFAULT 0 CHECK (edit_code_version >= 0);
