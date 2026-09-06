//! 切断による対局放棄の判定。
//!
//! 「いつ判定するか」（WebSocket接続時 / sweep）と「何を判定するか」を
//! 分離するため、判定そのものを純粋関数として切り出す。
//! 時刻を引数で受け取るので、60秒待たずにテストできる。

use chrono::{DateTime, Duration, Utc};

/// 再接続を待つ時間。
///
/// 30秒だとトンネルに入っただけで負ける。長すぎると勝った側が待たされる。
/// 持ち時間の概念が無い現状では、多少長くても実害は小さい。
pub const GRACE_SECONDS: i64 = 60;

#[derive(Debug, PartialEq, Eq)]
pub struct Abandonment {
    /// games.result に入れる値
    pub result: &'static str,
    /// games.end_reason に入れる値
    pub end_reason: &'static str,
}

/// 両者の切断時刻から、対局を放棄で終了させるべきか判定する。
///
/// `None` は「まだ終了させない」。接続中、または猶予内。
///
/// 両者とも猶予を過ぎている場合は引き分けにする。先に切断したほうを
/// 負けにする案もあるが、**その場にいない相手を勝者にすると
/// 「先に落ちたほうが負け」というレースを生む**。両者不在なら勝者なし。
pub fn decide(
    white_disconnected_at: Option<DateTime<Utc>>,
    black_disconnected_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<Abandonment> {
    let grace = Duration::seconds(GRACE_SECONDS);
    let expired = |at: Option<DateTime<Utc>>| at.is_some_and(|t| now - t >= grace);

    match (
        expired(white_disconnected_at),
        expired(black_disconnected_at),
    ) {
        (true, true) => Some(Abandonment {
            result: "draw",
            end_reason: "abandonment",
        }),
        (true, false) => Some(Abandonment {
            result: "black_win",
            end_reason: "disconnection",
        }),
        (false, true) => Some(Abandonment {
            result: "white_win",
            end_reason: "disconnection",
        }),
        (false, false) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    fn ago(seconds: i64) -> Option<DateTime<Utc>> {
        Some(now() - Duration::seconds(seconds))
    }

    #[test]
    fn both_connected_is_not_abandoned() {
        assert_eq!(decide(None, None, now()), None);
    }

    #[test]
    fn within_grace_is_not_abandoned() {
        assert_eq!(decide(ago(59), None, now()), None);
    }

    #[test]
    fn white_gone_past_grace_gives_black_the_win() {
        let d = decide(ago(61), None, now()).unwrap();
        assert_eq!(d.result, "black_win");
        assert_eq!(d.end_reason, "disconnection");
    }

    #[test]
    fn black_gone_past_grace_gives_white_the_win() {
        let d = decide(None, ago(61), now()).unwrap();
        assert_eq!(d.result, "white_win");
    }

    #[test]
    fn exactly_at_the_grace_boundary_counts_as_expired() {
        // 60秒ちょうどで成立させる。境界を「未満」にすると
        // 定期実行の間隔次第で判定が1周期ずれる
        assert!(decide(ago(GRACE_SECONDS), None, now()).is_some());
    }

    #[test]
    fn both_gone_is_a_draw() {
        let d = decide(ago(120), ago(90), now()).unwrap();
        assert_eq!(
            d.result, "draw",
            "その場にいない側を勝者にすると、先に落ちたほうが負けるレースになる"
        );
        assert_eq!(d.end_reason, "abandonment");
    }

    #[test]
    fn one_expired_one_within_grace_is_decided_by_the_expired_one() {
        // 白は切れたばかり、黒は2分前から不在 → 白の勝ち
        let d = decide(ago(5), ago(120), now()).unwrap();
        assert_eq!(d.result, "white_win");
    }
}
