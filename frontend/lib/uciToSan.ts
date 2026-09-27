import { Chess } from "chess.js";

/**
 * UCI の列を初手から再生して SAN に変換する。
 *
 * 通信は UCI（`e2e4`）、表示は SAN（`e4`, `Nf3`, `O-O`）という
 * 使い分けは task-22 で決めたもの。変換にはそれまでの手順が必要なので、
 * 1手ずつではなく列ごと渡す。
 *
 * 不正な手が混ざっていても表示全体は落とさず、その手を UCI のまま返す。
 * サーバーが権威なので通常は起きないが、古い棋譜や破損データを表示する
 * ときのフォールバックになる。
 */
export function uciListToSan(ucis: string[]): string[] {
  const chess = new Chess();
  const sans: string[] = [];

  for (const uci of ucis) {
    try {
      const move = chess.move({
        from: uci.slice(0, 2),
        to: uci.slice(2, 4),
        promotion: uci.length > 4 ? uci[4] : undefined,
      });
      sans.push(move.san);
    } catch {
      sans.push(uci);
    }
  }

  return sans;
}
