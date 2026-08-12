import { useEffect, useRef } from "react";
import type { MoveRow } from "../types";

interface MoveHistoryProps {
  moves: MoveRow[];
}

export default function MoveHistory({ moves }: MoveHistoryProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [moves.length]);

  // 1手ずつ(白番→黒番)を1行にまとめる
  const rows: { number: number; white?: string; black?: string }[] = [];
  moves.forEach((m, i) => {
    const pairIndex = Math.floor(i / 2);
    if (!rows[pairIndex]) rows[pairIndex] = { number: pairIndex + 1 };
    if (i % 2 === 0) rows[pairIndex].white = m.uci;
    else rows[pairIndex].black = m.uci;
  });

  return (
    <aside className="move-history">
      <h3>棋譜</h3>
      {rows.length === 0 ? (
        <p className="move-history-empty">まだ指し手はありません</p>
      ) : (
        <div className="move-history-list">
          {rows.map((row) => (
            <div key={row.number} className="move-history-row">
              <span className="move-history-number">{row.number}.</span>
              <span className="move-history-move">{row.white}</span>
              <span className="move-history-move">{row.black ?? ""}</span>
            </div>
          ))}
          <div ref={bottomRef} />
        </div>
      )}
    </aside>
  );
}