
-- 各プレイヤーが「いつから接続していないか」。
-- 接続中は NULL。WebSocket の接続が0本になった時刻を入れる。
--
-- 「切断中フラグ + 時刻」ではなく時刻だけにしているのは、
-- 状態が2つに分かれると片方だけ更新される不整合が起きうるため。
-- NULL かどうかが接続状態そのものを表す。
ALTER TABLE games ADD COLUMN white_disconnected_at TIMESTAMPTZ;
ALTER TABLE games ADD COLUMN black_disconnected_at TIMESTAMPTZ;
 
-- 一括判定（sweep）が対象を引くためのインデックス。
-- 進行中の対局だけが対象なので部分インデックスにする。
CREATE INDEX idx_games_in_progress
    ON games (id)
    WHERE status = 'in_progress';
