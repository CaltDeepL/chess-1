-- 既存ユーザーにも初期値が入る
ALTER TABLE users ADD COLUMN rating INTEGER NOT NULL DEFAULT 1500;

-- その対局で何点動いたかを保存する。
-- 保存しないと履歴画面で変動を表示できず、後から再計算もできない
-- （当時の相手のレーティングが分からないため）。
-- NULL は「レーティング未適用」を意味し、二重適用の防止にも使う。
ALTER TABLE games ADD COLUMN white_rating_delta INTEGER;
ALTER TABLE games ADD COLUMN black_rating_delta INTEGER;

-- ランキング表示・上位者の取得用
CREATE INDEX idx_users_rating ON users (rating DESC);