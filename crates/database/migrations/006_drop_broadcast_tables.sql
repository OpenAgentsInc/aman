-- Drop broadcast-related tables
-- notifications must be dropped first due to foreign key constraint

DROP TABLE IF EXISTS notifications;
DROP TABLE IF EXISTS topics;
