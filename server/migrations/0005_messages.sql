-- Unified messages inbox: email, WhatsApp, website messages — all in one place.
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    received_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    channel VARCHAR(20) NOT NULL,          -- email | whatsapp | website | sms
    from_name VARCHAR(200),
    from_contact VARCHAR(200),              -- email address / phone number / website visitor id
    subject VARCHAR(300),
    body TEXT NOT NULL,
    status VARCHAR(20) DEFAULT 'unread',    -- unread | read | archived | replied
    linked_patient_id INTEGER,              -- matched patient if any
    thread_id VARCHAR(60),                  -- for grouping conversations
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_messages_status ON messages(status);
CREATE INDEX IF NOT EXISTS idx_messages_channel ON messages(channel);
CREATE INDEX IF NOT EXISTS idx_messages_received ON messages(received_at);

-- Seed: a few sample messages so the inbox isn't empty.
INSERT OR IGNORE INTO messages (received_at, channel, from_name, from_contact, subject, body, status, thread_id) VALUES
('2026-07-25 08:12:00', 'website', 'Jane Carter', 'jane.carter@example.com', 'Booking enquiry',
 'Hi, I saw on your website that you do dry eye consultations. Could I book one for next week? I have been having gritty eyes for a few months. Thanks!', 'unread', 't-web-001'),
('2026-07-25 07:45:00', 'email', 'Tom Bradley', 'tom.bradley@example.com', 'IPL treatment question',
 'Hello, I had IPL session 2 last month and was wondering when I should book session 3. Is 4 weeks apart still ok?', 'unread', 't-email-002'),
('2026-07-24 16:30:00', 'whatsapp', 'Priya Shah', '+61411222333', '',
 'Hi! Do you have any availability this Friday afternoon for a follow-up?', 'unread', 't-wa-003'),
('2026-07-24 11:20:00', 'email', 'feedback@healthfund.com', 'claims@bupa.com.au', 'Claim reference 8841923',
 'Medicare claim processed for patient. Reference 8841923. Please retain for your records.', 'read', 't-email-004'),
('2026-07-23 14:00:00', 'website', 'Mark Davies', 'mark.d@example.com', 'Insurance question',
 'Do you accept HCF? My member number is HCF1234567.', 'read', 't-web-005');
