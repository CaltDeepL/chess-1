import { useState, useRef, useEffect } from "react";

interface GameMenuProps {
  onResign: () => void;
  onLogout: () => void;
  resignDisabled: boolean;
}

export default function GameMenu({ onResign, onLogout, resignDisabled }: GameMenuProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  return (
    <div className="game-menu" ref={ref}>
      <button
        className="game-menu-trigger"
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="true"
        aria-expanded={open}
        aria-label="メニュー"
      >
        <span />
        <span />
        <span />
      </button>

      {open && (
        <div className="game-menu-dropdown" role="menu">
          <button
            role="menuitem"
            onClick={() => {
              setOpen(false);
              onResign();
            }}
            disabled={resignDisabled}
          >
            投了する
          </button>
          <button
            role="menuitem"
            className="game-menu-danger"
            onClick={() => {
              setOpen(false);
              onLogout();
            }}
          >
            ログアウト
          </button>
        </div>
      )}
    </div>
  );
}