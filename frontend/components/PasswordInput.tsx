import { useState, useId } from "react";

interface Props {
  value: string;
  onChange: (value: string) => void;
  /** ラベルの文言 */
  label?: string;
  /**
   * 新規作成用か既存の入力か。
   * ブラウザとパスワードマネージャーが「保存するか」「補完するか」を
   * これで判断するため、必ず使い分ける。
   */
  autoComplete: "new-password" | "current-password";
  /** input の下に出す補足（要件の案内など） */
  hint?: string;
  required?: boolean;
  disabled?: boolean;
}

/**
 * 表示・非表示を切り替えられるパスワード入力。
 *
 * 目のアイコンは絵文字（👁）ではなくインライン SVG にしている。
 * 絵文字は環境によって字形とサイズが大きく変わり、ボタンの当たり判定が
 * 揃わないため。SVG なら currentColor で配色にも追従する。
 */
export default function PasswordInput({
  value,
  onChange,
  label = "パスワード",
  autoComplete,
  hint,
  required,
  disabled,
}: Props) {
  const [isVisible, setIsVisible] = useState(false);
  // 同じ画面に複数置いても id が衝突しない
  const inputId = useId();
  const hintId = useId();

  return (
    <div className="password-field">
      <label htmlFor={inputId}>{label}</label>

      <div className="password-input-wrapper">
        <input
          id={inputId}
          type={isVisible ? "text" : "password"}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          autoComplete={autoComplete}
          aria-describedby={hint ? hintId : undefined}
          required={required}
          disabled={disabled}
        />

        <button
          type="button"          /* 明示しないとフォームを送信してしまう */
          className="password-toggle"
          onClick={() => setIsVisible((v) => !v)}
          aria-label={isVisible ? "パスワードを隠す" : "パスワードを表示する"}
          aria-pressed={isVisible}
          disabled={disabled}
        >
          {isVisible ? <EyeOffIcon /> : <EyeIcon />}
        </button>
      </div>

      {hint && (
        <p id={hintId} className="password-hint">
          {hint}
        </p>
      )}
    </div>
  );
}

/* アイコンは fill="none" + stroke="currentColor" で、文字色に追従させる */

function EyeIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}

function EyeOffIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M9.9 4.24A9.1 9.1 0 0 1 12 4c6.5 0 10 7 10 7a17.6 17.6 0 0 1-2.7 3.55" />
      <path d="M6.6 6.6A17.7 17.7 0 0 0 2 11s3.5 7 10 7a9 9 0 0 0 5.4-1.6" />
      <path d="m1 1 22 22" />
      <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24" />
    </svg>
  );
}
