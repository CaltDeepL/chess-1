use shakmaty::{Chess, Color, Position};

/// 投了した側の逆の色が勝者になる
pub fn winner_after_resign(resigning_color: Color) -> &'static str {
    match resigning_color {
        Color::White => "black_win",
        Color::Black => "white_win",
    }
}

/// 終局した局面から (result, end_reason) を判定するヘルパー
pub fn determine_outcome(position: &Chess) -> (&'static str, &'static str) {
    if position.is_checkmate() {
        let winner = match position.turn() {
            Color::White => "black_win",
            Color::Black => "white_win",
        };
        (winner, "checkmate")
    } else if position.is_stalemate() {
        ("draw", "stalemate")
    } else if position.is_insufficient_material() {
        ("draw", "insufficient_material")
    } else {
        ("draw", "other")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::{fen::Fen, uci::UciMove, CastlingMode};

    /// 初期局面からUCI形式の指し手を順に適用して局面を作るテスト用ヘルパー
    fn play_moves(ucis: &[&str]) -> Chess {
        let mut position = Chess::default();
        for uci in ucis {
            let mv: UciMove = uci.parse().unwrap();
            let mv = mv.to_move(&position).unwrap();
            position = position.play(&mv).unwrap();
        }
        position
    }

    fn position_from_fen(fen: &str) -> Chess {
        fen.parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap()
    }

    #[test]
    fn winner_after_resign_white_gives_black_win() {
        assert_eq!(winner_after_resign(Color::White), "black_win");
    }

    #[test]
    fn winner_after_resign_black_gives_white_win() {
        assert_eq!(winner_after_resign(Color::Black), "white_win");
    }

    #[test]
    fn determine_outcome_fools_mate_is_black_win_by_checkmate() {
        // フールズメイト: 白が2手で詰まされる(手順はcheckmate_test.rsと同じ)
        let position = play_moves(&["f2f3", "e7e5", "g2g4", "d8h4"]);
        assert_eq!(determine_outcome(&position), ("black_win", "checkmate"));
    }

    #[test]
    fn determine_outcome_scholars_mate_is_white_win_by_checkmate() {
        // スカラーズメイト: 黒が4手で詰まされる(手順はcheckmate_test.rsと同じ)
        let position = play_moves(&["e2e4", "e7e5", "f1c4", "b8c6", "d1h5", "g8f6", "h5f7"]);
        assert_eq!(determine_outcome(&position), ("white_win", "checkmate"));
    }

    #[test]
    fn determine_outcome_stalemate_is_draw() {
        // 黒番だが合法手が一つもない古典的なステイルメイト局面
        let position = position_from_fen("7k/5K2/6Q1/8/8/8/8/8 b - - 0 1");
        assert_eq!(determine_outcome(&position), ("draw", "stalemate"));
    }

    #[test]
    fn determine_outcome_insufficient_material_is_draw() {
        // キング同士のみで詰ませようがない局面
        let position = position_from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(
            determine_outcome(&position),
            ("draw", "insufficient_material")
        );
    }
}
