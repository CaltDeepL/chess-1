//! Elo レーティングの計算。
//!
//! I/O を持たない純粋関数なので DB 不要でテストできる
//! (domain/outcome.rs, domain/player.rs, domain/history.rs と同じ方針)。

/// K 値。1局あたりの変動の大きさを決める。
///
/// 対局数に応じて可変にする（暫定レーティング）方式もあるが、
/// `games_played` の管理が必要になるため、まずは固定値とする。
pub const K_FACTOR: f64 = 32.0;

/// レーティング差から期待勝率を求める。
///
/// 差が 0 なら 0.5、+400 なら約 0.909。
pub fn expected_score(rating: i32, opponent_rating: i32) -> f64 {
    let diff = f64::from(opponent_rating - rating);
    1.0 / (1.0 + 10f64.powf(diff / 400.0))
}

/// 白側の変動値を求める。黒側はこの符号を反転した値になる。
///
/// `score` は白から見た結果（勝ち 1.0 / 引き分け 0.5 / 負け 0.0）。
///
/// 白黒それぞれ独立に計算して丸めると、合計が ±1 ずれてレーティングの
/// 総和が保存されない。片方だけ計算して反転させることでゼロサムを保証する。
pub fn white_delta(white_rating: i32, black_rating: i32, score: f64) -> i32 {
    let expected = expected_score(white_rating, black_rating);
    (K_FACTOR * (score - expected)).round() as i32
}

/// `games.result` の値から、白から見たスコアに変換する。
///
/// 未知の値では `None` を返す。ここで panic すると、
/// DB に想定外の値が1件入っただけで終局処理全体が落ちる。
pub fn score_from_result(result: &str) -> Option<f64> {
    match result {
        "white_win" => Some(1.0),
        "black_win" => Some(0.0),
        "draw" => Some(0.5),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_ratings_expect_half() {
        assert!((expected_score(1500, 1500) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn higher_rating_expects_more() {
        assert!(expected_score(1900, 1500) > 0.9);
        assert!(expected_score(1500, 1900) < 0.1);
    }

    #[test]
    fn expectations_sum_to_one() {
        let a = expected_score(1720, 1480);
        let b = expected_score(1480, 1720);
        assert!((a + b - 1.0).abs() < 1e-9);
    }

    #[test]
    fn equal_ratings_win_gives_half_k() {
        // 期待勝率0.5の相手に勝てば K/2 = 16
        assert_eq!(white_delta(1500, 1500, 1.0), 16);
        assert_eq!(white_delta(1500, 1500, 0.0), -16);
    }

    #[test]
    fn equal_ratings_draw_gives_zero() {
        assert_eq!(white_delta(1500, 1500, 0.5), 0);
    }

    #[test]
    fn beating_a_stronger_player_gains_more() {
        let upset = white_delta(1400, 1800, 1.0);
        let expected_win = white_delta(1800, 1400, 1.0);
        assert!(
            upset > expected_win,
            "格上に勝つほうが伸びる: {upset} vs {expected_win}"
        );
    }

    #[test]
    fn losing_to_a_stronger_player_costs_little() {
        let loss_to_strong = white_delta(1400, 1800, 0.0);
        let loss_to_weak = white_delta(1800, 1400, 0.0);
        assert!(
            loss_to_strong > loss_to_weak,
            "格上に負けるほうが失点が小さい: {loss_to_strong} vs {loss_to_weak}"
        );
    }

    #[test]
    fn draw_against_a_stronger_player_gains() {
        assert!(white_delta(1400, 1800, 0.5) > 0);
    }

    #[test]
    fn delta_never_exceeds_k() {
        for score in [0.0, 0.5, 1.0] {
            for (w, b) in [(1000, 2400), (2400, 1000), (1500, 1500)] {
                let d = white_delta(w, b, score);
                assert!(
                    d.abs() <= K_FACTOR as i32,
                    "変動が K を超えた: {d} ({w} vs {b}, score={score})"
                );
            }
        }
    }

    #[test]
    fn score_conversion() {
        assert_eq!(score_from_result("white_win"), Some(1.0));
        assert_eq!(score_from_result("black_win"), Some(0.0));
        assert_eq!(score_from_result("draw"), Some(0.5));
        assert_eq!(score_from_result("aborted"), None);
    }
}
