-- SMS system fixes: add sms_username for ClickSend auth + opt-out flag.

-- ClickSend uses HTTP Basic Auth with (username, api_key) — not the sender ID.
ALTER TABLE booking_settings ADD COLUMN sms_username TEXT;

-- Patient SMS opt-out flag (defensive backup — ClickSend handles STOP natively)
ALTER TABLE patients ADD COLUMN sms_opt_out INTEGER DEFAULT 0;

-- Update default templates to include STOP opt-out (Spam Act compliance)
UPDATE booking_settings SET
  template_booking_received = 'Hi {{name}}, we received your booking request for {{date}} at {{time}} ({{type}}). We will confirm shortly. Reply STOP to opt out. — OptiCore',
  template_booking_confirmed = 'Hi {{name}}, your appointment is confirmed for {{date}} at {{time}} ({{type}}). Reply STOP to opt out. — OptiCore',
  template_booking_declined = 'Hi {{name}}, unfortunately the requested time ({{date}} {{time}}) is no longer available. Please book again at a different time. Reply STOP to opt out. — OptiCore',
  template_reminder = 'Reminder: {{name}}, you have an appointment tomorrow at {{time}} ({{type}}). Reply STOP to opt out. — OptiCore'
WHERE id = 1;
