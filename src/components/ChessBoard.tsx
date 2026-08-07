import { Chessboard } from "react-chessboard";
import type { ChessboardOptions } from "react-chessboard";

interface ChessBoardProps {
  fen: string;
  orientation: "white" | "black";
  onPieceDrop: (sourceSquare: string, targetSquare: string) => boolean;
  isMyTurn: boolean;
}

export default function ChessBoard({
  fen,
  orientation,
  onPieceDrop,
  isMyTurn,
}: ChessBoardProps) {
  const options: ChessboardOptions = {
    position: fen,
    boardOrientation: orientation,
    allowDragging: isMyTurn,
    onPieceDrop: ({ sourceSquare, targetSquare }) => {
      if (!targetSquare) return false;
      return onPieceDrop(sourceSquare, targetSquare);
    },
    boardStyle: {
      borderRadius: "4px",
      boxShadow: "0 2px 10px rgba(0, 0, 0, 0.3)",
    },
  };

  return (
    <div className="chess-board-wrapper">
      <Chessboard options={options} />
    </div>
  );
}