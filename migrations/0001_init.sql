-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create messages table
CREATE TABLE IF NOT EXISTS messages (
    message_id   UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    sent_at      TIMESTAMPTZ NOT NULL,
    user_id      UUID NOT NULL,
    server_id    UUID NOT NULL,
    channel_id   UUID NOT NULL,
    content      TEXT NOT NULL
);

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_messages_sent_at ON messages(sent_at);
CREATE INDEX IF NOT EXISTS idx_messages_user_id ON messages(user_id);
CREATE INDEX IF NOT EXISTS idx_messages_server_id ON messages(server_id);
CREATE INDEX IF NOT EXISTS idx_messages_channel_id ON messages(channel_id); 