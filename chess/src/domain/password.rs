//! パスワードとユーザー名の検証。
//!
//! I/O を持たない純粋関数なので DB 不要でテストできる
//! (domain/outcome.rs, player.rs, history.rs, elo.rs と同じ方針)。
//!
//! 方針は NIST SP 800-63B に沿う:
//! - 長さを主な強度の担保とし、文字種の強制はしない
//! - 長いパスフレーズを許可する
//! - よく使われるパスワードは拒否する
//!
//! 文字種を強制すると `Password1!` のような、覚えにくいわりに
//! 破られやすいパスワードに誘導してしまう。

/// 最小文字数。NIST の推奨は8だが、文字種を強制しないぶん長さで補う。
pub const MIN_PASSWORD_CHARS: usize = 12;

/// 最大文字数。上限が無いと巨大な入力で Argon2 のハッシュ化が詰まる。
pub const MAX_PASSWORD_CHARS: usize = 128;

pub const MAX_USERNAME_CHARS: usize = 32;

/// よく使われるパスワードの拒否リスト。
///
/// 網羅性は目的ではない。「最頻出のものを弾く」ための最小限で、
/// 本格的にやるなら Have I Been Pwned の k-anonymity API や
/// zxcvbn のようなライブラリに寄せる（引き継ぎ参照）。
const COMMON_PASSWORDS: &[&str] = &[
    "password",
    "password1",
    "password12",
    "password123",
    "password1234",
    "passw0rd",
    "12345678",
    "123456789",
    "1234567890",
    "123456789012",
    "qwertyuiop",
    "qwerty123",
    "qwertyuiop123",
    "adminadmin",
    "administrator",
    "letmein123",
    "welcome123",
    "iloveyou123",
    "abc123456789",
    "aaaaaaaaaaaa",
    "111111111111",
    "correcthorsebatterystaple", // 有名になりすぎた例
];

#[derive(Debug, PartialEq, Eq)]
pub enum PasswordError {
    TooShort,
    TooLong,
    TooCommon,
    ContainsUsername,
}

impl PasswordError {
    /// 利用者に見せる文言。
    ///
    /// 「なぜ弾かれたか」と「どうすればよいか」の両方を含める。
    /// 「パスワードが不正です」だけでは直しようがない。
    pub fn detail(&self) -> String {
        match self {
            Self::TooShort => format!(
                "パスワードは{MIN_PASSWORD_CHARS}文字以上にしてください。\
                 記号や数字は必須ではないので、覚えやすい文の組み合わせでも構いません"
            ),
            Self::TooLong => {
                format!("パスワードは{MAX_PASSWORD_CHARS}文字以内にしてください")
            }
            Self::TooCommon => {
                "そのパスワードは広く知られており、推測されやすいため使用できません".to_string()
            }
            Self::ContainsUsername => "パスワードにユーザー名を含めないでください".to_string(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum UsernameError {
    Empty,
    TooLong,
}

impl UsernameError {
    pub fn detail(&self) -> String {
        match self {
            Self::Empty => "ユーザー名を入力してください".to_string(),
            Self::TooLong => {
                format!("ユーザー名は{MAX_USERNAME_CHARS}文字以内にしてください")
            }
        }
    }
}

/// パスワードを検証する。
///
/// 長さは**文字数**で数える。`str::len()` はバイト数を返すため、
/// 日本語やアクセント付き文字を含むパスフレーズが不当に長く
/// 評価されてしまう（「正しい馬の電池」は7文字だが21バイト）。
pub fn validate_password(password: &str, username: &str) -> Result<(), PasswordError> {
    let char_count = password.chars().count();

    if char_count < MIN_PASSWORD_CHARS {
        return Err(PasswordError::TooShort);
    }
    if char_count > MAX_PASSWORD_CHARS {
        return Err(PasswordError::TooLong);
    }

    let lower = password.to_lowercase();

    if COMMON_PASSWORDS.contains(&lower.as_str()) {
        return Err(PasswordError::TooCommon);
    }

    // ユーザー名そのものを含むパスワードは、そのユーザーを狙う攻撃に弱い
    let username_lower = username.trim().to_lowercase();
    if !username_lower.is_empty() && lower.contains(&username_lower) {
        return Err(PasswordError::ContainsUsername);
    }

    Ok(())
}

pub fn validate_username(username: &str) -> Result<(), UsernameError> {
    let trimmed = username.trim();

    if trimmed.is_empty() {
        return Err(UsernameError::Empty);
    }
    if trimmed.chars().count() > MAX_USERNAME_CHARS {
        return Err(UsernameError::TooLong);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_passphrase_is_accepted() {
        assert_eq!(
            validate_password("紫陽花と傘と月曜日の帰り道", "alice"),
            Ok(())
        );
        assert_eq!(
            validate_password("my cat sleeps on the keyboard", "alice"),
            Ok(())
        );
    }

    #[test]
    fn symbols_and_digits_are_not_required() {
        // 文字種を強制しない方針の確認
        assert_eq!(validate_password("abcdefghijkl", "alice"), Ok(()));
    }

    #[test]
    fn short_password_is_rejected() {
        assert_eq!(
            validate_password("short1234", "alice"),
            Err(PasswordError::TooShort)
        );
    }

    #[test]
    fn length_is_counted_in_chars_not_bytes() {
        // 7文字（21バイト）。バイト数で数えていると誤って通ってしまう
        assert_eq!(
            validate_password("正しい馬の電池", "alice"),
            Err(PasswordError::TooShort)
        );
        // 12文字ちょうど（36バイト）は通る
        assert_eq!(
            validate_password("正しい馬の電池と青い空色", "alice"),
            Ok(())
        );
    }

    #[test]
    fn overly_long_password_is_rejected() {
        // 上限が無いと Argon2 のハッシュ化でサーバーが詰まる
        let huge = "a".repeat(MAX_PASSWORD_CHARS + 1);
        assert_eq!(
            validate_password(&huge, "alice"),
            Err(PasswordError::TooLong)
        );
    }

    #[test]
    fn common_passwords_are_rejected() {
        assert_eq!(
            validate_password("password1234", "alice"),
            Err(PasswordError::TooCommon)
        );
        assert_eq!(
            validate_password("123456789012", "alice"),
            Err(PasswordError::TooCommon)
        );
    }

    #[test]
    fn common_password_check_is_case_insensitive() {
        assert_eq!(
            validate_password("PassWord1234", "alice"),
            Err(PasswordError::TooCommon)
        );
    }

    #[test]
    fn password_containing_the_username_is_rejected() {
        assert_eq!(
            validate_password("aliceisgreat123", "alice"),
            Err(PasswordError::ContainsUsername)
        );
        assert_eq!(
            validate_password("myALICEpassphrase", "Alice"),
            Err(PasswordError::ContainsUsername)
        );
    }

    #[test]
    fn length_is_checked_before_the_blocklist() {
        // 短くて かつ ありふれた場合、まず長さを伝える。
        // 「よくあるパスワードです」と言われて別の短いものに変えても、
        // また弾かれて往復が増える
        assert_eq!(
            validate_password("password", "alice"),
            Err(PasswordError::TooShort)
        );
    }

    #[test]
    fn error_messages_explain_what_to_do() {
        // 「不正です」だけでは直しようがない
        let msg = PasswordError::TooShort.detail();
        assert!(msg.contains(&MIN_PASSWORD_CHARS.to_string()));
    }

    #[test]
    fn empty_username_is_rejected() {
        assert_eq!(validate_username("   "), Err(UsernameError::Empty));
    }

    #[test]
    fn overly_long_username_is_rejected() {
        let long = "a".repeat(MAX_USERNAME_CHARS + 1);
        assert_eq!(validate_username(&long), Err(UsernameError::TooLong));
    }

    #[test]
    fn normal_username_is_accepted() {
        assert_eq!(validate_username("alice"), Ok(()));
        assert_eq!(validate_username("石田ひとみ"), Ok(()));
    }
}
