import { UNICODE_SYMBOLS } from "../lib/pieceSymbols";
import type { PieceColor, PieceType } from "../lib/pieceSymbols";

export default function UnicodePiece({ type, color }: { type: PieceType; color: PieceColor }) {
  return (
    <span className={`glass-piece glass-piece--${color}`} aria-hidden="true">
      {UNICODE_SYMBOLS[color][type]}
    </span>
  );
}
