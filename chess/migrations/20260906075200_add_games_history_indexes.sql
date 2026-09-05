
-- 履歴一覧は「自分が白または黒で参加した、終了済みの対局」を
-- updated_at の降順で引く。WHERE の OR は単一のインデックスでは
-- 効かないため、白番用と黒番用を別々に張る。
--
-- status = 'finished' の部分インデックスにしているのは、
-- 進行中・募集中の対局が対象外だから。インデックス自体が小さくなる。

CREATE INDEX idx_games_white_finished
    ON games (white_user_id, updated_at DESC)
    WHERE status = 'finished';

CREATE INDEX idx_games_black_finished
    ON games (black_user_id, updated_at DESC)
    WHERE status = 'finished';

-- 手数の集計で moves を game_id で引くため
CREATE INDEX IF NOT EXISTS idx_moves_game_id ON moves (game_id);