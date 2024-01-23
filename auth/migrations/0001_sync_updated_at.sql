-- Create a function (that can be registered on triggers) to automatically set updated_at to current_timestamp
CREATE OR REPLACE FUNCTION sync_updated_at() 
   RETURNS TRIGGER 
   LANGUAGE PLPGSQL
AS $$
BEGIN
   NEW.updated_at = current_timestamp;
   RETURN NEW;
END;
$$
