interface GameMenuProps {
  onResign: () => void;
  onLogout: () => void;
  resignDisabled?: boolean;
  /**
   * 対局が進行中か。
   *
   * 進行中はログアウトを表示しない。ログアウトは即敗北になるため、
   * 警告して許すより操作させないほうが単純。
   * 対局から抜けたい場合は投了する(そちらは常に押せる)。
   */
  isGameActive?: boolean;
}

export default function GameMenu({
  onResign,
  onLogout,
  resignDisabled,
  isGameActive,
}: GameMenuProps) {
  return (
    <div className="game-menu">
      <button onClick={onResign} disabled={resignDisabled}>
        投了
      </button>
      {/* 対局が終われば戻る */}
      {!isGameActive && <button onClick={onLogout}>ログアウト</button>}
    </div>
  );
}
