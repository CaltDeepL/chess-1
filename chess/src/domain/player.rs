use shakmaty::Color;
use uuid::Uuid;

/// 対局に対するユーザーの立場。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    White,
    Black,
    NotParticipant,
}

impl Role {
    /// 参加者であれば対応するshakmatyのColorを返す
    pub fn color(&self) -> Option<Color> {
        match self {
            Role::White => Some(Color::White),
            Role::Black => Some(Color::Black),
            Role::NotParticipant => None,
        }
    }

    pub fn is_participant(&self) -> bool {
        !matches!(self, Role::NotParticipant)
    }
}

/// white_user_id/black_user_idから、指定ユーザーの対局に対する立場を判定する
pub fn role_of(user_id: Uuid, white_user_id: Uuid, black_user_id: Option<Uuid>) -> Role {
    if user_id == white_user_id {
        Role::White
    } else if Some(user_id) == black_user_id {
        Role::Black
    } else {
        Role::NotParticipant
    }
}

/// 現在の手番のユーザーIDを返す。
/// 黒番だが対戦相手がまだ参加していない場合はNone。
pub fn expected_player(
    turn: Color,
    white_user_id: Uuid,
    black_user_id: Option<Uuid>,
) -> Option<Uuid> {
    match turn {
        Color::White => Some(white_user_id),
        Color::Black => black_user_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white() -> Uuid {
        Uuid::from_u128(1)
    }

    fn black() -> Uuid {
        Uuid::from_u128(2)
    }

    fn stranger() -> Uuid {
        Uuid::from_u128(3)
    }

    #[test]
    fn role_of_returns_white_for_white_user() {
        assert_eq!(role_of(white(), white(), Some(black())), Role::White);
    }

    #[test]
    fn role_of_returns_black_for_black_user() {
        assert_eq!(role_of(black(), white(), Some(black())), Role::Black);
    }

    #[test]
    fn role_of_returns_not_participant_for_stranger() {
        assert_eq!(
            role_of(stranger(), white(), Some(black())),
            Role::NotParticipant
        );
    }

    #[test]
    fn role_of_returns_not_participant_when_black_slot_is_empty() {
        assert_eq!(role_of(stranger(), white(), None), Role::NotParticipant);
    }

    #[test]
    fn color_returns_white_for_white_role() {
        assert_eq!(Role::White.color(), Some(Color::White));
    }

    #[test]
    fn color_returns_black_for_black_role() {
        assert_eq!(Role::Black.color(), Some(Color::Black));
    }

    #[test]
    fn color_returns_none_for_not_participant() {
        assert_eq!(Role::NotParticipant.color(), None);
    }

    #[test]
    fn is_participant_reflects_role() {
        assert!(Role::White.is_participant());
        assert!(Role::Black.is_participant());
        assert!(!Role::NotParticipant.is_participant());
    }

    #[test]
    fn expected_player_returns_white_on_white_turn_regardless_of_black_slot() {
        assert_eq!(
            expected_player(Color::White, white(), Some(black())),
            Some(white())
        );
        assert_eq!(expected_player(Color::White, white(), None), Some(white()));
    }

    #[test]
    fn expected_player_returns_black_on_black_turn_when_joined() {
        assert_eq!(
            expected_player(Color::Black, white(), Some(black())),
            Some(black())
        );
    }

    #[test]
    fn expected_player_returns_none_on_black_turn_when_not_joined() {
        assert_eq!(expected_player(Color::Black, white(), None), None);
    }
}
