//! 履歴表示のための、自分視点の勝敗判定。
//!
//! `games.result` は "white_win" / "black_win" / "draw" という
//! 盤面視点の値なので、「自分が勝ったか」に変換する必要がある。
//! I/O を持たない純粋関数なので DB 不要でテストできる(task-29 と同じ方針)。

use shakmaty::Color;

/// 自分視点の対局結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Win,
    Loss,
    Draw,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Win => "win",
            Self::Loss => "loss",
            Self::Draw => "draw",
        }
    }
}

/// `games.result` と自分の手番の色から、自分視点の勝敗を求める。
///
/// `result` が NULL の対局(終了していない、または結果未設定)は `None`。
pub fn outcome_for(result: Option<&str>, my_color: Color) -> Option<Outcome> {
    match (result?, my_color) {
        ("draw", _) => Some(Outcome::Draw),
        ("white_win", Color::White) | ("black_win", Color::Black) => Some(Outcome::Win),
        ("white_win", Color::Black) | ("black_win", Color::White) => Some(Outcome::Loss),
        // 想定外の値は「判定できない」として扱う。
        // ここで panic すると、DBに未知の値が入っただけで一覧全体が落ちる。
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_win_is_a_win_for_white() {
        assert_eq!(
            outcome_for(Some("white_win"), Color::White),
            Some(Outcome::Win)
        );
    }

    #[test]
    fn white_win_is_a_loss_for_black() {
        assert_eq!(
            outcome_for(Some("white_win"), Color::Black),
            Some(Outcome::Loss)
        );
    }

    #[test]
    fn black_win_is_a_win_for_black() {
        assert_eq!(
            outcome_for(Some("black_win"), Color::Black),
            Some(Outcome::Win)
        );
    }

    #[test]
    fn black_win_is_a_loss_for_white() {
        assert_eq!(
            outcome_for(Some("black_win"), Color::White),
            Some(Outcome::Loss)
        );
    }

    #[test]
    fn draw_is_a_draw_for_both_colors() {
        assert_eq!(outcome_for(Some("draw"), Color::White), Some(Outcome::Draw));
        assert_eq!(outcome_for(Some("draw"), Color::Black), Some(Outcome::Draw));
    }

    #[test]
    fn missing_result_has_no_outcome() {
        assert_eq!(outcome_for(None, Color::White), None);
    }

    #[test]
    fn unknown_result_has_no_outcome() {
        // 将来 result に値が増えたとき、一覧全体を落とさない
        assert_eq!(outcome_for(Some("aborted"), Color::White), None);
    }
}
