-- Booking settings + notification templates + booking notifications log.

-- ---------- Booking settings (single-row config table) ----------
CREATE TABLE IF NOT EXISTS booking_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    -- 'automatic' = bookings auto-confirm if the slot is free
    -- 'approval'  = bookings wait for staff approval
    booking_mode VARCHAR(20) DEFAULT 'approval',
    -- auto-send confirmation messages when a booking is received/confirmed
    auto_confirm_message INTEGER DEFAULT 1,
    auto_reminder_message INTEGER DEFAULT 1,
    reminder_hours_before INTEGER DEFAULT 24,
    -- provider config (set via the app; secrets stored here for local-only system)
    email_provider VARCHAR(20) DEFAULT 'postmark',
    email_api_key TEXT,
    email_from VARCHAR(200) DEFAULT 'bookings@clinic.local',
    sms_provider VARCHAR(20) DEFAULT 'clicksend',
    sms_api_key TEXT,
    sms_sender VARCHAR(20) DEFAULT 'OptiCore',
    -- templates (with {{name}}, {{date}}, {{time}}, {{type}} placeholders)
    template_booking_received TEXT DEFAULT 'Hi {{name}}, we received your booking request for {{date}} at {{time}} ({{type}}). We will confirm shortly. — OptiCore',
    template_booking_confirmed TEXT DEFAULT 'Hi {{name}}, your appointment is confirmed for {{date}} at {{time}} ({{type}}). Reply or call us if you need to change it. — OptiCore',
    template_booking_declined TEXT DEFAULT 'Hi {{name}}, unfortunately the requested time ({{date}} {{time}}) is no longer available. Please book again at a different time. — OptiCore',
    template_reminder TEXT DEFAULT 'Reminder: {{name}}, you have an appointment tomorrow at {{time}} ({{type}}). — OptiCore',
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
INSERT OR IGNORE INTO booking_settings (id) VALUES (1);

-- ---------- Booking notifications log (outgoing messages) ----------
CREATE TABLE IF NOT EXISTS booking_notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    booking_id INTEGER,
    intake_submission_id INTEGER,
    channel VARCHAR(10) NOT NULL,  -- email | sms
    recipient VARCHAR(200) NOT NULL,
    template_used VARCHAR(40),
    body TEXT NOT NULL,
    status VARCHAR(20) DEFAULT 'pending',  -- pending | sent | failed | skipped
    provider_response TEXT,
    sent_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_notif_status ON booking_notifications(status);
CREATE INDEX IF NOT EXISTS idx_notif_booking ON booking_notifications(booking_id);
