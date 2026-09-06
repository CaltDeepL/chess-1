import { Chess } from "chess.js";

/**
 * UCI の列を初手から再生して SAN に変換する。
 *
 * 通信は UCI（`e2e4`）、表示は SAN（`e4`, `Nf3`, `O-O`）という
 * 使い分けは task-22 で決めたもの。変換にはそれまでの手順が必要なので、
 * 1手ずつではなく列ごと渡す。
 *
 * 不正な手が混ざっていた場合はそこで打ち切り、変換できた分までを返す。
 * 例外を投げると棋譜の表示全体が消えてしまうため。
 */
export function uciListToSan(ucis: string[]): string[] {
  const chess = new Chess();
  const sans: string[] = [];

  for (const uci of ucis) {
    const move = chess.move({
      from: uci.slice(0, 2),
      to: uci.slice(2, 4),
      promotion: uci.length > 4 ? uci[4] : undefined,
    });
    if (!move) break;
    sans.push(move.san);
  }

  return sans;
}