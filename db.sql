-- lottery_draw
CREATE TABLE lottery_draw (
    id UUID PRIMARY KEY,
    draw_date DATE UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT NOW()
);

-- prize_type
CREATE TABLE prize_type (
    id SERIAL PRIMARY KEY,
    code TEXT UNIQUE NOT NULL,
    prize_amount TEXT
);

-- prize_number
CREATE TABLE prize_number (
    id SERIAL PRIMARY KEY,
    draw_id UUID REFERENCES lottery_draw(id) ON DELETE CASCADE,
    prize_type_id INT REFERENCES prize_type(id),
    round INT,
    number TEXT,
    UNIQUE (draw_id, prize_type_id, round, number)
);

-- index (สำคัญมาก)
CREATE INDEX idx_draw_date ON lottery_draw(draw_date);
CREATE INDEX idx_prize_number ON prize_number(number);
CREATE INDEX idx_draw_id ON prize_number(draw_id);