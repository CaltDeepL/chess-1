import { useEffect, useMemo, useRef } from "react";
import { Chess } from "chess.js";
import type { MoveRow } from "../types";

interface MoveHistoryProps {
  moves: MoveRow[];
}

// UCI表記("e2e4"/"e7e8q"等)をSAN(標準代数記法、"e4"/"Nf3"/"O-O"/"Qxe5+"等)に変換する。
// サーバー(shakmaty)が権威の指し手をUCIで送ってくるため、表示用にchess.jsで
// 手順を最初から再生してSANを復元する(合法手ハイライトと同じ「表示専用」の使い方)。
function toSan(moves: MoveRow[]): string[] {
  const chess = new Chess();
  const sans: string[] = [];
  for (const m of moves) {
    try {
      const from = m.uci.slice(0, 2);
      const to = m.uci.slice(2, 4);
      const promotion = m.uci.length > 4 ? m.uci.slice(4) : undefined;
      const result = chess.move({ from, to, promotion });
      sans.push(result.san);
    } catch {
      // 想定外の形式ならUCIのまま表示してフォールバックする(表示専用のため落とさない)
      sans.push(m.uci);
    }
  }
  return sans;
}

export default function MoveHistory({ moves }: MoveHistoryProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [moves.length]);

  const sans = useMemo(() => toSan(moves), [moves]);

  // 1手ずつ(白番→黒番)を1行にまとめる
  const rows: { number: number; white?: string; black?: string }[] = [];
  sans.forEach((san, i) => {
    const pairIndex = Math.floor(i / 2);
    if (!rows[pairIndex]) rows[pairIndex] = { number: pairIndex + 1 };
    if (i % 2 === 0) rows[pairIndex].white = san;
    else rows[pairIndex].black = san;
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