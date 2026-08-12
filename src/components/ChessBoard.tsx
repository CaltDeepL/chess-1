import { useState } from "react";
import type { CSSProperties } from "react";
import { Chessboard } from "react-chessboard";
import type { ChessboardOptions } from "react-chessboard";
import { Chess } from "chess.js";
import { unicodeGlassPieces } from "./UnicodePieces";
import "../styles/glass-board.css";

interface ChessBoardProps {
  fen: string;
  orientation: "white" | "black";
  onPieceDrop: (sourceSquare: string, targetSquare: string, promotion?: string) => boolean;
  onPromotionNeeded: (sourceSquare: string, targetSquare: string) => void;
  isMyTurn: boolean;
}

const lightSquareStyle = {
  background: "linear-gradient(135deg, rgba(210,230,255,0.55), rgba(180,210,245,0.25))",
  backdropFilter: "blur(6px)",
};

const darkSquareStyle = {
  background: "linear-gradient(135deg, rgba(30,55,110,0.75), rgba(10,20,50,0.55))",
  backdropFilter: "blur(6px)",
};

// ドラッグ中、移動可能なマスに表示する目印。
// 通常の移動先は中央にドット、駒を取れるマスは縁にリングを表示する
// (どちらも react-chessboard の squareStyles 経由で各マスの内側divに重ねて描画される)。
const legalMoveDotStyle: CSSProperties = {
  background: "radial-gradient(circle, rgba(125, 211, 252, 0.65) 24%, transparent 26%)",
};

const legalCaptureRingStyle: CSSProperties = {
  background:
    "radial-gradient(circle, transparent 58%, rgba(125, 211, 252, 0.65) 60%, transparent 66%)",
};

export default function ChessBoard({
  fen,
  orientation,
  onPieceDrop,
  onPromotionNeeded,
  isMyTurn,
}: ChessBoardProps) {
  // ドラッグ中の駒がどのマスへ動かせるかを示すハイライト。
  // 実際の合法手判定はサーバー(shakmaty)側の権威だが、ここでは見た目のガイドとして
  // chess.jsでクライアント側だけで計算する(サーバーへの問い合わせは行わない)。
  const [legalTargets, setLegalTargets] = useState<Record<string, CSSProperties>>({});

  const options: ChessboardOptions = {
    position: fen,
    boardOrientation: orientation,
    allowDragging: isMyTurn,

    // allowDraggingとcanDragPieceはdnd-kit内部で独立に評価されうるため、
    // isMyTurnはallowDraggingだけでなくここでも明示的にチェックする。
    // これが無いと(プロモーション選択待ちでisMyTurnがfalseの間なども含めて)
    // 自分の手番でない/操作不可であるべき瞬間でも、色さえ合えば駒を
    // ドラッグできてしまう。
    canDragPiece: ({ piece }) => {
      if (!isMyTurn) return false;
      const pieceColor = piece.pieceType.startsWith("w") ? "white" : "black";
      return pieceColor === orientation;
    },
    pieces: unicodeGlassPieces,
    // 各マスをコンテナクエリのコンテナにする。これが無いと.glass-pieceのcqw単位が
    // マス自身の幅ではなく、より外側の祖先(実質ビューポート相当)を基準にスケールしてしまう。
    squareStyle: {
      containerType: "inline-size",
    },
    squareStyles: legalTargets,
    // ドラッグ中の駒はdnd-kitのDragOverlayでdocument.body直下にportalされ、
    // 盤内のマス(container-type指定済み)の外に出てしまう。
    // このスタイルはportalされたピース本体(サイズは元のマスと同じ)に直接当たるので、
    // ここにもcontainer-typeを指定してcqwの基準を復元する。
    draggingPieceStyle: {
      containerType: "inline-size",
    },
    lightSquareStyle,
    darkSquareStyle,
    // ドラッグ開始: つまんだ駒がどこへ動かせるかをハイライトする。
    // 実際のプロパティ名はライブラリの型定義(ChessboardOptions)で確認したところ
    // onPieceDragBegin/onPieceDragEndではなく onPieceDrag/onPieceDragCancel だった。
    onPieceDrag: ({ square }) => {
      if (!square) return;
      try {
        const chess = new Chess(fen);
        const moves = chess.moves({ square: square as never, verbose: true });
        const styles: Record<string, CSSProperties> = {};
        for (const move of moves) {
          styles[move.to] = move.captured ? legalCaptureRingStyle : legalMoveDotStyle;
        }
        setLegalTargets(styles);
      } catch {
        // 現在のfenがchess.jsで解釈できない場合はハイライトなしで諦める(表示上のガイドに過ぎないため)
        setLegalTargets({});
      }
    },
    onPieceDragCancel: () => setLegalTargets({}),
    onPieceDrop: ({ sourceSquare, targetSquare, piece }) => {
      setLegalTargets({});
      if (!targetSquare) return false;
      // ポーンが最終ランクに到達した場合はプロモーション。駒種選択UIで確定するまで
      // このドロップ自体は不成立扱いにして駒を元の位置に戻し、選択後に
      // onPieceDrop(sourceSquare, targetSquare, promotion) を親側から呼んでもらう。
      const isPawn = piece.pieceType.endsWith("P");
      const isLastRank = targetSquare[1] === "8" || targetSquare[1] === "1";
      if (isPawn && isLastRank) {
        onPromotionNeeded(sourceSquare, targetSquare);
        return false;
      }
      return onPieceDrop(sourceSquare, targetSquare);
    },
    boardStyle: {
      borderRadius: 6,
    },
  };

  return (
    <div className="glass-board-frame">
      <div className="glass-board-inner">
        <Chessboard options={options} />
      </div>
    </div>
  );
}
